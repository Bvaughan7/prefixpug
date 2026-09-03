use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::vdf_parser::{InstalledGame, LibraryFolder};

#[derive(Debug, Clone)]
pub struct OrphanedPrefix {
    pub appid: String,
    pub library_path: PathBuf,
    pub compatdata_path: Option<PathBuf>,
    pub compatdata_size: u64,
    pub shadercache_path: Option<PathBuf>,
    pub shadercache_size: u64,
    pub detected_saves: Vec<PathBuf>,
}

impl OrphanedPrefix {
    pub fn total_size(&self) -> u64 {
        self.compatdata_size + self.shadercache_size
    }
}

/// Computes the disk usage of a directory in bytes without following symlinks.
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

/// The Pug's Nose heuristics: sniff through an orphaned prefix for save files.
pub fn sniff_save_files(compatdata_dir: &Path) -> Vec<PathBuf> {
    let mut saves = Vec::new();
    let pfx_drive_c = compatdata_dir.join("pfx").join("drive_c");
    if !pfx_drive_c.is_dir() {
        return saves;
    }

    // Common save locations inside Wine/Proton drive_c
    let user_roots = [
        pfx_drive_c.join("users").join("steamuser"),
        pfx_drive_c.join("users").join("default"),
    ];

    let check_exts = [".sav", ".save", ".ess", ".dat", ".xml", ".json", ".sqlite"];

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

                    // If file is in Saved Games, always consider it a save candidate
                    if path.starts_with(user_dir.join("Saved Games")) {
                        saves.push(path.to_path_buf());
                        continue;
                    }

                    // Check for typical save extensions or keywords
                    let has_save_ext = check_exts.iter().any(|ext| lower_name.ends_with(ext));
                    let has_save_word = lower_name.contains("save") || lower_name.contains("profile");

                    if has_save_ext || has_save_word {
                        saves.push(path.to_path_buf());
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

                orphans.push(OrphanedPrefix {
                    appid,
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

    orphans.sort_by(|a, b| b.total_size().cmp(&a.total_size()));
    Ok(orphans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniff_save_files_empty() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_empty");
        let _ = std::fs::create_dir_all(&temp_dir);
        let saves = sniff_save_files(&temp_dir);
        assert!(saves.is_empty());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sniff_save_files_detected() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_saves");
        let save_dir = temp_dir
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("Saved Games");
        let _ = std::fs::create_dir_all(&save_dir);
        let save_file = save_dir.join("game.sav");
        let _ = std::fs::write(&save_file, b"save_data");

        let saves = sniff_save_files(&temp_dir);
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0], save_file);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
