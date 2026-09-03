# PrefixPug — Hardening & Correctness Tasks

**Repo:** `prefixpug` (Rust, ratatui TUI + clap CLI)
**Purpose of this doc:** hand-off spec for an implementation agent. Tasks are ordered by severity. P0 items are data-loss bugs and must be fixed before the tool is recommended to anyone.

**Prime directive for all work below:** the tool's headline claim is *safety*. Any ambiguity must resolve toward "keep the data." Deleting a live prefix or silently missing a save is a total product failure; leaving a few GiB unreclaimed is not.

---

## P0 — Data-loss risks

### P0-1. Resolve every Steam library before declaring an orphan

**Problem:** if the scanner only inspects the default library, `compatdata` directories belonging to games installed on other drives can be classified as orphaned and deleted.

**Required:**
- Parse `~/.steam/steam/steamapps/libraryfolders.vdf` (also check `~/.local/share/Steam/`, `~/.var/app/com.valvesoftware.Steam/` for Flatpak, and the Steam Deck paths) and enumerate **all** library roots.
- Build the set of live app IDs from `appmanifest_*.acf` across **all** libraries, not just the one containing the `compatdata` dir under inspection.
- If any configured library root is unreachable at scan time (unmounted external drive, missing path), **abort the purge** with a clear error. Do not treat an unreadable library as an empty one. This is the single most likely way to nuke a live prefix.
- Surface the resolved library list in the TUI and in `--json` output so the user can sanity-check it.

**Acceptance:** integration test with two library roots where the game is installed in library B and its prefix lives in library B. Test that a scan started from library A does not flag it. Second test: mark library B unreadable, assert the tool refuses to purge rather than flagging everything.

### P0-2. Honor non-Steam game shortcuts

**Problem:** non-Steam shortcuts (Battle.net, EGS, itch, emulators, Lutris entries added to Steam) get their own `compatdata` prefixes but have **no `appmanifest_*.acf`**. Under the current orphan rule every one of them is a false positive. This class of user is common and their prefixes are hand-configured.

**Required:**
- Parse `userdata/<steamid3>/config/shortcuts.vdf` (binary VDF) for **every** user directory under `userdata/`, not just the first.
- Read the `appid` field directly from each shortcut entry and map it to its unsigned 32-bit `compatdata` directory name. Do **not** rely solely on a computed `crc32(exe + appname) | 0x80000000` formula. Verify the mapping empirically against a real install with at least one non-Steam shortcut, and document what you observed in a code comment.
- If `shortcuts.vdf` exists but cannot be parsed, **abort the purge**. Do not proceed on a partial view of protected IDs.
- If `shortcuts.vdf` is absent for a user, that is fine (no shortcuts configured); log it and continue.

**Acceptance:** fixture with a `shortcuts.vdf` containing two entries; assert both corresponding `compatdata` dirs are excluded from the orphan set. Corrupt the fixture; assert the tool errors out instead of flagging them.

### P0-3. Protect Steam infrastructure app IDs

**Problem:** Proton builds and the Steam Linux Runtime create their own `compatdata`/tool directories. Deleting them breaks the whole gaming stack.

**Required:**
- Primary defense: infrastructure tools *do* ship `appmanifest` files in whichever library they were installed to, so P0-1 (full library enumeration) should cover most of it. Verify this holds.
- Backstop: maintain an explicit deny-list of known runtime/compat-tool app IDs (Proton releases, Proton Experimental, Proton Hotfix, Steam Linux Runtime soldier/sniper/scout) and never offer them for deletion. Treat the list as a safety net, not the primary mechanism, and note in a comment that it will go stale.
- Any app ID the tool cannot positively classify goes into an **"unknown — not offered for deletion"** bucket, visible in the TUI but not selectable. Unknown must never default to deletable.

### P0-4. Invert the save-detection logic (allowlist → blocklist)

**Problem:** the current extension allowlist (`.sav`, `.save`, `.ess`, `.fos`, `.skse`, `.dat`, `.sqlite`, `.db`) misses a large fraction of real save formats: extensionless files, `.json`, `.xml`, `.bin`, `.slot`, `.profile`, `.cfg`, numbered files, and per-engine oddities. The current design reports "saves backed up" while the actual save is destroyed. This is fail-silent, which is the worst possible failure mode for a tool whose pitch is safety.

**Required:**
- Archive the **entire contents** of each save root, minus a blocklist:
  - `drive_c/users/steamuser/Saved Games/`
  - `drive_c/users/steamuser/Documents/` (incl. `My Games/`)
  - `drive_c/users/steamuser/AppData/Roaming/`
  - `drive_c/users/steamuser/AppData/Local/`
  - `drive_c/users/steamuser/AppData/LocalLow/`
  - `drive_c/ProgramData/` (some titles save here)
  - Legacy Wine aliases: `Application Data`, `Local Settings/Application Data`, `My Documents`
- Blocklist known junk only: crash dumps (`*.dmp`, `CrashDumps/`), log files, `Microsoft/`, `Temp/`, browser/CEF caches, `NVIDIA/`, `AMD/`, DXVK/VKD3D state caches, Wine's own `Mono`/`Gecko` dirs.
- Add a hard size guard: if the resulting archive for a single prefix would exceed a configurable cap (default ~2 GiB), do not silently truncate. Warn, list the largest contributors, and require explicit user confirmation to proceed or skip.
- Rationale: a few hundred extra MiB of tarball is free. A lost save is not.

**Acceptance:** fixture prefix containing (a) an extensionless save file, (b) a `.json` save, (c) a 40 MiB crash dump, (d) a shader state cache. Assert (a) and (b) are in the archive and (c) and (d) are not.

### P0-5. Symlink and path-traversal safety

**Problem:** Wine prefixes contain symlinks, some pointing outside the prefix into the real `$HOME`. Walking or deleting through them can archive private user data or delete files far outside the target.

**Required:**
- Never follow symlinks during the save scan. If a save root is itself a symlink pointing outside the prefix, skip it and record a warning in the manifest.
- Never delete through a symlink. Remove the link, never the target.
- Before any destructive operation, canonicalize the target path and assert it is a strict descendant of a resolved `steamapps/compatdata/` or `steamapps/shadercache/` directory. Reject paths containing `..`, reject `/`, `$HOME`, and any library root itself.
- Prefer `openat`-style directory-handle traversal (`cap-std` or equivalent) over path-string recursion so the check cannot be raced (TOCTOU) between validation and deletion.

**Acceptance:** fixture with a symlink from `drive_c/users/steamuser/Documents` to a sentinel directory outside the prefix; assert the sentinel is neither archived nor deleted.

### P0-6. Refuse to operate while Steam is running

**Required:** detect a running Steam process and/or a held `pfx.lock` in a target prefix and abort with a clear message. A game or an in-progress install can be actively writing to a prefix the tool considers dead.

---

## P1 — Correctness, reporting, and workflow

### P1-1. Report honest reclaimed sizes on btrfs / compressed filesystems
Apparent size (`du`-style summed file lengths) overstates what is actually freed on a compressed or CoW filesystem. The primary dev machine runs btrfs with zstd, so the numbers in the README are probably wrong.
- Compute allocated size from `st_blocks * 512`, not file length.
- Show both apparent and on-disk size where they differ materially.
- Take a `statvfs` reading before and after the purge and report the **measured** free-space delta as the authoritative "reclaimed" number.

### P1-2. Add last-access/modification age as a first-class signal
Orphan status alone is a weak signal. A prefix untouched for eighteen months is far more actionable, and an orphan touched last week is suspicious. Track the newest mtime within each prefix, display it as a column, sort by it, and support `--older-than <duration>`.

### P1-3. Split shader caches into their own low-risk mode
Shader caches are regenerable and contain nothing irreplaceable. Give them a separate command/mode with a lighter confirmation path, and let users clean them without going anywhere near prefix deletion. This is the safest and most frequently useful thing the tool does.

### P1-4. Make the tool scriptable and safe by default
- `--dry-run` is the **default** for any destructive path; require `--yes`/`--purge` to actually delete.
- `--json` output for scan results (schema-stable, documented).
- Meaningful exit codes: 0 success, non-zero distinguishing "nothing to do", "aborted by user", "unsafe state detected", "partial failure".
- No interactive-only functionality; everything reachable in the TUI must be reachable headlessly.

### P1-5. Make backups verifiable and restorable
- `manifest.json` must record: app ID, resolved game name and its source, Proton version from the prefix, absolute source paths, per-file size and hash, archive checksum, tool version, timestamp, and any warnings raised (skipped symlinks, size-cap hits).
- Add `prefixpug verify-backup <archive>` to check the archive against its manifest.
- Add a round-trip integration test: create prefix → back up → purge → restore → assert byte-identical tree.
- Write the archive and **fsync it** before deleting anything. Verify the archive is readable before the first `unlink`. Never delete on the assumption that the write succeeded.

### P1-6. Warn on high-value prefixes
Prefixes with evidence of manual investment (installed via protontricks, non-default `user.reg` entries, mod loader files such as SKSE/F4SE, large `drive_c/Program Files` payloads) should be flagged as expensive to rebuild and require a distinct extra confirmation. Reinstalling the game does not restore this work.

---

## P2 — Credibility and presentation

### P2-1. Fix or remove the broken badges
The README currently shows `CI: repo or workflow not found` and `RELEASE: no releases or repo not found` directly under a hero banner. That combination reads as vaporware to exactly the audience being targeted. Either get CI green and cut a real tagged release, or remove the badges until they are true.

### P2-2. Align README claims with reality
- Drop "high-performance" as a headline claim. The workload is I/O-bound directory traversal; the phrase invites skepticism and buys nothing. "Fast" or nothing.
- Lead the README with the safety engineering (multi-library resolution, shortcuts.vdf handling, blocklist-based save vaulting, verified backups), because that is the actual differentiator versus a shell one-liner.
- State the realistic value up front: typical reclaim is single-digit to low-double-digit GiB. Do not imply more.
- Keep the branding, but let it sit below substance rather than above it.

### P2-3. Working CI
`.github/workflows/ci.yml` should run `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and the sandbox integration suite on stable Rust. `release.yml` should produce tagged binaries. Add `cargo deny` or `cargo audit` if cheap.

---

## Testing requirements

Build a fixture generator (`tests/fixtures/`) that constructs a synthetic Steam tree on disk. It must be able to produce, in combination:

1. Two or more library roots, one of them optionally unreadable
2. Installed games with valid `appmanifest_*.acf`
3. Genuinely orphaned prefixes
4. Non-Steam shortcut prefixes backed by a `shortcuts.vdf`
5. Proton/runtime tool directories
6. A prefix containing an extensionless save, a `.json` save, a crash dump, and a shader state cache
7. A prefix with a symlink escaping to an outside sentinel directory
8. A prefix with an active `pfx.lock`

Every P0 item above needs at least one test that **fails before the fix**. Add a standing canary test asserting that a save file with no recognized extension survives a full purge-and-restore cycle.

---

## Definition of done

- [ ] No code path can delete outside a validated `compatdata`/`shadercache` descendant
- [ ] Unreadable library, unparseable `shortcuts.vdf`, or running Steam each abort the purge
- [ ] Unclassifiable app IDs are non-selectable, never default-deletable
- [ ] Save vaulting is blocklist-based; extensionless and `.json` saves are captured
- [ ] Symlinks are never followed or deleted through
- [ ] Backup archives are fsynced and verified before any deletion; restore round-trip is tested
- [ ] Reclaimed-space figures come from a measured `statvfs` delta
- [ ] `--dry-run` is default; `--json` and exit codes work headlessly
- [ ] CI green on a real workflow; badges reflect reality
- [ ] README claims match what the code actually guarantees
