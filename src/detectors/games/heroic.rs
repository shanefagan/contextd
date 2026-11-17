//! Heroic Games Launcher (Epic via Legendary, GOG, Amazon via Nile).
//!
//! Each store keeps its installed list in a different format:
//!
//! - Epic: `legendaryConfig/legendary/installed.json` (or the standalone
//!   `~/.config/legendary/installed.json`), an object keyed by app name
//!   with a `title` field.
//! - GOG: `gog_store/installed.json`, `{"installed": [{"appName": ...}]}`,
//!   titles in `store_cache/gog_library.json` (`{"games": [...]}`).
//! - Amazon: `nile_config/nile/installed.json`, `[{"id": ..., "path": ...}]`,
//!   titles in `store_cache/nile_library.json` (`{"library": [...]}`).
//!
//! Heroic sets `HEROIC_APP_NAME` to the same app name on game processes.

use super::{Game, GameDetector, GameRef, Installed, ProcessEnv, home_dirs};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct HeroicDetector;

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// app_name -> title from a Heroic store cache file.
fn titles_from_cache(path: &Path, key: &str) -> HashMap<String, String> {
    read_json(path)
        .and_then(|v| v.get(key).and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|g| {
            Some((
                g.get("app_name")?.as_str()?.to_string(),
                g.get("title")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

/// The install folder name is the best title we have without a cache.
fn folder_name(path: Option<&str>) -> Option<String> {
    Path::new(path?)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

fn game(id: &str, name: String, source: &str) -> Game {
    Game {
        name,
        id: Some(id.to_string()),
        source: source.to_string(),
        pid: None,
    }
}

/// Epic: `{ "<app_name>": { "title": ..., ... }, ... }`
pub fn parse_legendary(v: &Value) -> Vec<Game> {
    v.as_object()
        .into_iter()
        .flatten()
        .map(|(id, data)| {
            let name = data
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(id)
                .to_string();
            game(id, name, "Heroic (Epic)")
        })
        .collect()
}

/// GOG: `{ "installed": [ { "appName": ..., "install_path": ... } ] }`
pub fn parse_gog(v: &Value, titles: &HashMap<String, String>) -> Vec<Game> {
    v.get("installed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let id = g.get("appName")?.as_str()?;
            let name = titles
                .get(id)
                .cloned()
                .or_else(|| folder_name(g.get("install_path").and_then(Value::as_str)))
                .unwrap_or_else(|| id.to_string());
            Some(game(id, name, "Heroic (GOG)"))
        })
        .collect()
}

/// Amazon: `[ { "id": ..., "path": ... } ]`
pub fn parse_nile(v: &Value, titles: &HashMap<String, String>) -> Vec<Game> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let id = g.get("id")?.as_str()?;
            let name = titles
                .get(id)
                .cloned()
                .or_else(|| folder_name(g.get("path").and_then(Value::as_str)))
                .unwrap_or_else(|| id.to_string());
            Some(game(id, name, "Heroic (Amazon)"))
        })
        .collect()
}

impl HeroicDetector {
    pub fn new() -> Self {
        Self
    }

    /// Heroic config roots (native and Flatpak) per user.
    fn heroic_roots(&self) -> Vec<(PathBuf, PathBuf)> {
        let mut roots = Vec::new();
        for home in home_dirs() {
            for config in [
                home.join(".config"),
                home.join(".var/app/com.heroicgameslauncher.hgl/config"),
            ] {
                if config.join("heroic").is_dir() || config.join("legendary").is_dir() {
                    roots.push((config.clone(), config.join("heroic")));
                }
            }
        }
        roots
    }
}

impl GameDetector for HeroicDetector {
    fn name(&self) -> &str {
        "Heroic"
    }

    fn list_installed(&self) -> Vec<Installed> {
        let mut games = Vec::new();
        for (config, heroic) in self.heroic_roots() {
            for legendary in [
                heroic.join("legendaryConfig/legendary/installed.json"),
                config.join("legendary/installed.json"),
            ] {
                if let Some(v) = read_json(&legendary) {
                    games.extend(parse_legendary(&v));
                }
            }

            if let Some(v) = read_json(&heroic.join("gog_store/installed.json")) {
                let titles =
                    titles_from_cache(&heroic.join("store_cache/gog_library.json"), "games");
                games.extend(parse_gog(&v, &titles));
            }

            if let Some(v) = read_json(&heroic.join("nile_config/nile/installed.json")) {
                let titles =
                    titles_from_cache(&heroic.join("store_cache/nile_library.json"), "library");
                games.extend(parse_nile(&v, &titles));
            }
        }

        games.sort_by(|a, b| (&a.source, &a.id).cmp(&(&b.source, &b.id)));
        games.dedup_by(|a, b| a.id == b.id && a.source == b.source);
        games.into_iter().map(Installed::plain).collect()
    }

    fn env_keys(&self) -> &'static [&'static str] {
        &["HEROIC_APP_NAME"]
    }

    fn identify(&self, env: &ProcessEnv) -> Option<GameRef> {
        env.get("HEROIC_APP_NAME")
            .filter(|n| !n.is_empty())
            .map(|n| GameRef::Id(n.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_legendary() {
        let v = json!({"Fortnite": {"title": "Fortnite", "app_name": "Fortnite"}, "x": {}});
        let games = parse_legendary(&v);
        assert_eq!(games.len(), 2);
        assert!(games.iter().any(|g| g.name == "Fortnite"));
    }

    #[test]
    fn parses_gog_list_not_object() {
        let v = json!({"installed": [
            {"appName": "1207658924", "install_path": "/games/Unreal Gold"},
            {"appName": "1", "install_path": "/games/Other"}
        ]});
        let titles = HashMap::from([("1".to_string(), "Cached Title".to_string())]);
        let games = parse_gog(&v, &titles);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].name, "Unreal Gold");
        assert_eq!(games[1].name, "Cached Title");
        assert!(games.iter().all(|g| g.id.as_deref() != Some("installed")));
    }

    #[test]
    fn parses_nile_array() {
        let v =
            json!([{"id": "amzn1.adg.product.abc", "path": "/games/Some Game", "version": "1"}]);
        let games = parse_nile(&v, &HashMap::new());
        assert_eq!(games[0].name, "Some Game");
        assert_eq!(games[0].source, "Heroic (Amazon)");
    }
}
