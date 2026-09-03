use anyhow::{bail, Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::Builder;

use crate::scanner::OrphanedPrefix;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub original_path: String,
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub appid: String,
    pub title: Option<String>,
    pub timestamp: u64,
    pub total_save_size: u64,
    pub files: Vec<BackupEntry>,
    pub archive_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub directory: PathBuf,
    pub manifest: BackupManifest,
}

pub fn default_backup_root() -> Result<PathBuf> {
    if let Some(data_dir) = dirs::data_local_dir() {
        Ok(data_dir.join("prefixpug").join("backups"))
    } else if let Some(home) = dirs::home_dir() {
        Ok(home.join(".local/share/prefixpug/backups"))
    } else {
        bail!("Could not determine local data directory for save backups");
    }
}

pub fn backup_orphan_saves(orphan: &OrphanedPrefix, backup_root: &Path) -> Result<Option<PathBuf>> {
    if orphan.detected_saves.is_empty() {
        return Ok(None);
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let backup_dir_name = format!("{}_{}", orphan.appid, timestamp);
    let target_dir = backup_root.join(&backup_dir_name);
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create backup directory at {:?}", target_dir))?;

    let archive_name = "saves.tar.gz".to_string();
    let archive_path = target_dir.join(&archive_name);

    let tar_gz = File::create(&archive_path)
        .with_context(|| format!("Failed to create archive file at {:?}", archive_path))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    let compat_base = orphan.compatdata_path.as_deref();
    let mut entries = Vec::new();

    for save in &orphan.detected_saves {
        if !save.path.is_file() {
            continue;
        }

        let rel_path = if let Some(base) = compat_base {
            save.path.strip_prefix(base).unwrap_or(&save.path)
        } else {
            &save.path
        };

        let mut file = File::open(&save.path)
            .with_context(|| format!("Failed to open save file {:?}", save.path))?;
        tar.append_file(rel_path, &mut file)
            .with_context(|| format!("Failed to append {:?} to archive", rel_path))?;

        entries.push(BackupEntry {
            original_path: save.path.to_string_lossy().to_string(),
            relative_path: rel_path.to_string_lossy().to_string(),
            size_bytes: save.size_bytes,
        });
    }

    tar.finish().context("Failed to finalize tar.gz archive")?;

    let manifest = BackupManifest {
        appid: orphan.appid.clone(),
        title: orphan.title.clone(),
        timestamp,
        total_save_size: orphan.total_save_size(),
        files: entries,
        archive_file: archive_name,
    };

    let manifest_path = target_dir.join("manifest.json");
    let manifest_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize backup manifest")?;
    fs::write(&manifest_path, manifest_json)
        .with_context(|| format!("Failed to write manifest at {:?}", manifest_path))?;

    Ok(Some(target_dir))
}

pub fn list_backups(backup_root: &Path) -> Result<Vec<BackupRecord>> {
    let mut records = Vec::new();
    if !backup_root.is_dir() {
        return Ok(records);
    }

    for entry in fs::read_dir(backup_root)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.is_file() {
                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<BackupManifest>(&content) {
                        records.push(BackupRecord {
                            directory: path,
                            manifest,
                        });
                    }
                }
            }
        }
    }

    records.sort_by_key(|a| std::cmp::Reverse(a.manifest.timestamp));
    Ok(records)
}

pub fn restore_backup(
    backup_id_or_path: &str,
    backup_root: &Path,
    target_dir: &Path,
) -> Result<PathBuf> {
    let backup_dir = if Path::new(backup_id_or_path).is_dir() {
        PathBuf::from(backup_id_or_path)
    } else {
        backup_root.join(backup_id_or_path)
    };

    if !backup_dir.is_dir() {
        bail!("Backup directory not found at {:?}", backup_dir);
    }

    let manifest_path = backup_dir.join("manifest.json");
    if !manifest_path.is_file() {
        bail!("Missing manifest.json in backup directory {:?}", backup_dir);
    }

    let content = fs::read_to_string(&manifest_path)?;
    let manifest: BackupManifest = serde_json::from_str(&content)?;

    let archive_path = backup_dir.join(&manifest.archive_file);
    if !archive_path.is_file() {
        bail!("Archive file {:?} not found", archive_path);
    }

    fs::create_dir_all(target_dir)?;

    let tar_gz = File::open(&archive_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive
        .unpack(target_dir)
        .with_context(|| format!("Failed to unpack archive into {:?}", target_dir))?;

    Ok(target_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::SaveFileInfo;

    #[test]
    fn test_backup_no_saves() {
        let orphan = OrphanedPrefix {
            appid: "999".to_string(),
            title: None,
            library_path: PathBuf::from("/tmp"),
            compatdata_path: None,
            compatdata_size: 0,
            shadercache_path: None,
            shadercache_size: 0,
            detected_saves: vec![],
        };
        let backup_root = std::env::temp_dir().join("prefixpug_backup_test_empty");
        let res = backup_orphan_saves(&orphan, &backup_root).expect("backup");
        assert!(res.is_none());
    }

    #[test]
    fn test_backup_and_restore_cycle() {
        let temp_dir = std::env::temp_dir().join("prefixpug_backup_cycle_test");
        let _ = fs::create_dir_all(&temp_dir);

        let save_file = temp_dir.join("savegame.sav");
        fs::write(&save_file, b"save_data_payload_12345").expect("write save");

        let orphan = OrphanedPrefix {
            appid: "42".to_string(),
            title: Some("Galactic Hitchhiker".to_string()),
            library_path: PathBuf::from("/tmp"),
            compatdata_path: Some(temp_dir.clone()),
            compatdata_size: 100,
            shadercache_path: None,
            shadercache_size: 0,
            detected_saves: vec![SaveFileInfo {
                path: save_file.clone(),
                size_bytes: 23,
            }],
        };

        let backup_root = temp_dir.join("vault");
        let res = backup_orphan_saves(&orphan, &backup_root).expect("backup");
        assert!(res.is_some());
        let archived_dir = res.unwrap();
        assert!(archived_dir.join("manifest.json").exists());
        assert!(archived_dir.join("saves.tar.gz").exists());

        // List backups test
        let list = list_backups(&backup_root).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].manifest.appid, "42");
        assert_eq!(
            list[0].manifest.title.as_deref(),
            Some("Galactic Hitchhiker")
        );

        // Restore test
        let restore_dest = temp_dir.join("restored");
        let restored = restore_backup(&archived_dir.to_string_lossy(), &backup_root, &restore_dest)
            .expect("restore");
        assert!(restored.join("savegame.sav").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
