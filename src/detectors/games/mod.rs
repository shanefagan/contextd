pub use crate::contextd::Game;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod heroic;
pub mod lutris;
pub mod manager;
pub mod steam;
pub mod vdf;

/// Environment variables read from one process, limited to the keys the
/// detectors asked for.
pub type ProcessEnv = HashMap<&'static str, String>;

/// How a running process refers to a game.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameRef {
    /// Matches `Game.id`
    Id(String),
    /// Matches `Game.name`
    Name(String),
}

pub trait GameDetector: Send + Sync {
    /// Unique name of the detector (e.g., "Steam", "Lutris")
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Returns all games installed through this launcher.
    fn list_installed(&self) -> Vec<Game>;

    /// Environment variables this launcher sets on the processes it starts.
    fn env_keys(&self) -> &'static [&'static str];

    /// Identifies the game a process belongs to from its environment.
    /// `env` contains only the variables from `env_keys` the process has.
    fn identify(&self, env: &ProcessEnv) -> Option<GameRef>;
}

/// Home directories to scan for launcher data.
pub fn home_dirs() -> Vec<PathBuf> {
    std::fs::read_dir("/home")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}
