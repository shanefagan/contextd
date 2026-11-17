use super::vdf::{self, Value};
use super::{Game, GameDetector, GameRef, Installed, ProcessEnv, home_dirs};
use std::fs;
use std::path::{Path, PathBuf};

/// Steam app IDs that are tools rather than games.
const TOOL_APP_IDS: &[&str] = &[
    "228980",  // Steamworks Common Redistributables
    "1070560", // Steam Linux Runtime 1.0 (scout)
    "1391110", // Steam Linux Runtime 2.0 (soldier)
    "1628350", // Steam Linux Runtime 3.0 (sniper)
];

/// Proton builds, runtimes and redistributables have install manifests
/// like games do. New Proton versions get new app IDs, so match on name.
fn is_tool(id: &str, name: &str) -> bool {
    TOOL_APP_IDS.contains(&id)
        || name.starts_with("Proton ")
        || name.starts_with("Steam Linux Runtime")
        || name == "Steamworks Common Redistributables"
}

pub struct SteamDetector;

impl SteamDetector {
    pub fn new() -> Self {
        Self
    }

    /// Steam installation roots (native, legacy symlink and Flatpak).
    fn steam_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();
        for user_home in home_dirs() {
            for root in [
                user_home.join(".local/share/Steam"),
                user_home.join(".steam/steam"),
                user_home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            ] {
                if root.join("steamapps").exists() {
                    // .steam/steam is usually a symlink to the same root.
                    let root = fs::canonicalize(&root).unwrap_or(root);
                    if !roots.contains(&root) {
                        roots.push(root);
                    }
                }
            }
        }
        roots
    }

    fn find_steam_libraries(&self, roots: &[PathBuf]) -> Vec<PathBuf> {
        let mut all_libraries = Vec::new();
        for root in roots {
            for lib in self.get_library_folders(root) {
                let lib = fs::canonicalize(&lib).unwrap_or(lib);
                if !all_libraries.contains(&lib) {
                    all_libraries.push(lib);
                }
            }
        }
        all_libraries
    }

    fn get_library_folders(&self, steam_path: &Path) -> Vec<PathBuf> {
        let mut folders = vec![steam_path.to_path_buf()];
        let vdf_path = steam_path.join("steamapps/libraryfolders.vdf");
        let Some(doc) = fs::read_to_string(vdf_path)
            .ok()
            .and_then(|c| vdf::parse(&c))
        else {
            return folders;
        };
        let Some(root) = doc.get("libraryfolders") else {
            return folders;
        };
        for (_, folder) in root.children() {
            if let Some(path) = folder.get("path").and_then(Value::as_str) {
                let path = PathBuf::from(path);
                if path.exists() && !folders.contains(&path) {
                    folders.push(path);
                }
            }
        }
        folders
    }

    fn parse_acf(&self, path: &Path) -> Option<Game> {
        let doc = vdf::parse(&fs::read_to_string(path).ok()?)?;
        let fields = vdf::string_fields(&doc, "AppState");
        let id = fields.get("appid")?.clone();
        let name = fields.get("name")?.clone();
        if is_tool(&id, &name) {
            return None;
        }
        Some(Game {
            name,
            id: Some(id),
            source: "Steam".to_string(),
            pid: None,
            focused: None,
        })
    }

    /// Non-Steam games added to the library, from every Steam user's
    /// `userdata/<id>/config/shortcuts.vdf`.
    fn shortcuts(&self, roots: &[PathBuf]) -> Vec<Game> {
        let mut games = Vec::new();
        for root in roots {
            let Ok(users) = fs::read_dir(root.join("userdata")) else {
                continue;
            };
            for user in users.flatten() {
                let path = user.path().join("config/shortcuts.vdf");
                if let Ok(data) = fs::read(&path) {
                    games.extend(parse_shortcuts(&data));
                }
            }
        }
        games
    }
}

/// Games listed in a binary `shortcuts.vdf`. The id is the shortcut's
/// 32-bit app ID as an unsigned number, which is how Steam names its
/// artwork and Proton prefix for it.
pub fn parse_shortcuts(data: &[u8]) -> Vec<Game> {
    let Some(doc) = vdf::parse_binary(data) else {
        return Vec::new();
    };
    let Some(list) = doc.get("shortcuts") else {
        return Vec::new();
    };
    list.children()
        .iter()
        .filter_map(|(_, entry)| {
            let app_id = entry.get("appid")?.as_int()? as u32;
            let name = entry.get("appname")?.as_str()?.trim().to_string();
            (!name.is_empty()).then(|| Game {
                name,
                id: Some(app_id.to_string()),
                source: "Steam (Shortcut)".to_string(),
                pid: None,
                focused: None,
            })
        })
        .collect()
}

/// The game a Steam-launched process belongs to.
///
/// Steam games carry `SteamAppId`. Non-Steam shortcuts run with
/// `SteamAppId=0`; their 32-bit app ID is the top half of the 64-bit
/// `SteamGameId` (`app_id << 32 | 0x02000000`), and Proton shortcuts also
/// name their prefix after it (`.../compatdata/<app_id>`).
fn identify_steam(env: &ProcessEnv) -> Option<String> {
    if let Some(id) = env.get("SteamAppId").filter(|id| id.as_str() != "0") {
        return Some(id.clone());
    }
    if let Some(game_id) = env.get("SteamGameId").and_then(|g| g.parse::<u64>().ok()) {
        if game_id > u32::MAX as u64 {
            return Some((game_id >> 32).to_string());
        }
        if game_id != 0 {
            return Some(game_id.to_string());
        }
    }
    env.get("STEAM_COMPAT_DATA_PATH")
        .and_then(|p| {
            Path::new(p.trim_end_matches('/'))
                .file_name()?
                .to_str()?
                .parse::<u32>()
                .ok()
        })
        .filter(|id| *id != 0)
        .map(|id| id.to_string())
}

impl GameDetector for SteamDetector {
    fn name(&self) -> &str {
        "Steam"
    }

    fn list_installed(&self) -> Vec<Installed> {
        let roots = self.steam_roots();
        let mut games: Vec<Game> = Vec::new();
        for library in self.find_steam_libraries(&roots) {
            let Ok(entries) = fs::read_dir(library.join("steamapps")) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("acf")
                    && let Some(game) = self.parse_acf(&path)
                    && !games.iter().any(|g| g.id == game.id)
                {
                    games.push(game);
                }
            }
        }
        for game in self.shortcuts(&roots) {
            if !games.iter().any(|g| g.id == game.id) {
                games.push(game);
            }
        }
        games.into_iter().map(Installed::plain).collect()
    }

    fn env_keys(&self) -> &'static [&'static str] {
        &["SteamAppId", "SteamGameId", "STEAM_COMPAT_DATA_PATH"]
    }

    fn identify(&self, env: &ProcessEnv) -> Option<GameRef> {
        identify_steam(env).map(GameRef::Id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_tools() {
        assert!(is_tool("1493710", "Proton Experimental"));
        assert!(is_tool("2805730", "Proton 9.0"));
        assert!(is_tool("1628350", "Steam Linux Runtime 3.0 (sniper)"));
        assert!(is_tool("228980", "Steamworks Common Redistributables"));
        assert!(!is_tool("1245620", "ELDEN RING"));
        assert!(!is_tool("1", "Protonwar"));
    }

    fn env(pairs: &[(&'static str, &str)]) -> ProcessEnv {
        pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
    }

    #[test]
    fn identifies_steam_games_and_shortcuts() {
        assert_eq!(
            identify_steam(&env(&[("SteamAppId", "1245620")])).as_deref(),
            Some("1245620")
        );
        // Shortcut: app ID 3733884208 -> game ID (3733884208 << 32) | 0x02000000
        let game_id = ((3_733_884_208u64) << 32 | 0x0200_0000).to_string();
        assert_eq!(
            identify_steam(&env(&[("SteamAppId", "0"), ("SteamGameId", &game_id)])).as_deref(),
            Some("3733884208")
        );
        assert_eq!(
            identify_steam(&env(&[
                ("SteamAppId", "0"),
                (
                    "STEAM_COMPAT_DATA_PATH",
                    "/home/a/.local/share/Steam/steamapps/compatdata/3733884208/"
                )
            ]))
            .as_deref(),
            Some("3733884208")
        );
        assert_eq!(identify_steam(&env(&[("SteamAppId", "0")])), None);
    }

    #[test]
    fn reads_shortcuts_vdf() {
        let mut d = Vec::new();
        d.extend(b"\x00shortcuts\x00\x000\x00\x02appid\x00");
        d.extend(&(3_733_884_208u32 as i32).to_le_bytes());
        d.extend(b"\x01AppName\x00Dolphin\x00\x01Exe\x00\"/usr/bin/dolphin-emu\"\x00\x08\x08\x08");
        let games = parse_shortcuts(&d);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Dolphin");
        assert_eq!(games[0].id.as_deref(), Some("3733884208"));
        assert!(parse_shortcuts(b"garbage").is_empty());
    }

    #[test]
    fn reads_library_folders_and_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Steam");
        let extra = dir.path().join("Extra Library");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::create_dir_all(extra.join("steamapps")).unwrap();
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
                root.display(),
                extra.display()
            ),
        )
        .unwrap();
        fs::write(
            extra.join("steamapps/appmanifest_1.acf"),
            "\"AppState\" { \"appid\" \"1\" \"name\" \"Game One\" }",
        )
        .unwrap();
        fs::write(
            extra.join("steamapps/appmanifest_2.acf"),
            "\"AppState\" { \"appid\" \"2805730\" \"name\" \"Proton 9.0\" }",
        )
        .unwrap();

        let d = SteamDetector::new();
        let libs = d.get_library_folders(&root);
        assert_eq!(libs, vec![root.clone(), extra.clone()]);
        assert_eq!(
            d.parse_acf(&extra.join("steamapps/appmanifest_1.acf"))
                .unwrap()
                .name,
            "Game One"
        );
        assert!(
            d.parse_acf(&extra.join("steamapps/appmanifest_2.acf"))
                .is_none()
        );
    }
}
