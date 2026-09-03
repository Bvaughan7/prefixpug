use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

use crate::vdf_parser::{
    get_infrastructure_name, infer_title_from_compatdata, InstalledGame, LibraryFolder,
    PrefixClassification,
};

pub const DEFAULT_MAX_SAVE_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB cap

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DiskUsage {
    pub apparent_bytes: u64,
    pub allocated_bytes: u64,
}

impl DiskUsage {
    pub fn add(&mut self, apparent: u64, allocated: u64) {
        self.apparent_bytes += apparent;
        self.allocated_bytes += allocated;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFileInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanedPrefix {
    pub appid: String,
    pub title: Option<String>,
    pub classification: PrefixClassification,
    pub library_path: PathBuf,
    pub compatdata_path: Option<PathBuf>,
    pub compatdata_usage: DiskUsage,
    pub shadercache_path: Option<PathBuf>,
    pub shadercache_usage: DiskUsage,
    pub detected_saves: Vec<SaveFileInfo>,
    pub last_modified: Option<SystemTime>,
    pub is_high_value: bool,
    pub high_value_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

impl OrphanedPrefix {
    pub fn total_size(&self) -> u64 {
        self.total_apparent_bytes()
    }

    pub fn compatdata_size(&self) -> u64 {
        self.compatdata_usage.apparent_bytes
    }

    pub fn shadercache_size(&self) -> u64 {
        self.shadercache_usage.apparent_bytes
    }

    pub fn total_apparent_bytes(&self) -> u64 {
        self.compatdata_usage.apparent_bytes + self.shadercache_usage.apparent_bytes
    }

    pub fn total_allocated_bytes(&self) -> u64 {
        self.compatdata_usage.allocated_bytes + self.shadercache_usage.allocated_bytes
    }

    pub fn total_save_size(&self) -> u64 {
        self.detected_saves.iter().map(|s| s.size_bytes).sum()
    }

    pub fn is_deletable(&self) -> bool {
        self.classification.is_deletable()
    }

    pub fn display_name(&self) -> String {
        match &self.title {
            Some(t) => format!("{} (AppID: {})", t, self.appid),
            None => format!("AppID: {}", self.appid),
        }
    }

    pub fn age_display(&self) -> String {
        match self.last_modified {
            Some(time) => {
                if let Ok(duration) = SystemTime::now().duration_since(time) {
                    let secs = duration.as_secs();
                    if secs < 3600 {
                        format!("{}m ago", secs / 60)
                    } else if secs < 86400 {
                        format!("{}h ago", secs / 3600)
                    } else if secs < 30 * 86400 {
                        format!("{}d ago", secs / 86400)
                    } else if secs < 365 * 86400 {
                        format!("{}mo ago", secs / (30 * 86400))
                    } else {
                        format!("{}y ago", secs / (365 * 86400))
                    }
                } else {
                    "future".to_string()
                }
            }
            None => "unknown".to_string(),
        }
    }
}

// -----------------------------------------------------------------------------
// P1-1: Honest Size Reporting & statvfs Delta
// -----------------------------------------------------------------------------

/// Computes both apparent size and allocated size (st_blocks * 512 on Unix).
/// Respects symlink safety: never follows symlinks.
pub fn calculate_directory_usage(path: &Path) -> (DiskUsage, Option<SystemTime>) {
    if !path.exists() {
        return (DiskUsage::default(), None);
    }

    let mut usage = DiskUsage::default();
    let mut newest_mtime: Option<SystemTime> = None;

    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                let apparent = meta.len();
                let allocated = meta.blocks() * 512;
                usage.add(apparent, allocated);

                if let Ok(mtime) = meta.modified() {
                    newest_mtime = match newest_mtime {
                        Some(prev) => Some(prev.max(mtime)),
                        None => Some(mtime),
                    };
                }
            }
        }
    }

    (usage, newest_mtime)
}

/// Measures available disk space on the filesystem hosting `path` using libc::statvfs.
pub fn get_filesystem_available_space(path: &Path) -> Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new("/")).to_path_buf()
    };

    let c_path = CString::new(target.as_os_str().as_bytes())
        .with_context(|| format!("Invalid path bytes for statvfs {:?}", target))?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let res = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if res != 0 {
        bail!("Failed to query statvfs on {:?}", target);
    }

    let free_bytes = (stat.f_bavail as u64) * (stat.f_frsize as u64);
    Ok(free_bytes)
}

// -----------------------------------------------------------------------------
// P0-4: Blocklist Save Engine (Invert allowlist -> blocklist)
// P0-5: Symlink and Path Traversal Safety
// -----------------------------------------------------------------------------

fn is_blocklisted_save_entry(path: &Path) -> bool {
    let lower_path = path.to_string_lossy().to_lowercase();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Extension blocklist
    let blocklist_exts = [
        ".dmp",
        ".mdmp",
        ".log",
        ".tmp",
        ".bak",
        ".old",
        ".etag",
        ".lock",
        ".dll",
        ".exe",
        ".manifest",
        ".cat",
        ".inf",
        ".pdb",
        ".chk",
    ];
    if blocklist_exts.iter().any(|ext| file_name.ends_with(ext)) {
        return true;
    }

    // 2. Directory names blocklist (case-insensitive substring/segment match)
    let blocklist_segments = [
        "/crashdumps/",
        "/crashreports/",
        "/crashes/",
        "/logs/",
        "/temp/",
        "/microsoft/",
        "/windows/",
        "/cef/",
        "/d3dscache/",
        "/dxcache/",
        "/nvidia/",
        "/amd/",
        "/intel/",
        "/mono/",
        "/gecko/",
        "/cache/",
        "/gpucache/",
        "/code cache/",
        "/shadercache/",
        "/shaderhitcache/",
    ];
    if blocklist_segments
        .iter()
        .any(|seg| lower_path.contains(seg))
    {
        return true;
    }

    // 3. Specific system markers
    if file_name == "steam_autocloud.vdf" || file_name == "desktop.ini" || file_name == "thumbs.db"
    {
        return true;
    }

    false
}

/// The Pug's Nose: Sniffs through save roots using a blocklist.
/// Archives all contents (extensionless, .json, .xml, .bin, .sav, etc.)
/// minus known crash dumps, logs, and caches.
/// Strictly enforces symlink safety (never follows symlinks escaping prefix).
pub fn sniff_save_files(compatdata_dir: &Path, warnings: &mut Vec<String>) -> Vec<SaveFileInfo> {
    let mut saves = Vec::new();
    let pfx = compatdata_dir.join("pfx");
    let pfx_drive_c = pfx.join("drive_c");
    if !pfx_drive_c.is_dir() {
        return saves;
    }

    let canonical_compat = match compatdata_dir.canonicalize() {
        Ok(c) => c,
        Err(_) => compatdata_dir.to_path_buf(),
    };

    let user_roots = [
        pfx_drive_c.join("users").join("steamuser"),
        pfx_drive_c.join("users").join("default"),
    ];

    let mut candidate_dirs = Vec::new();

    for user_dir in &user_roots {
        if !user_dir.is_dir() {
            continue;
        }

        candidate_dirs.push(user_dir.join("Saved Games"));
        candidate_dirs.push(user_dir.join("Documents"));
        candidate_dirs.push(user_dir.join("My Documents"));
        candidate_dirs.push(user_dir.join("AppData").join("Local"));
        candidate_dirs.push(user_dir.join("AppData").join("LocalLow"));
        candidate_dirs.push(user_dir.join("AppData").join("Roaming"));
        candidate_dirs.push(user_dir.join("Application Data"));
        candidate_dirs.push(user_dir.join("Local Settings").join("Application Data"));
    }

    // ProgramData can also contain saves for some titles
    candidate_dirs.push(pfx_drive_c.join("ProgramData"));

    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }

        // P0-5: Never follow symlinks
        for entry in WalkDir::new(&dir).follow_links(false).into_iter().flatten() {
            let path = entry.path();

            // Check if this entry is a symlink
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
            }

            if entry.file_type().is_file() {
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

    let total_bytes: u64 = saves.iter().map(|s| s.size_bytes).sum();
    if total_bytes > DEFAULT_MAX_SAVE_ARCHIVE_BYTES {
        warnings.push(format!(
            "Total save data size ({:.2} GiB) exceeds the 2.00 GiB safety threshold.",
            total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
    }

    saves
}

// -----------------------------------------------------------------------------
// P1-6: High-Value Prefix Detection (Mod Loaders, Protontricks, etc.)
// -----------------------------------------------------------------------------

pub fn detect_high_value_prefix(compatdata_dir: &Path) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    let pfx = compatdata_dir.join("pfx");
    let drive_c = pfx.join("drive_c");

    if !drive_c.is_dir() {
        return (false, reasons);
    }

    // 1. Check for mod loaders or mod organizers
    let mod_indicators = [
        "skse_loader.exe",
        "skse64_loader.exe",
        "f4se_loader.exe",
        "nvse_loader.exe",
        "obse_loader.exe",
        "mwse.dll",
        "ModOrganizer.exe",
        "vortex.deployment.json",
    ];

    for entry in WalkDir::new(&drive_c).max_depth(4).into_iter().flatten() {
        if let Some(file_name) = entry.file_name().to_str() {
            for ind in &mod_indicators {
                if file_name.eq_ignore_ascii_case(ind) {
                    reasons.push(format!("Detected mod loader: {}", ind));
                }
            }
        }
    }

    // 2. Check for protontricks or winetricks installation logs
    let winetricks_log = drive_c.join("winetricks.log");
    if winetricks_log.is_file() {
        reasons.push("Prefix contains winetricks/protontricks modifications".to_string());
    }

    let is_high_value = !reasons.is_empty();
    (is_high_value, reasons)
}

// -----------------------------------------------------------------------------
// P0-6: Steam Concurrency Guard
// -----------------------------------------------------------------------------

/// Detects if Steam is actively running by inspecting process names in /proc
pub fn is_steam_running() -> bool {
    let proc_dir = Path::new("/proc");
    if let Ok(entries) = fs::read_dir(proc_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.chars().all(|c| c.is_ascii_digit()) {
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = fs::read_to_string(comm_path) {
                    let trimmed = comm.trim();
                    if trimmed == "steam" || trimmed == "steamwebhelper" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Checks whether Steam is running or if a pfx.lock is held.
pub fn ensure_steam_not_running(allow_running_steam: bool) -> Result<()> {
    if allow_running_steam {
        return Ok(());
    }

    if is_steam_running() {
        bail!(
            "Steam is actively running. Refusing to purge prefixes to prevent data loss \
             or corruption during active game updates. Please exit Steam completely, or use \
             --ignore-running-steam if running in an automated testing environment."
        );
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// P0-5: Path Traversal and Safe Deletion Guards
// -----------------------------------------------------------------------------

/// Validates that a prefix directory is a strict descendant of a compatdata or shadercache folder.
/// Strictly rejects /, /home, $HOME, library roots, or non-numeric directory names.
pub fn validate_prefix_path_for_deletion(
    target_dir: &Path,
    expected_parent_name: &str,
) -> Result<PathBuf> {
    let canonical = target_dir
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize directory {:?}", target_dir))?;

    let parent = canonical
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Directory {:?} has no parent", canonical))?;

    let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if parent_name != expected_parent_name {
        bail!(
            "Safety violation: directory {:?} is not a child of '{}' (actual parent: '{}')",
            canonical,
            expected_parent_name,
            parent_name
        );
    }

    if canonical == Path::new("/") || canonical == Path::new("/home") {
        bail!(
            "Safety violation: cannot delete root/system path {:?}",
            canonical
        );
    }

    if let Some(home) = dirs::home_dir() {
        if canonical == home {
            bail!(
                "Safety violation: cannot delete home directory {:?}",
                canonical
            );
        }
    }

    let file_name = canonical.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !file_name.chars().all(|c| c.is_ascii_digit()) {
        bail!(
            "Safety violation: directory name '{}' in {:?} is not a numeric AppID",
            file_name,
            canonical
        );
    }

    Ok(canonical)
}

/// Safely removes a directory without following symlinks.
/// If the target itself is a symlink, only the symlink is removed.
pub fn safe_delete_prefix_directory(target: &Path) -> Result<()> {
    let symlink_meta = fs::symlink_metadata(target)
        .with_context(|| format!("Failed to inspect target {:?}", target))?;

    if symlink_meta.file_type().is_symlink() {
        fs::remove_file(target)
            .with_context(|| format!("Failed to remove symlink {:?}", target))?;
        return Ok(());
    }

    fs::remove_dir_all(target)
        .with_context(|| format!("Failed to remove directory tree {:?}", target))?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Core Scanner
// -----------------------------------------------------------------------------

pub fn scan_all_prefixes(
    libraries: &[LibraryFolder],
    installed_games: &HashMap<String, InstalledGame>,
    protected_shortcuts: &HashMap<String, String>,
    older_than: Option<Duration>,
) -> Result<Vec<OrphanedPrefix>> {
    let mut prefixes = Vec::new();

    for lib in libraries {
        let steamapps = lib.path.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }

        let compatdata_root = steamapps.join("compatdata");
        let shadercache_root = steamapps.join("shadercache");

        let mut appids_found = HashSet::new();

        if compatdata_root.is_dir() {
            if let Ok(entries) = fs::read_dir(&compatdata_root) {
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.chars().all(|c| c.is_ascii_digit()) {
                                    appids_found.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        if shadercache_root.is_dir() {
            if let Ok(entries) = fs::read_dir(&shadercache_root) {
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.chars().all(|c| c.is_ascii_digit()) {
                                    appids_found.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        for appid in appids_found {
            // P0-3 & P0-2: Classify prefix before any orphan determination
            let (classification, resolved_title) = if let Some(game) = installed_games.get(&appid) {
                (
                    PrefixClassification::LiveGame(game.name.clone()),
                    Some(game.name.clone()),
                )
            } else if let Some(shortcut_name) = protected_shortcuts.get(&appid) {
                (
                    PrefixClassification::NonSteamShortcut(shortcut_name.clone()),
                    Some(shortcut_name.clone()),
                )
            } else if let Some(infra_name) = get_infrastructure_name(&appid) {
                (
                    PrefixClassification::SteamInfrastructure(infra_name.to_string()),
                    Some(infra_name.to_string()),
                )
            } else {
                (PrefixClassification::Orphaned, None)
            };

            let compatdata_path = {
                let p = compatdata_root.join(&appid);
                if p.is_dir() {
                    Some(p)
                } else {
                    None
                }
            };

            let shadercache_path = {
                let p = shadercache_root.join(&appid);
                if p.is_dir() {
                    Some(p)
                } else {
                    None
                }
            };

            let (compatdata_usage, compat_mtime) = compatdata_path
                .as_deref()
                .map(calculate_directory_usage)
                .unwrap_or_default();

            let (shadercache_usage, shader_mtime) = shadercache_path
                .as_deref()
                .map(calculate_directory_usage)
                .unwrap_or_default();

            let last_modified = match (compat_mtime, shader_mtime) {
                (Some(c), Some(s)) => Some(c.max(s)),
                (Some(c), None) => Some(c),
                (None, Some(s)) => Some(s),
                (None, None) => None,
            };

            // P1-2: Support --older-than filter
            if let Some(filter_dur) = older_than {
                if let Some(mtime) = last_modified {
                    if let Ok(elapsed) = SystemTime::now().duration_since(mtime) {
                        if elapsed < filter_dur {
                            continue; // Untouched duration is newer than threshold; skip
                        }
                    }
                }
            }

            let mut warnings = Vec::new();
            let detected_saves = compatdata_path
                .as_deref()
                .map(|p| sniff_save_files(p, &mut warnings))
                .unwrap_or_default();

            let (is_high_value, high_value_reasons) = compatdata_path
                .as_deref()
                .map(detect_high_value_prefix)
                .unwrap_or_default();

            let title = resolved_title.or_else(|| {
                compatdata_path
                    .as_deref()
                    .and_then(infer_title_from_compatdata)
            });

            prefixes.push(OrphanedPrefix {
                appid,
                title,
                classification,
                library_path: lib.path.clone(),
                compatdata_path,
                compatdata_usage,
                shadercache_path,
                shadercache_usage,
                detected_saves,
                last_modified,
                is_high_value,
                high_value_reasons,
                warnings,
            });
        }
    }

    prefixes.sort_by_key(|a| std::cmp::Reverse(a.total_apparent_bytes()));
    Ok(prefixes)
}

/// Convenience function returning only prefixes eligible for deletion (Orphaned).
pub fn scan_orphans(
    libraries: &[LibraryFolder],
    installed_games: &HashMap<String, InstalledGame>,
    protected_shortcuts: &HashMap<String, String>,
) -> Result<Vec<OrphanedPrefix>> {
    let all = scan_all_prefixes(libraries, installed_games, protected_shortcuts, None)?;
    Ok(all.into_iter().filter(|p| p.is_deletable()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniff_save_files_blocklist() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_blocklist_saves");
        let _ = fs::remove_dir_all(&temp_dir);

        let user_docs = temp_dir
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("Documents")
            .join("My Games")
            .join("TestRPG");
        let _ = fs::create_dir_all(&user_docs);

        // 1. Extensionless save file
        let extless = user_docs.join("SAVE_SLOT_01");
        fs::write(&extless, b"extensionless save payload").unwrap();

        // 2. .json save file
        let json_save = user_docs.join("state.json");
        fs::write(&json_save, b"{\"level\": 42}").unwrap();

        // 3. Crash dump (must be ignored)
        let crash_dmp = user_docs.join("crash_2026.dmp");
        fs::write(&crash_dmp, b"dump data").unwrap();

        // 4. Log file (must be ignored)
        let log_file = user_docs.join("debug.log");
        fs::write(&log_file, b"log data").unwrap();

        let mut warnings = Vec::new();
        let saves = sniff_save_files(&temp_dir, &mut warnings);

        let file_names: Vec<String> = saves
            .iter()
            .map(|s| s.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(file_names.contains(&"SAVE_SLOT_01".to_string()));
        assert!(file_names.contains(&"state.json".to_string()));
        assert!(!file_names.contains(&"crash_2026.dmp".to_string()));
        assert!(!file_names.contains(&"debug.log".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_validate_prefix_path_for_deletion() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_val_del");
        let compat_dir = temp_dir.join("compatdata");
        let prefix_dir = compat_dir.join("489830");
        let _ = fs::create_dir_all(&prefix_dir);

        // Valid path
        let validated = validate_prefix_path_for_deletion(&prefix_dir, "compatdata");
        assert!(validated.is_ok());

        // Invalid parent
        let wrong_parent = validate_prefix_path_for_deletion(&prefix_dir, "shadercache");
        assert!(wrong_parent.is_err());

        // Non-numeric dir name
        let bad_name = compat_dir.join("not_numeric");
        let _ = fs::create_dir_all(&bad_name);
        assert!(validate_prefix_path_for_deletion(&bad_name, "compatdata").is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_symlink_escaping_prefix_is_flagged() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_symlink_escape");
        let _ = fs::remove_dir_all(&temp_dir);

        let sentinel_dir = std::env::temp_dir().join("prefixpug_sentinel_outside");
        let _ = fs::create_dir_all(&sentinel_dir);
        fs::write(sentinel_dir.join("private.txt"), b"sensitive data").unwrap();

        let pfx_saved = temp_dir
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("Saved Games");
        let _ = fs::create_dir_all(&pfx_saved);

        let link_path = pfx_saved.join("symlink_to_outside");
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&sentinel_dir, &link_path);

        let mut warnings = Vec::new();
        let saves = sniff_save_files(&temp_dir, &mut warnings);

        // Sentinel outside must not be included
        for s in &saves {
            assert!(!s.path.starts_with(&sentinel_dir));
        }
        assert!(!warnings.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&sentinel_dir);
    }
}
