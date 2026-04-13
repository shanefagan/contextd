use super::{Game, GameDetector};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SteamDetector;

impl SteamDetector {
    pub fn new() -> Self {
        Self
    }

    fn find_steam_libraries(&self) -> Vec<PathBuf> {
        let mut all_libraries = Vec::new();
        
        // Scan /home for all users to find Steam installations
        if let Ok(entries) = fs::read_dir("/home") {
            for entry in entries.flatten() {
                let user_home = entry.path();
                
                // Potential Steam roots
                let roots = vec![
                    user_home.join(".local/share/Steam"),
                    user_home.join(".steam/steam"),
                    user_home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"), // Flatpak
                ];

                for root in roots {
                    if root.join("steamapps").exists() {
                        // Found a Steam root, now find its libraries
                        for lib in self.get_library_folders(&root) {
                            if !all_libraries.contains(&lib) {
                                all_libraries.push(lib);
                            }
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
        
        if let Ok(content) = fs::read_to_string(vdf_path) {
            // Basic VDF parser
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("\"path\"") {
                    let parts: Vec<&str> = line.split('"').filter(|s| !s.trim().is_empty()).collect();
                    if parts.len() >= 2 {
                        let path = PathBuf::from(parts[1]);
                        if path.exists() && !folders.contains(&path) {
                            folders.push(path);
                        }
                    }
                }
            }
        }
        
        folders
    }

    fn parse_acf(&self, path: &Path) -> Option<Game> {
        let content = fs::read_to_string(path).ok()?;
        let mut appid = None;
        let mut name = None;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("\"appid\"") || line.starts_with("\"name\"") {
                let parts: Vec<&str> = line.split('"').filter(|s| !s.trim().is_empty()).collect();
                if parts.len() >= 2 {
                    if parts[0] == "appid" {
                        appid = Some(parts[1].to_string());
                    } else if parts[0] == "name" {
                        name = Some(parts[1].to_string());
                    }
                }
            }
        }

        if let (Some(id), Some(n)) = (appid, name) {
            Some(Game {
                name: n,
                id: Some(id),
                source: "Steam".to_string(),
                pid: None,
            })
        } else {
            None
        }
    }
}

impl GameDetector for SteamDetector {
    fn name(&self) -> &str {
        "Steam"
    }

    fn list_installed(&self) -> Vec<Game> {
        let mut games = Vec::new();
        let libraries = self.find_steam_libraries();
        
        for library in libraries {
            let steamapps = library.join("steamapps");
            if let Ok(entries) = fs::read_dir(steamapps) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("acf") {
                        if let Some(ref game) = self.parse_acf(&path) {
                            // Deduplicate by AppID
                            if !games.iter().any(|g: &Game| g.id == game.id) {
                                games.push(game.clone());
                            }
                        }
                    }
                }
            }
        }
        games
    }

    fn list_running(&self) -> Vec<Game> {
        let mut running = Vec::new();
        let installed = self.list_installed();
        let id_map: HashMap<String, Game> = installed.into_iter()
            .filter_map(|g| g.id.clone().map(|id| (id, g)))
            .collect();

        if let Ok(proc_entries) = fs::read_dir("/proc") {
            for entry in proc_entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.chars().all(|c| c.is_ascii_digit()) {
                    let environ_path = entry.path().join("environ");
                    // Root can read any process's environ
                    if let Ok(environ) = fs::read(environ_path) {
                        for env_var in environ.split(|&b| b == 0) {
                            let var_str = String::from_utf8_lossy(env_var);
                            if var_str.starts_with("SteamAppId=") {
                                if let Some(id) = var_str.split('=').nth(1) {
                                    if let Some(game) = id_map.get(id) {
                                        let mut running_game = game.clone();
                                        running_game.pid = name_str.parse().ok();
                                        running.push(running_game);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        running.sort_by_key(|g| g.id.clone());
        running.dedup_by(|a, b| a.id == b.id);
        
        running
    }
}
