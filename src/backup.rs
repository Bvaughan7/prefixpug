use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Archive, Builder};

use crate::scanner::OrphanedPrefix;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub original_path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub appid: String,
    pub title: Option<String>,
    pub timestamp: u64,
    pub total_save_size: u64,
    pub tool_version: String,
    pub archive_file: String,
    pub archive_sha256: String,
    pub warnings: Vec<String>,
    pub files: Vec<BackupEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub directory: PathBuf,
    pub manifest: BackupManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub backup_id: String,
    pub is_valid: bool,
    pub files_verified: usize,
    pub total_bytes_verified: u64,
    pub errors: Vec<String>,
}

pub fn default_backup_root() -> Result<PathBuf> {
    // XDG compliance: honor $XDG_DATA_HOME
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        if !xdg_data.trim().is_empty() {
            return Ok(PathBuf::from(xdg_data).join("prefixpug").join("backups"));
        }
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        Ok(data_dir.join("prefixpug").join("backups"))
    } else if let Some(home) = dirs::home_dir() {
        Ok(home.join(".local/share/prefixpug/backups"))
    } else {
        bail!("Could not determine local data directory for save backups");
    }
}

fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open file for sha256 {:?}", path))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Backs up detected saves for an orphaned prefix into a compressed archive.
/// P1-5: Computes SHA-256 for each file, computes archive SHA-256, fsyncs to disk,
/// and verifies decompression before returning success.
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

    let compat_base = orphan.compatdata_path.as_deref();
    let mut entries = Vec::new();

    // 1. Build tar.gz
    {
        let tar_gz_file = File::create(&archive_path)
            .with_context(|| format!("Failed to create archive file at {:?}", archive_path))?;
        let enc = GzEncoder::new(tar_gz_file, Compression::default());
        let mut tar = Builder::new(enc);

        for save in &orphan.detected_saves {
            if !save.path.is_file() {
                continue;
            }

            let rel_path = if let Some(base) = compat_base {
                save.path.strip_prefix(base).unwrap_or(&save.path)
            } else {
                &save.path
            };

            let file_hash = compute_file_sha256(&save.path)?;

            let mut file = File::open(&save.path)
                .with_context(|| format!("Failed to open save file {:?}", save.path))?;
            tar.append_file(rel_path, &mut file)
                .with_context(|| format!("Failed to append {:?} to archive", rel_path))?;

            entries.push(BackupEntry {
                original_path: save.path.to_string_lossy().to_string(),
                relative_path: rel_path.to_string_lossy().to_string(),
                size_bytes: save.size_bytes,
                sha256: file_hash,
            });
        }

        let mut enc = tar.into_inner().context("Failed to finalize tar archive")?;
        enc.flush()?;
        let file = enc.finish().context("Failed to finish gzip compression")?;
        // P1-5: fsync archive before proceeding
        file.sync_all().context("Failed to fsync archive file")?;
    }

    // 2. Compute archive SHA-256
    let archive_sha256 =
        compute_file_sha256(&archive_path).context("Failed to compute archive checksum")?;

    // 3. Verify archive is readable by decompressing headers
    {
        let test_file = File::open(&archive_path)?;
        let test_decoder = GzDecoder::new(test_file);
        let mut test_archive = Archive::new(test_decoder);
        let mut count = 0;
        for entry in test_archive.entries()? {
            let _ = entry?;
            count += 1;
        }
        if count != entries.len() {
            bail!(
                "Archive verification failed: expected {} entries, found {}",
                entries.len(),
                count
            );
        }
    }

    // 4. Write manifest.json
    let manifest = BackupManifest {
        appid: orphan.appid.clone(),
        title: orphan.title.clone(),
        timestamp,
        total_save_size: orphan.total_save_size(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        archive_file: archive_name,
        archive_sha256,
        warnings: orphan.warnings.clone(),
        files: entries,
    };

    let manifest_path = target_dir.join("manifest.json");
    let manifest_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize backup manifest")?;
    let mut manifest_file = File::create(&manifest_path)?;
    manifest_file.write_all(manifest_json.as_bytes())?;
    manifest_file.sync_all()?;

    Ok(Some(target_dir))
}

/// Verifies a vaulted backup archive against its manifest and checksums.
pub fn verify_backup(backup_id_or_path: &str, backup_root: &Path) -> Result<VerificationReport> {
    let backup_dir = if Path::new(backup_id_or_path).is_dir() {
        PathBuf::from(backup_id_or_path)
    } else {
        backup_root.join(backup_id_or_path)
    };

    let backup_id = backup_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(backup_id_or_path)
        .to_string();

    let mut report = VerificationReport {
        backup_id: backup_id.clone(),
        is_valid: true,
        files_verified: 0,
        total_bytes_verified: 0,
        errors: Vec::new(),
    };

    if !backup_dir.is_dir() {
        report.is_valid = false;
        report
            .errors
            .push(format!("Backup directory {:?} does not exist", backup_dir));
        return Ok(report);
    }

    let manifest_path = backup_dir.join("manifest.json");
    if !manifest_path.is_file() {
        report.is_valid = false;
        report.errors.push("Missing manifest.json".to_string());
        return Ok(report);
    }

    let content = fs::read_to_string(&manifest_path)?;
    let manifest: BackupManifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            report.is_valid = false;
            report.errors.push(format!("Corrupt manifest.json: {}", e));
            return Ok(report);
        }
    };

    let archive_path = backup_dir.join(&manifest.archive_file);
    if !archive_path.is_file() {
        report.is_valid = false;
        report
            .errors
            .push(format!("Archive file {:?} is missing", archive_path));
        return Ok(report);
    }

    // Verify archive SHA-256
    let actual_archive_hash = compute_file_sha256(&archive_path)?;
    if actual_archive_hash != manifest.archive_sha256 {
        report.is_valid = false;
        report.errors.push(format!(
            "Archive checksum mismatch! Expected {}, got {}",
            manifest.archive_sha256, actual_archive_hash
        ));
        return Ok(report);
    }

    // Read and verify archive entries
    let archive_file = File::open(&archive_path)?;
    let gz = GzDecoder::new(archive_file);
    let mut tar = Archive::new(gz);

    let mut manifest_map = std::collections::HashMap::new();
    for f in &manifest.files {
        manifest_map.insert(f.relative_path.clone(), f.clone());
    }

    for entry_res in tar.entries()? {
        let mut entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                report.is_valid = false;
                report
                    .errors
                    .push(format!("Error reading archive entry: {}", e));
                return Ok(report);
            }
        };

        let path = entry.path()?.to_string_lossy().to_string();
        if let Some(expected) = manifest_map.remove(&path) {
            let mut hasher = Sha256::new();
            let mut buf = [0u8; 64 * 1024];
            let mut total_read = 0u64;

            loop {
                let n = entry.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                total_read += n as u64;
            }

            let hash = format!("{:x}", hasher.finalize());
            if hash != expected.sha256 {
                report.is_valid = false;
                report.errors.push(format!(
                    "File {} checksum mismatch! Expected {}, got {}",
                    path, expected.sha256, hash
                ));
            } else {
                report.files_verified += 1;
                report.total_bytes_verified += total_read;
            }
        } else {
            report.is_valid = false;
            report
                .errors
                .push(format!("Unexpected file in archive: {}", path));
        }
    }

    if !manifest_map.is_empty() {
        report.is_valid = false;
        for missing in manifest_map.keys() {
            report.errors.push(format!(
                "File in manifest missing from archive: {}",
                missing
            ));
        }
    }

    Ok(report)
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
    use crate::scanner::{DiskUsage, OrphanedPrefix, SaveFileInfo};
    use crate::vdf_parser::PrefixClassification;

    #[test]
    fn test_backup_no_saves() {
        let orphan = OrphanedPrefix {
            appid: "999".to_string(),
            title: None,
            classification: PrefixClassification::Orphaned,
            library_path: PathBuf::from("/tmp"),
            compatdata_path: None,
            compatdata_usage: DiskUsage::default(),
            shadercache_path: None,
            shadercache_usage: DiskUsage::default(),
            detected_saves: vec![],
            last_modified: None,
            is_high_value: false,
            high_value_reasons: vec![],
            cloud_status: crate::vdf_parser::SteamCloudStatus::default(),
            warnings: vec![],
        };
        let backup_root = std::env::temp_dir().join("prefixpug_backup_test_empty");
        let res = backup_orphan_saves(&orphan, &backup_root).expect("backup");
        assert!(res.is_none());
    }

    #[test]
    fn test_backup_verify_and_restore_cycle() {
        let temp_dir = std::env::temp_dir().join("prefixpug_backup_cycle_verify_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let save_file = temp_dir.join("savegame.sav");
        fs::write(&save_file, b"save_data_payload_12345").expect("write save");

        let orphan = OrphanedPrefix {
            appid: "42".to_string(),
            title: Some("Galactic Hitchhiker".to_string()),
            classification: PrefixClassification::Orphaned,
            library_path: PathBuf::from("/tmp"),
            compatdata_path: Some(temp_dir.clone()),
            compatdata_usage: DiskUsage {
                apparent_bytes: 100,
                allocated_bytes: 512,
            },
            shadercache_path: None,
            shadercache_usage: DiskUsage::default(),
            detected_saves: vec![SaveFileInfo {
                path: save_file.clone(),
                size_bytes: 23,
            }],
            last_modified: None,
            is_high_value: false,
            high_value_reasons: vec![],
            cloud_status: crate::vdf_parser::SteamCloudStatus::default(),
            warnings: vec![],
        };

        let backup_root = temp_dir.join("vault");
        let res = backup_orphan_saves(&orphan, &backup_root).expect("backup");
        assert!(res.is_some());
        let archived_dir = res.unwrap();
        assert!(archived_dir.join("manifest.json").exists());
        assert!(archived_dir.join("saves.tar.gz").exists());

        // Verification test (P1-5)
        let report = verify_backup(&archived_dir.to_string_lossy(), &backup_root).expect("verify");
        assert!(report.is_valid);
        assert_eq!(report.files_verified, 1);
        assert!(report.errors.is_empty());

        // Restore test
        let restore_dest = temp_dir.join("restored");
        let restored = restore_backup(&archived_dir.to_string_lossy(), &backup_root, &restore_dest)
            .expect("restore");
        assert!(restored.join("savegame.sav").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
