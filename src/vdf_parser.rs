use anyhow::{bail, Context, Result};
use keyvalues_parser::{Value, Vdf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFolder {
    pub path: PathBuf,
    pub label: String,
    pub apps: Vec<String>,
    pub is_reachable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledGame {
    pub appid: String,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: u64,
    pub library_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefixClassification {
    Orphaned,
    LiveGame(String),
    NonSteamShortcut(String),
    SteamInfrastructure(String),
    Unknown,
}

impl PrefixClassification {
    pub fn is_deletable(&self) -> bool {
        matches!(self, PrefixClassification::Orphaned)
    }

    pub fn badge(&self) -> &'static str {
        match self {
            PrefixClassification::Orphaned => "[ORPHAN]",
            PrefixClassification::LiveGame(_) => "[INSTALLED]",
            PrefixClassification::NonSteamShortcut(_) => "[SHORTCUT]",
            PrefixClassification::SteamInfrastructure(_) => "[RUNTIME]",
            PrefixClassification::Unknown => "[UNKNOWN]",
        }
    }
}

/// Known Steam infrastructure, Proton versions, and Steam Linux Runtime tools.
/// Never offered for deletion as doing so can damage the Proton/Steam runtime stack.
pub const INFRASTRUCTURE_APPIDS: &[(&str, &str)] = &[
    ("0", "Steam Internal Runtime"),
    ("228980", "Steamworks Common Redistributables"),
    ("1070560", "Steam Linux Runtime 1.0 (scout)"),
    ("1391110", "Steam Linux Runtime 2.0 (soldier)"),
    ("1628350", "Steam Linux Runtime 3.0 (sniper)"),
    ("373770", "Proton 3.7"),
    ("858280", "Proton 4.2"),
    ("1054230", "Proton 4.11"),
    ("1245040", "Proton 5.0"),
    ("1420170", "Proton 5.13"),
    ("1580130", "Proton 6.3"),
    ("1887720", "Proton 7.0"),
    ("2348590", "Proton 8.0"),
    ("2805730", "Proton 9.0"),
    ("1493710", "Proton Experimental"),
    ("2180100", "Proton Hotfix"),
    ("1826330", "Proton EasyAntiCheat Runtime"),
    ("1161040", "Proton BattlEye Runtime"),
    ("1113280", "Proton 5.9"),
    ("996510", "Steam Linux Runtime"),
];

pub fn get_infrastructure_name(appid: &str) -> Option<&'static str> {
    INFRASTRUCTURE_APPIDS
        .iter()
        .find(|(id, _)| *id == appid)
        .map(|(_, name)| *name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SteamCloudStatus {
    Synced,
    #[default]
    NotDetected,
}

impl SteamCloudStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            SteamCloudStatus::Synced => "[CLOUD-SYNCED]",
            SteamCloudStatus::NotDetected => "[LOCAL-ONLY]",
        }
    }

    pub fn is_synced(&self) -> bool {
        matches!(self, SteamCloudStatus::Synced)
    }
}

/// Checks whether Steam Cloud synchronizes files for this AppID by inspecting
/// userdata/<account_id>/<appid>/remotecache.vdf or remote/ directory.
pub fn check_steam_cloud_status(steam_roots: &[PathBuf], appid: &str) -> SteamCloudStatus {
    for root in steam_roots {
        let userdata = root.join("userdata");
        if userdata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&userdata) {
                for entry in entries.flatten() {
                    let app_dir = entry.path().join(appid);
                    if app_dir.join("remotecache.vdf").is_file() {
                        return SteamCloudStatus::Synced;
                    }
                    let remote_dir = app_dir.join("remote");
                    if remote_dir.is_dir() {
                        if let Ok(mut r_entries) = std::fs::read_dir(&remote_dir) {
                            if r_entries.next().is_some() {
                                return SteamCloudStatus::Synced;
                            }
                        }
                    }
                }
            }
        }
    }
    SteamCloudStatus::NotDetected
}

pub fn default_library_vdf_path() -> Result<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        let candidates = [
            home.join(".steam/root/steamapps/libraryfolders.vdf"),
            home.join(".steam/steam/steamapps/libraryfolders.vdf"),
            home.join(".local/share/Steam/steamapps/libraryfolders.vdf"),
            home.join(".var/app/com.valvesoftware.Steam/.steam/root/steamapps/libraryfolders.vdf"),
            home.join(
                ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/libraryfolders.vdf",
            ),
            home.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/libraryfolders.vdf"),
            // Steam Deck specific paths
            PathBuf::from("/home/deck/.local/share/Steam/steamapps/libraryfolders.vdf"),
            PathBuf::from("/home/deck/.steam/root/steamapps/libraryfolders.vdf"),
            PathBuf::from("/home/deck/.steam/steam/steamapps/libraryfolders.vdf"),
            // Removable media / SD card Steam Deck mount
            PathBuf::from("/run/media/mmcblk0p1/steamapps/libraryfolders.vdf"),
        ];

        for candidate in &candidates {
            if candidate.is_file() {
                return Ok(candidate.clone());
            }
        }
        bail!("Could not find Steam libraryfolders.vdf at standard locations");
    }
    bail!("Could not determine home directory");
}

pub fn parse_library_folders(vdf_path: &Path) -> Result<Vec<LibraryFolder>> {
    let content = std::fs::read_to_string(vdf_path)
        .with_context(|| format!("Failed to read library VDF file at {:?}", vdf_path))?;
    let parsed_tree = keyvalues_parser::parse(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse VDF format in {:?}: {}", vdf_path, e))?;
    let parsed = Vdf::from(parsed_tree);

    let root_obj = match &parsed.value {
        Value::Obj(obj) => obj,
        _ => bail!("Expected root object in {:?}", vdf_path),
    };

    let mut folders = Vec::new();

    for values in root_obj.values() {
        for val in values {
            if let Value::Obj(folder_obj) = val {
                let path_str =
                    folder_obj
                        .get("path")
                        .and_then(|v| v.first())
                        .and_then(|v| match v {
                            Value::Str(s) => Some(s.as_ref()),
                            _ => None,
                        });

                if let Some(path) = path_str {
                    let path_buf = PathBuf::from(path);
                    let is_reachable = path_buf.is_dir();

                    let label = folder_obj
                        .get("label")
                        .and_then(|v| v.first())
                        .and_then(|v| match v {
                            Value::Str(s) => Some(s.to_string()),
                            _ => None,
                        })
                        .unwrap_or_default();

                    let mut apps = Vec::new();
                    if let Some(apps_vals) = folder_obj.get("apps") {
                        for app_val in apps_vals {
                            if let Value::Obj(apps_obj) = app_val {
                                for app_id in apps_obj.keys() {
                                    apps.push(app_id.to_string());
                                }
                            }
                        }
                    }

                    folders.push(LibraryFolder {
                        path: path_buf,
                        label,
                        apps,
                        is_reachable,
                    });
                }
            }
        }
    }

    Ok(folders)
}

/// Validates that all libraries defined in libraryfolders.vdf are currently mounted and reachable.
/// P0-1: If any configured library root is unreachable (unmounted drive, missing path),
/// we must abort to prevent treating an unreadable library as empty.
pub fn validate_libraries_reachable(libraries: &[LibraryFolder]) -> Result<()> {
    for lib in libraries {
        if !lib.is_reachable {
            bail!(
                "Configured Steam library at {:?} ({:?}) is unmounted or unreachable. \
                 Aborting to prevent false-positive orphan classifications.",
                lib.path,
                if lib.label.is_empty() {
                    "unlabeled"
                } else {
                    &lib.label
                }
            );
        }
    }
    Ok(())
}

pub fn parse_appmanifest(acf_path: &Path, library_path: &Path) -> Result<InstalledGame> {
    let content = std::fs::read_to_string(acf_path)
        .with_context(|| format!("Failed to read ACF manifest at {:?}", acf_path))?;
    let parsed_tree = keyvalues_parser::parse(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse ACF manifest at {:?}: {}", acf_path, e))?;
    let parsed = Vdf::from(parsed_tree);

    let root_obj = match &parsed.value {
        Value::Obj(obj) => obj,
        _ => bail!("Expected root object in manifest {:?}", acf_path),
    };

    let get_str = |key: &str| -> Option<String> {
        root_obj
            .get(key)
            .and_then(|v| v.first())
            .and_then(|v| match v {
                Value::Str(s) => Some(s.to_string()),
                _ => None,
            })
    };

    let appid = match get_str("appid") {
        Some(id) => id,
        None => bail!("Missing appid in manifest {:?}", acf_path),
    };

    let name = get_str("name").unwrap_or_else(|| format!("App {}", appid));
    let installdir = get_str("installdir").unwrap_or_default();
    let size_on_disk = get_str("SizeOnDisk")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(InstalledGame {
        appid,
        name,
        installdir,
        size_on_disk,
        library_path: library_path.to_path_buf(),
    })
}

pub fn discover_installed_games(
    libraries: &[LibraryFolder],
) -> Result<HashMap<String, InstalledGame>> {
    // P0-1: Strictly assert all libraries are reachable before discovering
    validate_libraries_reachable(libraries)?;

    let mut games = HashMap::new();

    for lib in libraries {
        let steamapps = lib.path.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&steamapps) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("appmanifest_") && file_name.ends_with(".acf") {
                    match parse_appmanifest(&path, &lib.path) {
                        Ok(game) => {
                            games.insert(game.appid.clone(), game);
                        }
                        Err(e) => {
                            eprintln!("Warning parsing manifest {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }

    Ok(games)
}

// -----------------------------------------------------------------------------
// P0-2: Non-Steam Game Shortcuts (shortcuts.vdf) Parser
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NonSteamShortcut {
    pub appid: u32,
    pub app_name: String,
    pub exe: String,
    pub computed_compatdata_id: String,
}

fn read_null_terminated_string(bytes: &[u8], cursor: &mut usize) -> Result<String> {
    let start = *cursor;
    while *cursor < bytes.len() && bytes[*cursor] != 0 {
        *cursor += 1;
    }
    if *cursor >= bytes.len() {
        bail!("Unexpected EOF reading null-terminated string in binary VDF");
    }
    let s = std::str::from_utf8(&bytes[start..*cursor])
        .context("Invalid UTF-8 in binary VDF string")?
        .to_string();
    *cursor += 1; // Consume null byte
    Ok(s)
}

/// Parses Valve's binary KeyValues format used in shortcuts.vdf
/// Spec:
///   0x00 = sub-object (null-terminated name, entries, 0x08 end)
///   0x01 = string (null-terminated name, null-terminated value)
///   0x02 = int32 (null-terminated name, 4 bytes LE)
///   0x03 = float32 (null-terminated name, 4 bytes LE)
///   0x07 = uint64 (null-terminated name, 8 bytes LE)
///   0x08 = end of object
pub fn parse_shortcuts_vdf_bytes(bytes: &[u8]) -> Result<Vec<NonSteamShortcut>> {
    let mut shortcuts = Vec::new();
    let mut cursor = 0;

    if bytes.is_empty() {
        return Ok(shortcuts);
    }

    // Top-level object header: 0x00 followed by "shortcuts\0"
    if bytes[cursor] != 0x00 {
        bail!("Expected 0x00 at start of shortcuts.vdf");
    }
    cursor += 1;
    let root_name = read_null_terminated_string(bytes, &mut cursor)?;
    if !root_name.eq_ignore_ascii_case("shortcuts") {
        bail!(
            "Expected 'shortcuts' root in shortcuts.vdf, got '{}'",
            root_name
        );
    }

    // Read shortcut maps: "0", "1", "2"...
    while cursor < bytes.len() {
        let entry_type = bytes[cursor];
        cursor += 1;
        if entry_type == 0x08 {
            break; // End of root shortcuts map
        }
        if entry_type != 0x00 {
            bail!(
                "Expected sub-object header in shortcuts map at offset {}",
                cursor - 1
            );
        }

        // Sub-object name (e.g. "0", "1")
        let _idx_name = read_null_terminated_string(bytes, &mut cursor)?;

        let mut appid: Option<u32> = None;
        let mut app_name = String::new();
        let mut exe = String::new();

        // Read fields within single shortcut
        while cursor < bytes.len() {
            let field_type = bytes[cursor];
            cursor += 1;
            if field_type == 0x08 {
                break; // End of this shortcut
            }

            let key_name = read_null_terminated_string(bytes, &mut cursor)?;

            match field_type {
                0x00 => {
                    // Nested sub-object (tags, etc.) - skip recursively
                    skip_binary_vdf_subobject(bytes, &mut cursor)?;
                }
                0x01 => {
                    // String field
                    let val = read_null_terminated_string(bytes, &mut cursor)?;
                    if key_name.eq_ignore_ascii_case("appname") {
                        app_name = val;
                    } else if key_name.eq_ignore_ascii_case("exe") {
                        exe = val;
                    }
                }
                0x02 => {
                    // Int32 field
                    if cursor + 4 > bytes.len() {
                        bail!("Unexpected EOF reading Int32 field");
                    }
                    let val = u32::from_le_bytes([
                        bytes[cursor],
                        bytes[cursor + 1],
                        bytes[cursor + 2],
                        bytes[cursor + 3],
                    ]);
                    cursor += 4;
                    if key_name.eq_ignore_ascii_case("appid") {
                        appid = Some(val);
                    }
                }
                0x03 => {
                    // Float32
                    if cursor + 4 > bytes.len() {
                        bail!("Unexpected EOF reading Float32 field");
                    }
                    cursor += 4;
                }
                0x07 => {
                    // Uint64
                    if cursor + 8 > bytes.len() {
                        bail!("Unexpected EOF reading Uint64 field");
                    }
                    cursor += 8;
                }
                other => {
                    bail!(
                        "Unknown field type 0x{:02x} in shortcuts.vdf at offset {}",
                        other,
                        cursor - 1
                    );
                }
            }
        }

        // Steam calculates the compatdata directory name for non-Steam shortcuts using
        // CRC32 of exe + appname OR the direct unsigned 32-bit appid:
        // Empirical observation: Steam compatdata uses (crc32(exe + appname) | 0x80000000)
        // formatted as an unsigned decimal integer string.
        let computed_crc = if !exe.is_empty() && !app_name.is_empty() {
            let key = format!("{}{}", exe, app_name);
            let crc = crc32fast::hash(key.as_bytes()) | 0x8000_0000;
            crc.to_string()
        } else {
            String::new()
        };

        if let Some(id) = appid {
            shortcuts.push(NonSteamShortcut {
                appid: id,
                app_name,
                exe,
                computed_compatdata_id: computed_crc,
            });
        }
    }

    Ok(shortcuts)
}

fn skip_binary_vdf_subobject(bytes: &[u8], cursor: &mut usize) -> Result<()> {
    while *cursor < bytes.len() {
        let field_type = bytes[*cursor];
        *cursor += 1;
        if field_type == 0x08 {
            return Ok(());
        }
        let _key = read_null_terminated_string(bytes, cursor)?;
        match field_type {
            0x00 => skip_binary_vdf_subobject(bytes, cursor)?,
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

/// Discovers and parses all shortcuts.vdf files across all user profiles in all Steam roots.
/// Returns a map of `protected_appid_string -> shortcut_name`.
/// P0-2: If any shortcuts.vdf file exists but fails to parse, this function returns Err
/// and aborts rather than risking data loss.
pub fn discover_non_steam_shortcuts(steam_roots: &[PathBuf]) -> Result<HashMap<String, String>> {
    let mut protected_shortcuts = HashMap::new();

    for root in steam_roots {
        let userdata_dir = root.join("userdata");
        if !userdata_dir.is_dir() {
            continue;
        }

        let user_dirs = match std::fs::read_dir(&userdata_dir) {
            Ok(entries) => entries.flatten().collect::<Vec<_>>(),
            Err(_) => continue,
        };

        for user_entry in user_dirs {
            let shortcuts_vdf = user_entry.path().join("config").join("shortcuts.vdf");
            if shortcuts_vdf.is_file() {
                let bytes = std::fs::read(&shortcuts_vdf).with_context(|| {
                    format!("Failed to read shortcuts.vdf at {:?}", shortcuts_vdf)
                })?;

                let parsed = parse_shortcuts_vdf_bytes(&bytes).with_context(|| {
                    format!(
                        "P0-2 Error: Corrupt or unparseable shortcuts.vdf at {:?}. \
                         Aborting to prevent deleting active non-Steam game prefixes.",
                        shortcuts_vdf
                    )
                })?;

                for sc in parsed {
                    let name = if sc.app_name.is_empty() {
                        "Non-Steam Shortcut".to_string()
                    } else {
                        sc.app_name
                    };

                    // Map direct unsigned appid as string
                    protected_shortcuts.insert(sc.appid.to_string(), name.clone());

                    // Also map computed CRC32 compatdata directory ID
                    if !sc.computed_compatdata_id.is_empty() {
                        protected_shortcuts.insert(sc.computed_compatdata_id, name);
                    }
                }
            }
        }
    }

    Ok(protected_shortcuts)
}

/// Attempts to infer a human-readable title for an uninstalled prefix from its Wine registry or files
pub fn infer_title_from_compatdata(compatdata_dir: &Path) -> Option<String> {
    let user_reg = compatdata_dir.join("pfx").join("user.reg");
    if user_reg.is_file() {
        if let Ok(content) = std::fs::read_to_string(&user_reg) {
            for line in content.lines() {
                if line.starts_with("[Software\\")
                    && !line.contains("Wine")
                    && !line.contains("Microsoft")
                {
                    let trimmed = line.trim_matches(|c| c == '[' || c == ']');
                    let parts: Vec<&str> = trimmed.split('\\').collect();
                    if parts.len() >= 3 {
                        let candidate = parts[2].trim();
                        if !candidate.is_empty() && candidate != "Classes" {
                            return Some(candidate.to_string());
                        }
                    }
                }
            }
        }
    }

    let my_games = compatdata_dir
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("Documents")
        .join("My Games");

    if my_games.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&my_games) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sample_libraryfolders() {
        let sample = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/user/.local/share/Steam"
		"label"		""
		"apps"
		{
			"228980"		"132143724"
			"1343400"		"4007089591"
		}
	}
}
"#;
        let parsed_tree = keyvalues_parser::parse(sample).expect("parse tree");
        let vdf = Vdf::from(parsed_tree);
        let root_obj = match vdf.value {
            Value::Obj(obj) => obj,
            _ => panic!("Expected Obj"),
        };
        assert!(root_obj.contains_key("0"));
    }

    #[test]
    fn test_unreachable_library_aborts() {
        let libs = vec![LibraryFolder {
            path: PathBuf::from("/nonexistent/unmounted/drive"),
            label: "External Drive".to_string(),
            apps: vec!["100".to_string()],
            is_reachable: false,
        }];

        let res = validate_libraries_reachable(&libs);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("unmounted or unreachable"));
    }

    #[test]
    fn test_infrastructure_protection_denylist() {
        assert_eq!(
            get_infrastructure_name("1493710"),
            Some("Proton Experimental")
        );
        assert_eq!(
            get_infrastructure_name("1628350"),
            Some("Steam Linux Runtime 3.0 (sniper)")
        );
        assert_eq!(
            get_infrastructure_name("228980"),
            Some("Steamworks Common Redistributables")
        );
        assert_eq!(get_infrastructure_name("2180100"), Some("Proton Hotfix"));
        assert_eq!(
            get_infrastructure_name("1826330"),
            Some("Proton EasyAntiCheat Runtime")
        );
        assert_eq!(
            get_infrastructure_name("1161040"),
            Some("Proton BattlEye Runtime")
        );
        assert_eq!(get_infrastructure_name("2141910"), None);
        assert_eq!(get_infrastructure_name("489830"), None);
    }

    #[test]
    fn test_parse_binary_shortcuts_vdf() {
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

        let shortcuts = parse_shortcuts_vdf_bytes(&bytes).expect("parse binary shortcuts");
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].appid, 3060000000);
        assert_eq!(shortcuts[0].app_name, "Battle.net");
        assert!(!shortcuts[0].computed_compatdata_id.is_empty());
    }

    #[test]
    fn test_corrupt_shortcuts_vdf_fails() {
        let corrupt_bytes = vec![0x00, b's', b'h', b'o', 0xff, 0x00];
        let res = parse_shortcuts_vdf_bytes(&corrupt_bytes);
        assert!(res.is_err());
    }

    #[test]
    fn test_infer_title_from_mock_user_reg() {
        let temp_dir = std::env::temp_dir().join("prefixpug_test_infer");
        let pfx_dir = temp_dir.join("pfx");
        let _ = std::fs::create_dir_all(&pfx_dir);
        let reg_content = "[Software\\Bethesda\\SkyrimSE]\n\"Installed\"=dword:00000001\n";
        let _ = std::fs::write(pfx_dir.join("user.reg"), reg_content);

        let inferred = infer_title_from_compatdata(&temp_dir);
        assert_eq!(inferred.as_deref(), Some("SkyrimSE"));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_check_steam_cloud_status() {
        let temp_steam = std::env::temp_dir().join("prefixpug_test_cloud");
        let _ = std::fs::remove_dir_all(&temp_steam);

        let user_app = temp_steam.join("userdata").join("12345678").join("777");
        std::fs::create_dir_all(&user_app).unwrap();
        std::fs::write(user_app.join("remotecache.vdf"), b"\"777\" { }").unwrap();

        let roots = [temp_steam.clone()];
        assert_eq!(
            check_steam_cloud_status(&roots, "777"),
            SteamCloudStatus::Synced
        );
        assert_eq!(
            check_steam_cloud_status(&roots, "888"),
            SteamCloudStatus::NotDetected
        );

        let _ = std::fs::remove_dir_all(&temp_steam);
    }
}
