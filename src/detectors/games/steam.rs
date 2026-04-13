use super::{Game, GameDetector};

pub struct SteamDetector;

impl SteamDetector {
    pub fn new() -> Self {
        Self
    }
}

impl GameDetector for SteamDetector {
    fn name(&self) -> &str {
        "Steam"
    }

    fn list_installed(&self) -> Vec<Game> {
        // TODO: Parse ~/.local/share/Steam/steamapps/*.acf
        // and libraryfolders.vdf
        vec![]
    }

    fn list_running(&self) -> Vec<Game> {
        // TODO: Watch for steam processes or check steam's own status files
        vec![]
    }
}
