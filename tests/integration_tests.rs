use std::fs;

// Use test helpers simulating Steam filesystem structure
#[test]
fn test_end_to_end_steam_scan_and_orphan_detection() {
    let base_dir = std::env::temp_dir().join("prefixpug_integration_test");
    let _ = fs::remove_dir_all(&base_dir);

    let steamapps = base_dir.join("steamapps");
    let compatdata = steamapps.join("compatdata");
    let shadercache = steamapps.join("shadercache");

    fs::create_dir_all(&compatdata).expect("create compatdata");
    fs::create_dir_all(&shadercache).expect("create shadercache");

    // 1. Create an INSTALLED game (AppID 100)
    let installed_acf = steamapps.join("appmanifest_100.acf");
    let acf_content = r#"
"AppState"
{
	"appid"		"100"
	"name"		"Cyberpunk Action Game"
	"installdir"	"CyberpunkAction"
	"SizeOnDisk"	"5000000"
}
"#;
    fs::write(&installed_acf, acf_content).expect("write acf");

    // Prefix for installed game 100
    let installed_pfx = compatdata.join("100");
    fs::create_dir_all(&installed_pfx).expect("create pfx 100");
    fs::write(installed_pfx.join("dummy.bin"), b"data").expect("write dummy");

    // 2. Create an ORPHANED game (AppID 200) with save files
    let orphan_pfx = compatdata.join("200");
    let save_dir = orphan_pfx
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Saved Games");
    fs::create_dir_all(&save_dir).expect("create save_dir");
    fs::write(save_dir.join("profile.sav"), b"my_game_save_12345").expect("write save");

    let orphan_shader = shadercache.join("200");
    fs::create_dir_all(&orphan_shader).expect("create orphan shader");
    fs::write(orphan_shader.join("dxvk.cache"), b"shader_cache_bytes").expect("write shader");

    // 3. Create libraryfolders.vdf
    let vdf_file = steamapps.join("libraryfolders.vdf");
    let vdf_content = format!(
        r#"
"libraryfolders"
{{
	"0"
	{{
		"path"		"{}"
		"label"		"Test Drive"
		"apps"
		{{
			"100"		"5000000"
		}}
	}}
}}
"#,
        base_dir.display()
    );
    fs::write(&vdf_file, vdf_content).expect("write vdf");

    // Execute scan
    let libraries = prefixpug::vdf_parser::parse_library_folders(&vdf_file).expect("parse libs");
    assert_eq!(libraries.len(), 1);

    let installed_games =
        prefixpug::vdf_parser::discover_installed_games(&libraries).expect("discover installed");
    assert!(installed_games.contains_key("100"));
    assert!(!installed_games.contains_key("200"));

    let orphans =
        prefixpug::scanner::scan_orphans(&libraries, &installed_games).expect("scan orphans");

    assert_eq!(orphans.len(), 1);
    let orphan = &orphans[0];
    assert_eq!(orphan.appid, "200");
    assert!(orphan.compatdata_path.is_some());
    assert!(orphan.shadercache_path.is_some());
    assert_eq!(orphan.detected_saves.len(), 1);
    assert_eq!(orphan.detected_saves[0].size_bytes, 18);

    // Test save vault backup
    let vault_dir = base_dir.join("vault");
    let backup_res =
        prefixpug::backup::backup_orphan_saves(orphan, &vault_dir).expect("backup saves");
    assert!(backup_res.is_some());
    let backup_folder = backup_res.unwrap();
    assert!(backup_folder.join("manifest.json").exists());
    assert!(backup_folder.join("saves.tar.gz").exists());

    // Clean up test sandbox
    let _ = fs::remove_dir_all(&base_dir);
}
