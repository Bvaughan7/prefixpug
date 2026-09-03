use anyhow::{bail, Context, Result};
use keyvalues_parser::{Value, Vdf};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LibraryFolder {
    pub path: PathBuf,
    pub label: String,
    pub apps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstalledGame {
    pub appid: String,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: u64,
    pub library_path: PathBuf,
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
                        path: PathBuf::from(path),
                        label,
                        apps,
                    });
                }
            }
        }
    }

    Ok(folders)
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

/// Attempts to infer a human-readable title for an uninstalled prefix from its Wine registry or files
pub fn infer_title_from_compatdata(compatdata_dir: &Path) -> Option<String> {
    let user_reg = compatdata_dir.join("pfx").join("user.reg");
    if user_reg.is_file() {
        if let Ok(content) = std::fs::read_to_string(&user_reg) {
            for line in content.lines() {
                // Look for [Software\\Publisher\\GameName] or similar registry paths
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

    // Secondary heuristic: check for game directories under Saved Games or My Games
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
}
