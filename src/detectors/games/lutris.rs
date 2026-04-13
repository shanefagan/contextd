use super::{Game, GameDetector};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use rusqlite::{Connection, OpenFlags};

pub struct LutrisDetector;

impl LutrisDetector {
    pub fn new() -> Self {
        Self
    }

    fn find_lutris_db_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(entries) = fs::read_dir("/home") {
            for entry in entries.flatten() {
                let user_home = entry.path();
                // Native path
                let native = user_home.join(".local/share/lutris/pga.db");
                if native.exists() {
                    paths.push(native);
                }
                // Flatpak path
                let flatpak = user_home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db");
                if flatpak.exists() {
                    paths.push(flatpak);
                }
            }
        }
        paths
    }

    fn query_db(&self, path: &Path) -> Vec<Game> {
        let mut games = Vec::new();
        // Open as RO to avoid locking issues with running Lutris
        if let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            let stmt = conn.prepare("SELECT name, slug, installed FROM games WHERE installed = 1").ok();
            if let Some(mut s) = stmt {
                let rows = s.query_map([], |row| {
                    Ok(Game {
                        name: row.get(0)?,
                        id: Some(row.get(1)?),
                        source: "Lutris".to_string(),
                        pid: None,
                    })
                }).ok();

                if let Some(r) = rows {
                    for game in r.flatten() {
                        games.push(game);
                    }
                }
            }
        }
        games
    }
}

impl GameDetector for LutrisDetector {
    fn name(&self) -> &str {
        "Lutris"
    }

    fn list_installed(&self) -> Vec<Game> {
        let mut games = Vec::new();
        for db_path in self.find_lutris_db_paths() {
            games.extend(self.query_db(&db_path));
        }
        // Deduplicate
        games.sort_by_key(|g| g.id.clone());
        games.dedup_by(|a, b| a.id == b.id);
        games
    }

    fn list_running(&self) -> Vec<Game> {
        let mut running = Vec::new();
        let installed = self.list_installed();
        let slug_map: HashMap<String, Game> = installed.into_iter()
            .filter_map(|g| g.id.clone().map(|slug| (slug, g)))
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
                            // Lutris often sets variables like LUTRIS_GAME_SLUG
                            if var_str.starts_with("LUTRIS_GAME_SLUG=") {
                                if let Some(slug) = var_str.split('=').nth(1) {
                                    if let Some(game) = slug_map.get(slug) {
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
