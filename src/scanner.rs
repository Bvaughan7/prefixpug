use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::vdf_parser::{infer_title_from_compatdata, InstalledGame, LibraryFolder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFileInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanedPrefix {
    pub appid: String,
    pub title: Option<String>,
    pub library_path: PathBuf,
    pub compatdata_path: Option<PathBuf>,
    pub compatdata_size: u64,
    pub shadercache_path: Option<PathBuf>,
    pub shadercache_size: u64,
    pub detected_saves: Vec<SaveFileInfo>,
}

impl OrphanedPrefix {
    pub fn total_size(&self) -> u64 {
        self.compatdata_size + self.shadercache_size
    }

    pub fn total_save_size(&self) -> u64 {
        self.detected_saves.iter().map(|s| s.size_bytes).sum()
    }

    pub fn display_name(&self) -> String {
        match &self.title {
            Some(t) => format!("{} (AppID: {})", t, self.appid),
            None => format!("AppID: {}", self.appid),
        }
    }
}

/// Computes disk usage in bytes without following symlinks.
pub fn calculate_directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// The Pug's Nose heuristics: sniff through an orphaned prefix for game save files.
pub fn sniff_save_files(compatdata_dir: &Path) -> Vec<SaveFileInfo> {
    let mut saves = Vec::new();
    let pfx_drive_c = compatdata_dir.join("pfx").join("drive_c");
    if !pfx_drive_c.is_dir() {
        return saves;
    }

    // Standard Wine/Proton user roots
    let user_roots = [
        pfx_drive_c.join("users").join("steamuser"),
        pfx_drive_c.join("users").join("default"),
    ];

    let check_exts = [
        ".sav", ".save", ".ess", ".fos", ".skse", ".dat", ".sqlite", ".db", ".xml", ".json",
    ];

    for user_dir in &user_roots {
        if !user_dir.is_dir() {
            continue;
        }

        let target_subdirs = [
            user_dir.join("Saved Games"),
            user_dir.join("Documents"),
            user_dir.join("My Documents"),
            user_dir.join("AppData").join("Local"),
            user_dir.join("AppData").join("LocalLow"),
            user_dir.join("AppData").join("Roaming"),
        ];

        for subdir in &target_subdirs {
            if !subdir.is_dir() {
                continue;
            }

            for entry in WalkDir::new(subdir)
                .follow_links(false)
                .max_depth(6)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let lower_name = file_name.to_lowercase();

                    // Skip large logs or crash dumps
                    if lower_name.ends_with(".log")
                        || lower_name.ends_with(".dmp")
                        || lower_name.contains("cache")
                    {
                        continue;
                    }

                    // If file is in Saved Games or My Games, consider it a save
                    let in_saved_games = path.starts_with(user_dir.join("Saved Games"));
                    let in_my_games = path.starts_with(user_dir.join("Documents").join("My Games"))
                        || path.starts_with(user_dir.join("My Documents").join("My Games"));

                    let has_save_ext = check_exts.iter().any(|ext| lower_name.ends_with(ext));
                    let has_save_word =
                        lower_name.contains("save") || lower_name.contains("profile");

                    if in_saved_games || in_my_games || has_save_ext || has_save_word {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        // Save files are rarely > 500MB each
                        if size < 500 * 1024 * 1024 {
                            saves.push(SaveFileInfo {
                                path: path.to_path_buf(),
                                size_bytes: size,
                            });
                        }
                    }
                }
            }
        }
    }

    saves
}

/// Scans all library folders and discovers orphaned compatdata & shadercache prefixes.
pub fn scan_orphans(
    libraries: &[LibraryFolder],
    installed_games: &HashMap<String, InstalledGame>,
) -> Result<Vec<OrphanedPrefix>> {
    let mut orphans = Vec::new();

    for lib in libraries {
        let steamapps = lib.path.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }

        let compatdata_root = steamapps.join("compatdata");
        let shadercache_root = steamapps.join("shadercache");

        let mut appids_found = HashSet::new();

        if compatdata_root.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&compatdata_root) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.chars().all(|c| c.is_ascii_digit()) && name != "0" {
                                    appids_found.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        if shadercache_root.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&shadercache_root) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.chars().all(|c| c.is_ascii_digit()) && name != "0" {
                                    appids_found.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        for appid in appids_found {
            // Check if AppID is present in installed games
            if !installed_games.contains_key(&appid) {
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

                let compatdata_size = compatdata_path
                    .as_deref()
                    .map(calculate_directory_size)
                    .unwrap_or(0);

                let shadercache_size = shadercache_path
                    .as_deref()
                    .map(calculate_directory_size)
                    .unwrap_or(0);

                let detected_saves = compatdata_path
                    .as_deref()
                    .map(sniff_save_files)
                    .unwrap_or_default();

                let title = compatdata_path
                    .as_deref()
                    .and_then(infer_title_from_compatdata);

                orphans.push(OrphanedPrefix {
                    appid,
                    title,
                    library_path: lib.path.clone(),
                    compatdata_path,
                    compatdata_size,
                    shadercache_path,
                    shadercache_size,
                    detected_saves,
                });
            }
        }
    }

    orphans.sort_by_key(|a| std::cmp::Reverse(a.total_size()));
    Ok(orphans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniff_save_files_empty() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_empty_scan");
        let _ = std::fs::create_dir_all(&temp_dir);
        let saves = sniff_save_files(&temp_dir);
        assert!(saves.is_empty());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sniff_save_files_detected() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_saves_scan");
        let save_dir = temp_dir
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("Saved Games");
        let _ = std::fs::create_dir_all(&save_dir);
        let save_file = save_dir.join("game.sav");
        let _ = std::fs::write(&save_file, b"save_data_12345");

        let saves = sniff_save_files(&temp_dir);
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].path, save_file);
        assert_eq!(saves[0].size_bytes, 15);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
