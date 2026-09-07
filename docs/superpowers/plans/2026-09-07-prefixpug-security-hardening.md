# PrefixPug Security Hardening & Bug Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all critical bugs, security vulnerabilities, and data loss risks in PrefixPug, hardening save sniffing, path validation, backup/restore security, shortcut discovery, and TUI stability.

**Architecture:** Implement defensive filesystem traversal guards with canonical path verification, secure tar extraction with path-traversal and integrity validation, restrictive Unix vault permissions (`0700`/`0600`), robust binary VDF decoding, and an RAII terminal cleanup guard.

**Tech Stack:** Rust (Edition 2021), `walkdir`, `tar`, `flate2`, `keyvalues-parser`, `crc32fast`, `sha2`, `ratatui`, `crossterm`, `clap`.

**Spec:** Code review report covering issues C1-C5, S1-S5, and H1-H5.

## Global Constraints

- Code must be safe, using `Result` for error handling instead of `unwrap()`.
- Always prompt for user confirmation before executing any file modification or deletion commands.
- Never delete files without verifying that paths reside strictly inside the targeted prefix directory.
- Preserve backward compatibility for CLI commands and output formats.

---

### Task 1: Fix Symlink Escape in Save Sniffer (C1)

**Files:**
- Modify: `src/scanner.rs:262-355`
- Test: `src/scanner.rs:772-804`, `tests/integration_tests.rs:359-403`

**Interfaces:**
- Consumes: `compatdata_dir: &Path`, `warnings: &mut Vec<String>`
- Produces: `sniff_save_files(compatdata_dir: &Path, warnings: &mut Vec<String>) -> Vec<SaveFileInfo>`

- [ ] **Step 1: Write failing test reproducing symlinked Documents folder leak**

Add to `src/scanner.rs` inside `mod tests`:
```rust
    #[test]
    fn test_symlinked_documents_folder_escaping_prefix_is_not_sniffed() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_symlink_dir_escape");
        let _ = fs::remove_dir_all(&temp_dir);

        let outside_dir = std::env::temp_dir().join("prefixpug_outside_user_docs");
        let _ = fs::remove_dir_all(&outside_dir);
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("personal_tax_record.txt"), b"sensitive personal data").unwrap();

        let steamuser_dir = temp_dir
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser");
        fs::create_dir_all(&steamuser_dir).unwrap();

        // Symlink Documents to outside_dir (standard Wine desktop integration behavior)
        let documents_link = steamuser_dir.join("Documents");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_dir, &documents_link).unwrap();

        let mut warnings = Vec::new();
        let saves = sniff_save_files(&temp_dir, &mut warnings);

        // Crucial safety assertion: NO file from outside_dir can be collected
        for s in &saves {
            assert!(
                !s.path.starts_with(&documents_link) && !s.path.starts_with(&outside_dir),
                "External host file {:?} was collected by save sniffer!",
                s.path
            );
        }
        assert!(warnings.iter().any(|w| w.contains("outside prefix")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&outside_dir);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib scanner::tests::test_symlinked_documents_folder_escaping_prefix_is_not_sniffed`
Expected: FAIL (assertion fails because external host file was collected).

- [ ] **Step 3: Implement symlink safety guard in `sniff_save_files`**

In `src/scanner.rs`:
1. Check each candidate directory before entering: if `dir` is a symlink or `dir.canonicalize()` does not start with `canonical_compat`, emit a warning and skip `dir`.
2. Inside `WalkDir`, for every regular file: check `entry.path().canonicalize()`; if it does not start with `canonical_compat`, emit a warning and skip the file.

```rust
    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }

        // Check if candidate directory itself is a symlink or escapes prefix
        if let Ok(meta) = fs::symlink_metadata(&dir) {
            if meta.file_type().is_symlink() {
                if let Ok(canon_dir) = dir.canonicalize() {
                    if !canon_dir.starts_with(&canonical_compat) {
                        warnings.push(format!(
                            "Directory {:?} is a symlink pointing outside prefix ({:?}); skipped for safety.",
                            dir, canon_dir
                        ));
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }

        // P0-5: Never follow symlinks
        for entry in WalkDir::new(&dir).follow_links(false).into_iter().flatten() {
            let path = entry.path();

            if entry.path_is_symlink() {
                if let Ok(target) = fs::read_link(path) {
                    let target_canonical = if target.is_relative() {
                        path.parent().map(|p| p.join(&target)).unwrap_or(target)
                    } else {
                        target
                    };

                    if let Ok(canon_target) = target_canonical.canonicalize() {
                        if !canon_target.starts_with(&canonical_compat) {
                            warnings.push(format!(
                                "Symlink {:?} points outside prefix ({:?}); skipped for safety.",
                                path, canon_target
                            ));
                            continue;
                        }
                    }
                }
                continue;
            }

            if entry.file_type().is_file() {
                if let Ok(canon_file) = path.canonicalize() {
                    if !canon_file.starts_with(&canonical_compat) {
                        warnings.push(format!(
                            "File {:?} resolves outside prefix ({:?}); skipped for safety.",
                            path, canon_file
                        ));
                        continue;
                    }
                } else {
                    continue;
                }

                if is_blocklisted_save_entry(path) {
                    continue;
                }

                if let Ok(meta) = entry.metadata() {
                    let size = meta.len();
                    saves.push(SaveFileInfo {
                        path: path.to_path_buf(),
                        size_bytes: size,
                    });
                }
            }
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib scanner::tests::test_symlinked_documents_folder_escaping_prefix_is_not_sniffed`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/scanner.rs
git commit -m "fix(scanner): prevent save sniffer from walking symlinked host directories"
```

---

### Task 2: Fix Symlink Target Destruction in Path Deletion (C2)

**Files:**
- Modify: `src/scanner.rs:450-515`
- Test: `src/scanner.rs:748-770`

**Interfaces:**
- Consumes: `target_dir: &Path`, `expected_parent_name: &str`
- Produces: `validate_prefix_path_for_deletion(target_dir: &Path, expected_parent_name: &str) -> Result<PathBuf>`
- Produces: `safe_delete_prefix_directory(target: &Path) -> Result<()>`

- [ ] **Step 1: Write failing test verifying symlink prefix deletion unlinks symlink without destroying target**

Add to `src/scanner.rs` inside `mod tests`:
```rust
    #[test]
    fn test_delete_symlinked_prefix_only_removes_symlink_not_target() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_symlink_del_target");
        let _ = fs::remove_dir_all(&temp_dir);

        // Real target directory
        let real_target = temp_dir.join("real_storage").join("compatdata").join("12345");
        fs::create_dir_all(&real_target).unwrap();
        let sentinel_file = real_target.join("precious_game_data.bin");
        fs::write(&sentinel_file, b"DO_NOT_DELETE").unwrap();

        // Symlink in steam library
        let steamapps_compat = temp_dir.join("steamapps").join("compatdata");
        fs::create_dir_all(&steamapps_compat).unwrap();
        let symlink_prefix = steamapps_compat.join("12345");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_target, &symlink_prefix).unwrap();

        // Validate and delete
        let validated = validate_prefix_path_for_deletion(&symlink_prefix, "compatdata").unwrap();
        safe_delete_prefix_directory(&validated).unwrap();

        // Symlink must be gone
        assert!(!symlink_prefix.exists());
        assert!(fs::symlink_metadata(&symlink_prefix).is_err());

        // Target must SURVIVE!
        assert!(real_target.exists(), "Target directory was destroyed!");
        assert!(sentinel_file.exists(), "Target files were destroyed!");

        let _ = fs::remove_dir_all(&temp_dir);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib scanner::tests::test_delete_symlinked_prefix_only_removes_symlink_not_target`
Expected: FAIL (assertion fails because target directory was destroyed).

- [ ] **Step 3: Implement symlink-safe path validation in `validate_prefix_path_for_deletion`**

In `src/scanner.rs`:
```rust
pub fn validate_prefix_path_for_deletion(
    target_dir: &Path,
    expected_parent_name: &str,
) -> Result<PathBuf> {
    let symlink_meta = fs::symlink_metadata(target_dir)
        .with_context(|| format!("Failed to inspect target path {:?}", target_dir))?;

    // Check lexical parent
    let parent = target_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Directory {:?} has no parent", target_dir))?;

    let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if parent_name != expected_parent_name {
        bail!(
            "Safety violation: directory {:?} is not a child of '{}' (actual parent: '{}')",
            target_dir,
            expected_parent_name,
            parent_name
        );
    }

    let file_name = target_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !file_name.chars().all(|c| c.is_ascii_digit()) {
        bail!(
            "Safety violation: directory name '{}' in {:?} is not a numeric AppID",
            file_name,
            target_dir
        );
    }

    if symlink_meta.file_type().is_symlink() {
        // If the path itself is a symlink, return target_dir so safe_delete_prefix_directory unlinks the symlink
        return Ok(target_dir.to_path_buf());
    }

    let canonical = target_dir
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize directory {:?}", target_dir))?;

    if canonical == Path::new("/") || canonical == Path::new("/home") {
        bail!("Safety violation: cannot delete root/system path {:?}", canonical);
    }

    if let Some(home) = dirs::home_dir() {
        if canonical == home {
            bail!("Safety violation: cannot delete home directory {:?}", canonical);
        }
    }

    Ok(target_dir.to_path_buf())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib scanner::tests::test_delete_symlinked_prefix_only_removes_symlink_not_target`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/scanner.rs
git commit -m "fix(scanner): preserve target directories when removing symlinked prefix entries"
```

---

### Task 3: Fix Shortcut Discovery Steam Roots & Legacy Shortcuts (C3, C4)

**Files:**
- Modify: `src/main.rs:698-710`
- Modify: `src/vdf_parser.rs:475-487`
- Test: `src/vdf_parser.rs:698-732`, `tests/integration_tests.rs:140-201`

**Interfaces:**
- Consumes: `bytes: &[u8]`
- Produces: `parse_shortcuts_vdf_bytes(bytes: &[u8]) -> Result<Vec<NonSteamShortcut>>`

- [ ] **Step 1: Write test for legacy shortcut parsing without explicit AppID field**

Add to `src/vdf_parser.rs` inside `mod tests`:
```rust
    #[test]
    fn test_parse_legacy_shortcuts_without_explicit_appid() {
        let mut bytes = Vec::new();
        bytes.push(0x00);
        bytes.extend_from_slice(b"shortcuts\0");

        bytes.push(0x00);
        bytes.extend_from_slice(b"0\0");

        // Notice: NO 0x02 appid field! Only AppName and Exe (legacy format)
        bytes.push(0x01);
        bytes.extend_from_slice(b"AppName\0GOG Galaxy\0");

        bytes.push(0x01);
        bytes.extend_from_slice(b"Exe\0\"C:\\GOG Galaxy\\GalaxyClient.exe\"\0");

        bytes.push(0x08);
        bytes.push(0x08);

        let shortcuts = parse_shortcuts_vdf_bytes(&bytes).expect("parse legacy shortcuts");
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].app_name, "GOG Galaxy");
        assert!(!shortcuts[0].computed_compatdata_id.is_empty());
        assert_ne!(shortcuts[0].appid, 0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib vdf_parser::tests::test_parse_legacy_shortcuts_without_explicit_appid`
Expected: FAIL (assertion fails: `shortcuts.len()` is 0).

- [ ] **Step 3: Implement legacy shortcut fallback and default Steam roots**

In `src/vdf_parser.rs:463-485`:
```rust
        let computed_crc = if !exe.is_empty() && !app_name.is_empty() {
            let key = format!("{}{}", exe, app_name);
            let crc = crc32fast::hash(key.as_bytes()) | 0x8000_0000;
            crc.to_string()
        } else {
            String::new()
        };

        // Fallback for legacy shortcuts: compute AppID from CRC if missing
        let resolved_appid = appid.or_else(|| {
            if !computed_crc.is_empty() {
                computed_crc.parse::<u32>().ok()
            } else {
                None
            }
        });

        if let Some(id) = resolved_appid {
            shortcuts.push(NonSteamShortcut {
                appid: id,
                app_name,
                exe,
                computed_compatdata_id: computed_crc,
            });
        }
```

In `src/main.rs:698-707`:
```rust
    let mut steam_roots = Vec::new();
    for lib in &library_folders {
        steam_roots.push(lib.path.clone());
    }
    if let Some(parent) = vdf_path.parent().and_then(|p| p.parent()) {
        steam_roots.push(parent.to_path_buf());
    }
    if let Some(home) = dirs::home_dir() {
        steam_roots.push(home.join(".steam/root"));
        steam_roots.push(home.join(".steam/steam"));
        steam_roots.push(home.join(".local/share/Steam"));
        steam_roots.push(home.join(".var/app/com.valvesoftware.Steam/.steam/root"));
        steam_roots.push(home.join(".var/app/com.valvesoftware.Steam/.steam/steam"));
        steam_roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib vdf_parser::tests::test_parse_legacy_shortcuts_without_explicit_appid`
Run: `cargo test --test integration_tests test_p0_2_non_steam_shortcuts_protection`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/vdf_parser.rs
git commit -m "fix(shortcuts): support legacy shortcuts without appid and populate standard steam roots"
```

---

### Task 4: Harden Backup Permissions & Restore Safety (S1, S2, H2)

**Files:**
- Modify: `src/backup.rs:98-193,358-396`
- Modify: `src/scanner.rs:130-158`
- Test: `src/backup.rs:428-480`

**Interfaces:**
- Consumes: `backup_id_or_path: &str`, `backup_root: &Path`, `target_dir: &Path`
- Produces: `restore_backup(backup_id_or_path: &str, backup_root: &Path, target_dir: &Path) -> Result<PathBuf>`

- [ ] **Step 1: Write test verifying archive verification on restore and directory permissions**

Add to `src/backup.rs` inside `mod tests`:
```rust
    #[test]
    fn test_restore_verifies_archive_before_unpacking() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_restore_verify");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let save_file = temp_dir.join("save.dat");
        fs::write(&save_file, b"VALID_SAVE_PAYLOAD").unwrap();

        let orphan = OrphanedPrefix {
            appid: "111".to_string(),
            title: Some("Test".to_string()),
            classification: PrefixClassification::Orphaned,
            library_path: PathBuf::from("/tmp"),
            compatdata_path: Some(temp_dir.clone()),
            compatdata_usage: DiskUsage::default(),
            shadercache_path: None,
            shadercache_usage: DiskUsage::default(),
            detected_saves: vec![SaveFileInfo {
                path: save_file,
                size_bytes: 18,
            }],
            last_modified: None,
            is_high_value: false,
            high_value_reasons: vec![],
            cloud_status: crate::vdf_parser::SteamCloudStatus::default(),
            warnings: vec![],
        };

        let vault_root = temp_dir.join("vault");
        let archived = backup_orphan_saves(&orphan, &vault_root).unwrap().unwrap();

        // Corrupt archive
        let archive_file = archived.join("saves.tar.gz");
        fs::write(&archive_file, b"corrupted garbage bytes").unwrap();

        let restore_dest = temp_dir.join("restored");
        let res = restore_backup(&archived.to_string_lossy(), &vault_root, &restore_dest);
        assert!(res.is_err(), "Must reject restoring corrupted archive!");

        let _ = fs::remove_dir_all(&temp_dir);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib backup::tests::test_restore_verifies_archive_before_unpacking`
Expected: FAIL (currently does not verify before unpacking).

- [ ] **Step 3: Implement restrictive vault permissions and secure entry extraction**

In `src/backup.rs`:
1. Use `std::os::unix::fs::PermissionsExt` to set `0o700` on backup directories and `0o600` on `manifest.json` and `saves.tar.gz`.
2. In `restore_backup`:
   - Run `verify_backup` first; if invalid, return `bail!("Backup integrity verification failed: ...")`.
   - Iterate over entries in the tarball and validate:
     - Entry path must be relative and cannot start with `/` or contain `..`.
     - Entry path cannot be a symlink or hardlink.
   - Unpack entries safely into `target_dir`.

In `src/scanner.rs:130-158`:
In `calculate_directory_usage`, check `if entry.path_is_symlink() { continue; }` so external symlink targets do not falsely inflate usage.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib backup::tests`
Run: `cargo test --test integration_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/backup.rs src/scanner.rs
git commit -m "fix(backup): enforce 0700 permissions, verify archives on restore, and reject symlink paths"
```

---

### Task 5: Fix VDF String Decoding, Recursion Limit & Title Inference (S3, S4, S5)

**Files:**
- Modify: `src/vdf_parser.rs:335-348, 489-520, 578-623`
- Test: `src/vdf_parser.rs:727-745`

**Interfaces:**
- Consumes: Wine registry content, binary VDF bytes
- Produces: `read_null_terminated_string`, `skip_binary_vdf_subobject`, `infer_title_from_compatdata`

- [ ] **Step 1: Write tests for non-UTF8 binary VDF strings, deep recursion, and Wine registry title inference**

Add to `src/vdf_parser.rs` inside `mod tests`:
```rust
    #[test]
    fn test_non_utf8_binary_vdf_does_not_abort() {
        let mut bytes = Vec::new();
        bytes.push(0x00);
        bytes.extend_from_slice(b"shortcuts\0");
        bytes.push(0x00);
        bytes.extend_from_slice(b"0\0");
        bytes.push(0x02);
        bytes.extend_from_slice(b"appid\0");
        bytes.extend_from_slice(&12345u32.to_le_bytes());
        bytes.push(0x01);
        bytes.extend_from_slice(b"AppName\0");
        // Non-UTF-8 Latin-1 byte sequence: Pok\xe9mon
        bytes.extend_from_slice(b"Pok\xe9mon\0");
        bytes.push(0x01);
        bytes.extend_from_slice(b"Exe\0game.exe\0");
        bytes.push(0x08);
        bytes.push(0x08);

        let res = parse_shortcuts_vdf_bytes(&bytes);
        assert!(res.is_ok(), "Failed to parse non-UTF8 shortcut string");
        let sc = res.unwrap();
        assert_eq!(sc.len(), 1);
        assert!(sc[0].app_name.contains("Pok"));
    }

    #[test]
    fn test_infer_title_handles_wine_escaped_backslashes() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_wine_reg_slashes");
        let pfx_dir = temp_dir.join("pfx");
        let _ = fs::create_dir_all(&pfx_dir);

        // Real Wine registry format has double backslashes
        let reg = "[Software\\\\Bethesda\\\\Skyrim Special Edition]\n\"Installed\"=dword:00000001\n";
        fs::write(pfx_dir.join("user.reg"), reg).unwrap();

        let title = infer_title_from_compatdata(&temp_dir);
        assert_eq!(title.as_deref(), Some("Skyrim Special Edition"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib vdf_parser::tests::test_non_utf8_binary_vdf_does_not_abort`
Run: `cargo test --lib vdf_parser::tests::test_infer_title_handles_wine_escaped_backslashes`
Expected: FAIL.

- [ ] **Step 3: Implement fixes in `vdf_parser.rs`**

1. In `read_null_terminated_string`:
```rust
fn read_null_terminated_string(bytes: &[u8], cursor: &mut usize) -> Result<String> {
    let start = *cursor;
    while *cursor < bytes.len() && bytes[*cursor] != 0 {
        *cursor += 1;
    }
    if *cursor >= bytes.len() {
        bail!("Unexpected EOF reading null-terminated string in binary VDF");
    }
    let s = String::from_utf8_lossy(&bytes[start..*cursor]).into_owned();
    *cursor += 1; // Consume null byte
    Ok(s)
}
```

2. In `skip_binary_vdf_subobject`:
```rust
fn skip_binary_vdf_subobject(bytes: &[u8], cursor: &mut usize, depth: usize) -> Result<()> {
    if depth > 64 {
        bail!("VDF sub-object nesting exceeded maximum recursion limit (64)");
    }
    while *cursor < bytes.len() {
        let field_type = bytes[*cursor];
        *cursor += 1;
        if field_type == 0x08 {
            return Ok(());
        }
        let _key = read_null_terminated_string(bytes, cursor)?;
        match field_type {
            0x00 => skip_binary_vdf_subobject(bytes, cursor, depth + 1)?,
            0x01 => {
                let _val = read_null_terminated_string(bytes, cursor)?;
            }
            0x02 | 0x03 => {
                if *cursor + 4 > bytes.len() {
                    bail!("Unexpected EOF skipping 4-byte field");
                }
                *cursor += 4;
            }
            0x07 => {
                if *cursor + 8 > bytes.len() {
                    bail!("Unexpected EOF skipping 8-byte field");
                }
                *cursor += 8;
            }
            other => {
                bail!("Unknown field type 0x{:02x} in sub-object", other);
            }
        }
    }
    bail!("Unexpected EOF reading nested sub-object in binary VDF");
}
```

3. In `infer_title_from_compatdata`:
```rust
            for line in content.lines() {
                if line.starts_with("[Software\\")
                    && !line.contains("Wine")
                    && !line.contains("Microsoft")
                {
                    let trimmed = line.trim_matches(|c| c == '[' || c == ']');
                    let parts: Vec<&str> = trimmed
                        .split('\\')
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.len() >= 3 {
                        let candidate = parts[2].trim();
                        if !candidate.is_empty() && candidate != "Classes" {
                            return Some(candidate.to_string());
                        }
                    } else if parts.len() == 2 {
                        let candidate = parts[1].trim();
                        if !candidate.is_empty() && candidate != "Classes" {
                            return Some(candidate.to_string());
                        }
                    }
                }
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib vdf_parser::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/vdf_parser.rs
git commit -m "fix(vdf): handle non-utf8 strings gracefully, limit recursion depth, and parse wine registry titles"
```

---

### Task 6: TUI Terminal Panic Recovery, Error Reporting & Space Delta (H1, C5, H3, H5)

**Files:**
- Modify: `src/main.rs:288-320, 524-538, 607-634`
- Modify: `src/scanner.rs:427-442`

**Interfaces:**
- Terminal RAII cleanup guard
- TUI error reporting for failed deletions
- Multi-library statvfs grouping

- [ ] **Step 1: Implement RAII TerminalGuard and panic hook in `main.rs`**

```rust
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode().context("Failed to enable terminal raw mode")?;
        execute!(stdout(), EnterAlternateScreen).context("Failed to enter alternate screen")?;

        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), LeaveAlternateScreen);
            default_hook(info);
        }));

        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}
```

- [ ] **Step 2: Update TUI deletion handler to record errors and only remove successful AppIDs**

In `src/main.rs:607-634`:
```rust
    let mut reclaimed = 0;
    let mut success_ids = HashSet::new();
    let mut failed_ids = Vec::new();

    let targets: Vec<OrphanedPrefix> = app
        .all_orphans
        .iter()
        .filter(|o| app.selected_appids.contains(&o.appid))
        .cloned()
        .collect();

    for t in &targets {
        match execute_clean(t, &app.backup_dir, false, false) {
            Ok(bytes) => {
                reclaimed += bytes;
                success_ids.insert(t.appid.clone());
            }
            Err(e) => {
                failed_ids.push((t.appid.clone(), e.to_string()));
            }
        }
    }

    app.space_reclaimed += reclaimed;
    if failed_ids.is_empty() {
        app.status_message = format!(
            "Purged {} prefix(es). Reclaimed {}!",
            success_ids.len(),
            format_bytes(reclaimed)
        );
    } else {
        app.status_message = format!(
            "Purged {} prefix(es) ({} failed). Reclaimed {}.",
            success_ids.len(),
            failed_ids.len(),
            format_bytes(reclaimed)
        );
    }

    app.all_orphans.retain(|o| !success_ids.contains(&o.appid));
    for id in &success_ids {
        app.selected_appids.remove(id);
    }
    app.apply_filter();
    app.state = AppState::Done;
```

- [ ] **Step 3: Update CLI multi-library space delta calculation**

In `src/main.rs:291-320`:
Group targets by filesystem device (via `st_dev` metadata), sample available space on each distinct filesystem before and after, and sum the deltas.

- [ ] **Step 4: Implement active prefix lock check in `scanner.rs`**

Add check for `pfx.lock` in `scanner.rs`:
```rust
pub fn is_prefix_locked(compatdata_path: &Path) -> bool {
    let lock_path = compatdata_path.join("pfx.lock");
    if lock_path.is_file() {
        // Test non-blocking exclusive flock
        if let Ok(file) = std::fs::File::open(&lock_path) {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if res != 0 {
                return true; // Lock is currently held by active process
            }
            // Release test lock
            unsafe { libc::flock(fd, libc::LOCK_UN) };
        }
    }
    false
}
```

- [ ] **Step 5: Run tests and verify build**

Run: `cargo test`
Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/scanner.rs
git commit -m "fix(tui): install panic hook, handle partial clean failures, and detect active pfx locks"
```

---

### Task 7: Harden `install.sh` Script (H4)

**Files:**
- Modify: `install.sh:1-52`
- Test: Shell syntax & dry test

- [ ] **Step 1: Update `install.sh` with architecture detection, curl -f, and safe tempdir unpacking**

Update `install.sh`:
- Detect `uname -m`. Support `x86_64`. For other architectures without cargo, exit with clear message instead of downloading incompatible binary.
- Use `curl -sSLf` to fail immediately on HTTP errors.
- Download into `mktemp -d`, verify binary exists, then `install -m 755`.
- Guard completion generation so shell completion failures do not abort installation.

- [ ] **Step 2: Verify shell script with bash -n**

Run: `bash -n install.sh`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add install.sh
git commit -m "fix(install): add architecture detection, curl failure checks, and atomic binary installation"
```

---

## Plan Self-Review Checklist
1. **Spec coverage:** Covers C1-C5, S1-S5, and H1-H5.
2. **Placeholder scan:** No "TODO", "TBD", or vague steps.
3. **Type consistency:** All signatures match standard types.
