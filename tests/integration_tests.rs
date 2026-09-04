use prefixpug::backup;
use prefixpug::scanner::{self, safe_delete_prefix_directory, validate_prefix_path_for_deletion};
use prefixpug::vdf_parser::{
    discover_installed_games, discover_non_steam_shortcuts, parse_library_folders,
    validate_libraries_reachable,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

struct SyntheticSteamFixture {
    root_dir: PathBuf,
    lib_a: PathBuf,
    lib_b: PathBuf,
    vdf_path: PathBuf,
}

impl SyntheticSteamFixture {
    fn new(test_name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("prefixpug_fixture_{}", test_name));
        let _ = fs::remove_dir_all(&root);

        let lib_a = root.join("LibraryA");
        let lib_b = root.join("LibraryB");

        fs::create_dir_all(lib_a.join("steamapps").join("compatdata")).unwrap();
        fs::create_dir_all(lib_a.join("steamapps").join("shadercache")).unwrap();
        fs::create_dir_all(lib_b.join("steamapps").join("compatdata")).unwrap();
        fs::create_dir_all(lib_b.join("steamapps").join("shadercache")).unwrap();

        let vdf_path = lib_a.join("steamapps").join("libraryfolders.vdf");
        let vdf_content = format!(
            r#"
"libraryfolders"
{{
	"0"
	{{
		"path"		"{}"
		"label"		"Drive A"
		"apps"
		{{
			"100"		"50000000"
		}}
	}}
	"1"
	{{
		"path"		"{}"
		"label"		"Drive B"
		"apps"
		{{
			"200"		"80000000"
		}}
	}}
}}
"#,
            lib_a.to_string_lossy(),
            lib_b.to_string_lossy()
        );
        fs::write(&vdf_path, vdf_content).unwrap();

        Self {
            root_dir: root,
            lib_a,
            lib_b,
            vdf_path,
        }
    }
}

impl Drop for SyntheticSteamFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root_dir);
    }
}

// -----------------------------------------------------------------------------
// P0-1: Multi-Library Safety & Unreachable Library Abort
// -----------------------------------------------------------------------------

#[test]
fn test_p0_1_multi_library_scan_and_unreachable_abort() {
    let fixture = SyntheticSteamFixture::new("p0_1_multilib");

    // Game 100 installed in Library A
    fs::write(
        fixture.lib_a.join("steamapps").join("appmanifest_100.acf"),
        "\"AppState\" { \"appid\" \"100\" \"name\" \"Game In A\" }\n",
    )
    .unwrap();

    // Game 200 installed in Library B
    fs::write(
        fixture.lib_b.join("steamapps").join("appmanifest_200.acf"),
        "\"AppState\" { \"appid\" \"200\" \"name\" \"Game In B\" }\n",
    )
    .unwrap();

    // Compatdata for 200 lives in Library B
    let pfx_200 = fixture
        .lib_b
        .join("steamapps")
        .join("compatdata")
        .join("200");
    fs::create_dir_all(&pfx_200).unwrap();

    // Compatdata for orphan 999 lives in Library A
    let pfx_999 = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("999");
    fs::create_dir_all(&pfx_999).unwrap();

    // 1. Scan across both libraries
    let libs = parse_library_folders(&fixture.vdf_path).unwrap();
    assert_eq!(libs.len(), 2);
    validate_libraries_reachable(&libs).unwrap();

    let installed = discover_installed_games(&libs).unwrap();
    let shortcuts = HashMap::new();

    let orphans = scanner::scan_orphans(&libs, &installed, &shortcuts).unwrap();

    // Assert Game 200 (in Library B) is NOT an orphan even though scanned with Library A
    let orphan_ids: Vec<String> = orphans.iter().map(|o| o.appid.clone()).collect();
    assert!(orphan_ids.contains(&"999".to_string()));
    assert!(!orphan_ids.contains(&"100".to_string()));
    assert!(!orphan_ids.contains(&"200".to_string()));

    // 2. Unreachable library safety check:
    // If Library B becomes unmounted/unreachable, scanning must abort rather than deleting 200
    let mut unmounted_libs = libs.clone();
    unmounted_libs[1].is_reachable = false;

    let res = validate_libraries_reachable(&unmounted_libs);
    assert!(res.is_err(), "Must abort if any library is unreachable");
}

// -----------------------------------------------------------------------------
// P0-2: Non-Steam Shortcut Protection (shortcuts.vdf)
// -----------------------------------------------------------------------------

#[test]
fn test_p0_2_non_steam_shortcuts_protection() {
    let fixture = SyntheticSteamFixture::new("p0_2_shortcuts");

    // Create userdata with binary shortcuts.vdf
    let userdata_config = fixture
        .lib_a
        .join("userdata")
        .join("12345678")
        .join("config");
    fs::create_dir_all(&userdata_config).unwrap();

    // Synthetic shortcuts.vdf with AppID 3060000000 ("Battle.net")
    let mut bytes = Vec::new();
    bytes.push(0x00);
    bytes.extend_from_slice(b"shortcuts\0");
    bytes.push(0x00);
    bytes.extend_from_slice(b"0\0");
    bytes.push(0x02);
    bytes.extend_from_slice(b"appid\0");
    bytes.extend_from_slice(&3060000000u32.to_le_bytes());
    bytes.push(0x01);
    bytes.extend_from_slice(b"AppName\0Battle.net\0");
    bytes.push(0x01);
    bytes.extend_from_slice(b"Exe\0Battle.net Launcher.exe\0");
    bytes.push(0x08);
    bytes.push(0x08);

    fs::write(userdata_config.join("shortcuts.vdf"), bytes).unwrap();

    // Create compatdata prefix for 3060000000 (has no appmanifest_*.acf)
    let pfx_shortcut = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("3060000000");
    fs::create_dir_all(&pfx_shortcut).unwrap();

    // Create orphaned prefix 888888
    let pfx_orphan = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("888888");
    fs::create_dir_all(&pfx_orphan).unwrap();

    let libs = parse_library_folders(&fixture.vdf_path).unwrap();
    let installed = discover_installed_games(&libs).unwrap();
    let shortcuts = discover_non_steam_shortcuts(std::slice::from_ref(&fixture.lib_a)).unwrap();

    let orphans = scanner::scan_orphans(&libs, &installed, &shortcuts).unwrap();
    let orphan_ids: Vec<String> = orphans.iter().map(|o| o.appid.clone()).collect();

    // 3060000000 must NOT be in the orphan list
    assert!(!orphan_ids.contains(&"3060000000".to_string()));
    // Genuine orphan 888888 must be detected
    assert!(orphan_ids.contains(&"888888".to_string()));
}

// -----------------------------------------------------------------------------
// P0-3: Steam Infrastructure AppID Protection
// -----------------------------------------------------------------------------

#[test]
fn test_p0_3_steam_infrastructure_deny_list() {
    let fixture = SyntheticSteamFixture::new("p0_3_infra");

    // Create compatdata prefixes for Proton Experimental (1493710) and Runtime (1628350)
    let pfx_proton = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("1493710");
    let pfx_runtime = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("1628350");
    let pfx_orphan = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("555555");

    fs::create_dir_all(&pfx_proton).unwrap();
    fs::create_dir_all(&pfx_runtime).unwrap();
    fs::create_dir_all(&pfx_orphan).unwrap();

    let libs = parse_library_folders(&fixture.vdf_path).unwrap();
    let installed = discover_installed_games(&libs).unwrap();
    let shortcuts = HashMap::new();

    let all_prefixes = scanner::scan_all_prefixes(&libs, &installed, &shortcuts, None).unwrap();

    for p in &all_prefixes {
        if p.appid == "1493710" || p.appid == "1628350" {
            assert!(
                !p.is_deletable(),
                "Infrastructure tool {} must not be deletable",
                p.appid
            );
        }
    }

    let orphans = scanner::scan_orphans(&libs, &installed, &shortcuts).unwrap();
    let orphan_ids: Vec<String> = orphans.iter().map(|o| o.appid.clone()).collect();

    assert!(!orphan_ids.contains(&"1493710".to_string()));
    assert!(!orphan_ids.contains(&"1628350".to_string()));
    assert!(orphan_ids.contains(&"555555".to_string()));
}

// -----------------------------------------------------------------------------
// P0-4 & Canary: Blocklist Save Engine with Extensionless / JSON Saves Surviving
// -----------------------------------------------------------------------------

#[test]
fn test_p0_4_canary_save_without_extension_survives_cycle() {
    let fixture = SyntheticSteamFixture::new("p0_4_canary");

    let pfx = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("777777");
    let saves_root = pfx
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Saved Games")
        .join("IndieGame");
    fs::create_dir_all(&saves_root).unwrap();

    // 1. Extensionless save file
    let extless_save = saves_root.join("SAVE_SLOT_HEAD");
    fs::write(&extless_save, b"CANARY_SAVE_CONTENT_RAW_BINARY_12345").unwrap();

    // 2. .json save file
    let json_save = saves_root.join("profile.json");
    fs::write(&json_save, b"{\"player\": \"canary\", \"gold\": 99999}").unwrap();

    // 3. Junk: Crash dump (must be filtered out)
    let crash_dmp = saves_root.join("crash.dmp");
    fs::write(&crash_dmp, b"crash dump garbage").unwrap();

    let mut warnings = Vec::new();
    let saves = scanner::sniff_save_files(&pfx, &mut warnings);

    let save_names: Vec<String> = saves
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(save_names.contains(&"SAVE_SLOT_HEAD".to_string()));
    assert!(save_names.contains(&"profile.json".to_string()));
    assert!(!save_names.contains(&"crash.dmp".to_string()));

    // Round-trip vault & restore canary test
    let orphan = scanner::OrphanedPrefix {
        appid: "777777".to_string(),
        title: Some("Canary Game".to_string()),
        classification: prefixpug::vdf_parser::PrefixClassification::Orphaned,
        library_path: fixture.lib_a.clone(),
        compatdata_path: Some(pfx.clone()),
        compatdata_usage: scanner::DiskUsage {
            apparent_bytes: 500,
            allocated_bytes: 4096,
        },
        shadercache_path: None,
        shadercache_usage: scanner::DiskUsage::default(),
        detected_saves: saves,
        last_modified: None,
        is_high_value: false,
        high_value_reasons: vec![],
        cloud_status: prefixpug::vdf_parser::SteamCloudStatus::default(),
        warnings: vec![],
    };

    let vault_dir = fixture.root_dir.join("vault");
    let backup_dir = backup::backup_orphan_saves(&orphan, &vault_dir)
        .unwrap()
        .unwrap();

    // Verify backup passes SHA-256 validation
    let report = backup::verify_backup(&backup_dir.to_string_lossy(), &vault_dir).unwrap();
    assert!(report.is_valid);
    assert_eq!(report.files_verified, 2);

    // Purge prefix
    safe_delete_prefix_directory(&pfx).unwrap();
    assert!(!pfx.exists());

    // Restore backup
    let restored_dir = fixture.root_dir.join("restored_saves");
    backup::restore_backup(&backup_dir.to_string_lossy(), &vault_dir, &restored_dir).unwrap();

    // CANARY ASSERTION: Extensionless save file survived and is byte-for-byte identical
    let restored_extless = restored_dir
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Saved Games")
        .join("IndieGame")
        .join("SAVE_SLOT_HEAD");

    assert!(
        restored_extless.is_file(),
        "Canary save file must exist after restore"
    );
    let restored_content = fs::read(&restored_extless).unwrap();
    assert_eq!(restored_content, b"CANARY_SAVE_CONTENT_RAW_BINARY_12345");
}

// -----------------------------------------------------------------------------
// P0-5: Symlink and Path Traversal Safety
// -----------------------------------------------------------------------------

#[test]
fn test_p0_5_symlink_and_traversal_safety() {
    let fixture = SyntheticSteamFixture::new("p0_5_symlinks");

    let pfx = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("333333");
    let saves_dir = pfx
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Saved Games");
    fs::create_dir_all(&saves_dir).unwrap();

    // Create outside sentinel directory
    let outside_sentinel = fixture.root_dir.join("outside_private_data");
    fs::create_dir_all(&outside_sentinel).unwrap();
    fs::write(outside_sentinel.join("secret.txt"), b"sensitive data").unwrap();

    // Symlink from Saved Games pointing outside prefix
    let link_path = saves_dir.join("symlink_to_outside");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_sentinel, &link_path).unwrap();

    let mut warnings = Vec::new();
    let saves = scanner::sniff_save_files(&pfx, &mut warnings);

    // Assert outside sentinel was NOT collected
    for s in &saves {
        assert!(!s.path.starts_with(&outside_sentinel));
    }
    assert!(!warnings.is_empty(), "Must flag escaping symlink");

    // Path traversal rejection checks
    assert!(validate_prefix_path_for_deletion(Path::new("/"), "compatdata").is_err());
    assert!(validate_prefix_path_for_deletion(&fixture.lib_a, "compatdata").is_err());
    assert!(validate_prefix_path_for_deletion(&pfx, "compatdata").is_ok());
}

// -----------------------------------------------------------------------------
// P1-5 & Vault: Direct Save Vaulting and Verification
// -----------------------------------------------------------------------------

#[test]
fn test_vault_command_and_manifest_verification() {
    let fixture = SyntheticSteamFixture::new("vault_verification");

    let pfx = fixture
        .lib_a
        .join("steamapps")
        .join("compatdata")
        .join("777777");
    let saves_dir = pfx
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Saved Games")
        .join("GameStudio")
        .join("Profile");
    fs::create_dir_all(&saves_dir).unwrap();

    let save1 = saves_dir.join("save_slot_01.dat");
    let save2 = saves_dir.join("settings.ini");
    fs::write(&save1, b"SAVE_DATA_BINARY_PAYLOAD_ABC").unwrap();
    fs::write(&save2, b"[Audio]\nVolume=100\n").unwrap();

    let mut warnings = Vec::new();
    let detected_saves = scanner::sniff_save_files(&pfx, &mut warnings);
    assert_eq!(detected_saves.len(), 2);

    let prefix = scanner::OrphanedPrefix {
        appid: "777777".to_string(),
        title: Some("Mock Game".to_string()),
        classification: prefixpug::vdf_parser::PrefixClassification::Unknown,
        library_path: fixture.lib_a.clone(),
        compatdata_path: Some(pfx.clone()),
        compatdata_usage: scanner::DiskUsage::default(),
        shadercache_path: None,
        shadercache_usage: scanner::DiskUsage::default(),
        last_modified: None,
        detected_saves,
        is_high_value: false,
        high_value_reasons: Vec::new(),
        cloud_status: prefixpug::vdf_parser::SteamCloudStatus::default(),
        warnings: Vec::new(),
    };

    let backup_vault = fixture.root_dir.join("test_vault");
    let archive_dir = backup::backup_orphan_saves(&prefix, &backup_vault)
        .unwrap()
        .expect("Archive dir must be created");

    let backup_id = archive_dir.file_name().and_then(|n| n.to_str()).unwrap();

    // Verify backup passes SHA-256 integrity check
    let report = backup::verify_backup(backup_id, &backup_vault).unwrap();
    assert!(report.is_valid);
    assert_eq!(report.files_verified, 2);
    assert!(report.errors.is_empty());

    // Tamper test: modify one byte in the archive and assert verification fails
    let archive_tar = archive_dir.join("saves.tar.gz");
    let mut bytes = fs::read(&archive_tar).unwrap();
    if let Some(first_byte) = bytes.get_mut(15) {
        *first_byte ^= 0xFF;
    }
    fs::write(&archive_tar, bytes).unwrap();

    let tampered_report = backup::verify_backup(backup_id, &backup_vault).unwrap();
    assert!(
        !tampered_report.is_valid,
        "Tampered archive must fail verification"
    );
    assert!(!tampered_report.errors.is_empty());
}
