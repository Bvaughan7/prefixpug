use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::scanner::OrphanedPrefix;

pub fn default_backup_root() -> Result<PathBuf> {
    if let Some(data_dir) = dirs::data_local_dir() {
        Ok(data_dir.join("prefixpug").join("backups"))
    } else if let Some(home) = dirs::home_dir() {
        Ok(home.join(".local/share/prefixpug/backups"))
    } else {
        anyhow::bail!("Could not determine local data directory for save backups");
    }
}

pub fn backup_orphan_saves(
    orphan: &OrphanedPrefix,
    backup_root: &Path,
) -> Result<Option<PathBuf>> {
    if orphan.detected_saves.is_empty() {
        return Ok(None);
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let target_dir = backup_root.join(format!("{}_{}", timestamp, orphan.appid));
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create backup directory at {:?}", target_dir))?;

    let compat_base = orphan.compatdata_path.as_deref();

    for save_file in &orphan.detected_saves {
        if !save_file.is_file() {
            continue;
        }

        let rel_path = if let Some(base) = compat_base {
            save_file.strip_prefix(base).unwrap_or(save_file)
        } else {
            save_file
        };

        let dest = target_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(save_file, &dest)
            .with_context(|| format!("Failed to copy save file from {:?} to {:?}", save_file, dest))?;
    }

    Ok(Some(target_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_no_saves() {
        let orphan = OrphanedPrefix {
            appid: "999".to_string(),
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
    fn test_backup_with_saves() {
        let temp_dir = std::env::temp_dir().join("prefixpug_backup_test_with_saves");
        let save_file = temp_dir.join("save.dat");
        let _ = fs::create_dir_all(&temp_dir);
        let _ = fs::write(&save_file, b"save_bytes");

        let orphan = OrphanedPrefix {
            appid: "123".to_string(),
            library_path: PathBuf::from("/tmp"),
            compatdata_path: Some(temp_dir.clone()),
            compatdata_size: 10,
            shadercache_path: None,
            shadercache_size: 0,
            detected_saves: vec![save_file],
        };

        let backup_root = temp_dir.join("backups");
        let res = backup_orphan_saves(&orphan, &backup_root).expect("backup");
        assert!(res.is_some());
        let archived = res.unwrap();
        assert!(archived.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
