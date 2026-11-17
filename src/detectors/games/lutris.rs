use super::{Game, GameDetector, GameRef, Installed, ProcessEnv, home_dirs};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub struct LutrisDetector;

impl LutrisDetector {
    pub fn new() -> Self {
        Self
    }

    fn find_lutris_db_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for user_home in home_dirs() {
            for candidate in [
                user_home.join(".local/share/lutris/pga.db"),
                user_home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db"),
            ] {
                if candidate.exists() {
                    paths.push(candidate);
                }
            }
        }
        paths
    }

    fn query_db(&self, path: &Path) -> Vec<Game> {
        // Read-only, so a running Lutris is never blocked by our lock.
        let Ok(conn) = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare("SELECT name, slug FROM games WHERE installed = 1") else {
            return Vec::new();
        };
        stmt.query_map([], |row| {
            Ok(Game {
                name: row.get(0)?,
                id: Some(row.get(1)?),
                source: "Lutris".to_string(),
                pid: None,
                focused: None,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }
}

impl GameDetector for LutrisDetector {
    fn name(&self) -> &str {
        "Lutris"
    }

    fn list_installed(&self) -> Vec<Installed> {
        let mut games: Vec<Game> = self
            .find_lutris_db_paths()
            .iter()
            .flat_map(|p| self.query_db(p))
            .collect();
        games.sort_by(|a, b| a.id.cmp(&b.id));
        games.dedup_by(|a, b| a.id == b.id);
        games.into_iter().map(Installed::plain).collect()
    }

    fn env_keys(&self) -> &'static [&'static str] {
        &["LUTRIS_GAME_UUID", "GAME_NAME"]
    }

    /// Lutris tags game processes with a random per-launch
    /// `LUTRIS_GAME_UUID` and the game's display name in `GAME_NAME`.
    /// `GAME_NAME` alone is too generic, so both must be present.
    fn identify(&self, env: &ProcessEnv) -> Option<GameRef> {
        env.get("LUTRIS_GAME_UUID")?;
        env.get("GAME_NAME").map(|n| GameRef::Name(n.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_both_variables() {
        let d = LutrisDetector::new();
        let mut env = ProcessEnv::new();
        env.insert("GAME_NAME", "Diablo II".into());
        assert_eq!(d.identify(&env), None);
        env.insert(
            "LUTRIS_GAME_UUID",
            "1b4e28ba-2fa1-11d2-883f-0016d3cca427".into(),
        );
        assert_eq!(d.identify(&env), Some(GameRef::Name("Diablo II".into())));
    }

    #[test]
    fn reads_installed_games() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("pga.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE games (name TEXT, slug TEXT, installed INTEGER);
             INSERT INTO games VALUES ('Diablo II', 'diablo-ii', 1), ('Old', 'old', 0);",
        )
        .unwrap();
        drop(conn);
        let games = LutrisDetector::new().query_db(&db);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id.as_deref(), Some("diablo-ii"));
    }
}
