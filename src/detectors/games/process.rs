use super::{Game, GameDetector};

pub struct ProcessDetector;

impl ProcessDetector {
    pub fn new() -> Self {
        Self
    }
}

impl GameDetector for ProcessDetector {
    fn name(&self) -> &str {
        "Process"
    }

    fn list_installed(&self) -> Vec<Game> {
        // Generic process detector doesn't know about "installed" games
        // unless we provide a list of known game binaries to watch for.
        vec![]
    }

    fn list_running(&self) -> Vec<Game> {
        // TODO: Iterate /proc and match against a known database of game process names
        vec![]
    }
}
