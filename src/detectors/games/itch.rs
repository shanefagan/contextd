//! itch.io app.
//!
//! The itch app keeps its library in butler's SQLite database
//! (`~/.config/itch/db/butler.db`): `caves` are installed games, each in
//! an install location plus folder (or a custom folder). itch sets no
//! environment variable on games, so a running game is a process whose
//! executable, or for Windows games whose working directory, is inside the
//! install folder.

use super::{Game, GameDetector, Installed, MatchHint, home_dirs};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub struct ItchDetector;

impl ItchDetector {
    pub fn new() -> Self {
        Self
    }

    fn db_paths(&self) -> Vec<PathBuf> {
        home_dirs()
            .into_iter()
            .flat_map(|home| {
                [
                    home.join(".config/itch/db/butler.db"),
                    home.join(".var/app/io.itch.itch/config/itch/db/butler.db"),
                ]
            })
            .filter(|p| p.is_file())
            .collect()
    }
}

/// Installed games in a butler database. Any schema mismatch yields an
/// empty list rather than an error: the database belongs to another app.
pub fn read_caves(db: &Path) -> Vec<Installed> {
    let Ok(conn) = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };
    let query = "SELECT c.game_id, g.title, l.path, c.install_folder_name, c.custom_install_folder
                 FROM caves c
                 LEFT JOIN games g ON g.id = c.game_id
                 LEFT JOIN install_locations l ON l.id = c.install_location_id";
    let mut stmt = match conn.prepare(query) {
        Ok(s) => s,
        Err(e) => {
            log::debug!("Unexpected itch database schema in {}: {}", db.display(), e);
            return Vec::new();
        }
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    });
    let Ok(rows) = rows else {
        return Vec::new();
    };

    rows.flatten()
        .filter_map(|(game_id, title, location, folder, custom)| {
            let dir = match custom.filter(|c| !c.is_empty()) {
                Some(custom) => PathBuf::from(custom),
                None => PathBuf::from(location?).join(folder.as_deref()?),
            };
            let name = title
                .filter(|t| !t.is_empty())
                .or(folder)
                .unwrap_or_else(|| game_id.to_string());
            Some(Installed {
                game: Game {
                    name,
                    id: Some(game_id.to_string()),
                    source: "itch.io".to_string(),
                    pid: None,
                },
                hint: MatchHint::InstallDir(dir),
            })
        })
        .collect()
}

impl GameDetector for ItchDetector {
    fn name(&self) -> &str {
        "itch.io"
    }

    fn list_installed(&self) -> Vec<Installed> {
        self.db_paths().iter().flat_map(|p| read_caves(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_caves_with_locations_and_custom_folders() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("butler.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE games (id INTEGER PRIMARY KEY, title TEXT);
             CREATE TABLE install_locations (id TEXT PRIMARY KEY, path TEXT);
             CREATE TABLE caves (id TEXT, game_id INTEGER, install_location_id TEXT,
                                 install_folder_name TEXT, custom_install_folder TEXT);
             INSERT INTO games VALUES (1, 'Celeste'), (2, 'Baba Is You');
             INSERT INTO install_locations VALUES ('loc', '/home/a/.config/itch/apps');
             INSERT INTO caves VALUES ('c1', 1, 'loc', 'celeste', NULL),
                                      ('c2', 2, '', '', '/games/baba');",
        )
        .unwrap();
        drop(conn);

        let games = read_caves(&db);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].game.name, "Celeste");
        assert_eq!(
            games[0].hint,
            MatchHint::InstallDir("/home/a/.config/itch/apps/celeste".into())
        );
        assert_eq!(games[1].hint, MatchHint::InstallDir("/games/baba".into()));
    }

    #[test]
    fn unexpected_schema_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("butler.db");
        Connection::open(&db)
            .unwrap()
            .execute_batch("CREATE TABLE other (x INTEGER);")
            .unwrap();
        assert!(read_caves(&db).is_empty());
    }
}
