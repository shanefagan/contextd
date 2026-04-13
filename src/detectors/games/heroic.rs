use super::{Game, GameDetector};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde_json::Value;

pub struct HeroicDetector;

impl HeroicDetector {
    pub fn new() -> Self {
        Self
    }

    fn find_heroic_config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(entries) = fs::read_dir("/home") {
            for entry in entries.flatten() {
                let user_home = entry.path();
                // Native/AppImage path
                let native = user_home.join(".config");
                if native.exists() {
                    paths.push(native);
                }
                // Flatpak path
                let flatpak = user_home.join(".var/app/com.heroicgameslauncher.hgl/config");
                if flatpak.exists() {
                    paths.push(flatpak);
                }
            }
        }
        paths
    }

    fn parse_installed_json(&self, path: &Path, source: &str) -> Vec<Game> {
        let mut games = Vec::new();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = v.as_object() {
                    for (id, data) in obj {
                        let name = data["title"].as_str() 
                            .or_else(|| data["app_name"].as_str())
                            .unwrap_or(id);
                            
                        games.push(Game {
                            name: name.to_string(),
                            id: Some(id.clone()),
                            source: source.to_string(),
                            pid: None,
                        });
                    }
                }
            }
        }
        games
    }
}

impl GameDetector for HeroicDetector {
    fn name(&self) -> &str {
        "Heroic"
    }

    fn list_installed(&self) -> Vec<Game> {
        let mut games = Vec::new();
        for config_root in self.find_heroic_config_paths() {
            // Epic (Legendary)
            let epic_path = config_root.join("legendary/installed.json");
            games.extend(self.parse_installed_json(&epic_path, "Heroic (Epic)"));

            // GOG
            let gog_path = config_root.join("heroic/gog_store/installed.json");
            games.extend(self.parse_installed_json(&gog_path, "Heroic (GOG)"));

            // Amazon
            let amazon_path = config_root.join("heroic/nile_store/installed.json");
            games.extend(self.parse_installed_json(&amazon_path, "Heroic (Amazon)"));
        }
        
        // Deduplicate
        games.sort_by_key(|g| g.id.clone());
        games.dedup_by(|a, b| a.id == b.id);
        
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
                let pid = entry.file_name();
                let pid_str = pid.to_string_lossy();
                if pid_str.chars().all(|c| c.is_ascii_digit()) {
                    let environ_path = entry.path().join("environ");
                    if let Ok(environ) = fs::read(environ_path) {
                        for env_var in environ.split(|&b| b == 0) {
                            let var_str = String::from_utf8_lossy(env_var);
                            // Heroic sets HEROIC_APP_NAME
                            if var_str.starts_with("HEROIC_APP_NAME=") {
                                if let Some(id) = var_str.split('=').nth(1) {
                                    if let Some(game) = id_map.get(id) {
                                        let mut running_game = game.clone();
                                        running_game.pid = pid_str.parse().ok();
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
