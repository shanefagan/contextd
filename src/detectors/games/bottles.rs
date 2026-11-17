//! Bottles (Wine prefix manager).
//!
//! Each bottle is a directory with a `bottle.yml` whose `External_Programs`
//! lists the programs the user added, with their Windows executable name.
//! Bottles sets no variable of its own on what it launches, but every
//! process in a bottle has `WINEPREFIX` set to the bottle directory, so a
//! game is the process in that prefix whose executable matches a program.

use super::{Game, GameDetector, Installed, MatchHint, home_dirs, windows_file_name};
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust2::{Yaml, YamlLoader};

pub struct BottlesDetector;

impl BottlesDetector {
    pub fn new() -> Self {
        Self
    }

    fn bottle_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for home in home_dirs() {
            for root in [
                home.join(".local/share/bottles/bottles"),
                home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
            ] {
                if let Ok(entries) = fs::read_dir(&root) {
                    dirs.extend(
                        entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| p.join("bottle.yml").is_file()),
                    );
                }
            }
        }
        dirs
    }
}

/// Programs in one bottle, given its directory and `bottle.yml` contents.
pub fn parse_bottle(dir: &Path, config: &str) -> Vec<Installed> {
    let Ok(docs) = YamlLoader::load_from_str(config) else {
        return Vec::new();
    };
    let Some(doc) = docs.first() else {
        return Vec::new();
    };
    let bottle = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(programs) = doc["External_Programs"].as_hash() else {
        return Vec::new();
    };

    programs
        .iter()
        .filter_map(|(key, program)| {
            if program["removed"].as_bool() == Some(true) {
                return None;
            }
            let exe = program["executable"]
                .as_str()
                .filter(|e| !e.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    program["path"]
                        .as_str()
                        .map(|p| windows_file_name(p).to_string())
                })?;
            let name = program["name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| exe.trim_end_matches(".exe").to_string());
            let program_id = program["id"]
                .as_str()
                .or(match key {
                    Yaml::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or(&exe)
                .to_string();
            Some(Installed {
                game: Game {
                    name,
                    id: Some(format!("{bottle}/{program_id}")),
                    source: "Bottles".to_string(),
                    pid: None,
                    focused: None,
                },
                hint: MatchHint::WineProgram {
                    prefix: dir.to_path_buf(),
                    exe,
                },
            })
        })
        .collect()
}

impl GameDetector for BottlesDetector {
    fn name(&self) -> &str {
        "Bottles"
    }

    fn list_installed(&self) -> Vec<Installed> {
        self.bottle_dirs()
            .iter()
            .filter_map(|dir| {
                let config = fs::read_to_string(dir.join("bottle.yml")).ok()?;
                Some(parse_bottle(dir, &config))
            })
            .flatten()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_external_programs() {
        let yml = r#"
Name: Gaming
Arch: win64
External_Programs:
  0f2c-uuid:
    arguments: ''
    executable: Game.exe
    name: The Game
    folder: C:\Games\The Game
    id: 0f2c-uuid
    path: C:\Games\The Game\Game.exe
  9a1b-uuid:
    executable: ''
    name: ''
    path: C:\Tools\tool.exe
  dead-uuid:
    executable: Old.exe
    name: Old
    removed: true
"#;
        let dir = Path::new("/home/a/.local/share/bottles/bottles/Gaming");
        let games = parse_bottle(dir, yml);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].game.name, "The Game");
        assert_eq!(games[0].game.id.as_deref(), Some("Gaming/0f2c-uuid"));
        assert_eq!(
            games[0].hint,
            MatchHint::WineProgram {
                prefix: dir.to_path_buf(),
                exe: "Game.exe".into()
            }
        );
        assert_eq!(
            games[1].game.name, "tool",
            "falls back to the path's file name"
        );
    }

    #[test]
    fn bad_yaml_is_empty() {
        assert!(parse_bottle(Path::new("/x"), "::: not yaml").is_empty());
        assert!(parse_bottle(Path::new("/x"), "Name: x\n").is_empty());
    }
}
