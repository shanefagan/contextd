use super::vdf::{self, Value};
use super::{Game, GameDetector, GameRef, ProcessEnv, home_dirs};
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

    fn find_steam_libraries(&self) -> Vec<PathBuf> {
        let mut all_libraries = Vec::new();
        for user_home in home_dirs() {
            let roots = [
                user_home.join(".local/share/Steam"),
                user_home.join(".steam/steam"),
                user_home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"), // Flatpak
            ];
            for root in roots {
                if root.join("steamapps").exists() {
                    for lib in self.get_library_folders(&root) {
                        // .steam/steam is usually a symlink to the same root.
                        let lib = fs::canonicalize(&lib).unwrap_or(lib);
                        if !all_libraries.contains(&lib) {
                            all_libraries.push(lib);
                        }
                    }
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
        })
    }
}

impl GameDetector for SteamDetector {
    fn name(&self) -> &str {
        "Steam"
    }

    fn list_installed(&self) -> Vec<Game> {
        let mut games: Vec<Game> = Vec::new();
        for library in self.find_steam_libraries() {
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
        games
    }

    fn env_keys(&self) -> &'static [&'static str] {
        &["SteamAppId"]
    }

    fn identify(&self, env: &ProcessEnv) -> Option<GameRef> {
        env.get("SteamAppId")
            .filter(|id| id.as_str() != "0")
            .map(|id| GameRef::Id(id.clone()))
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
