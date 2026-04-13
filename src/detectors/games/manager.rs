use super::{Game, GameDetector};
use std::sync::{Arc, RwLock};

pub struct GameManager {
    detectors: Vec<Box<dyn GameDetector>>,
    // Cache for performance? Or just query on demand?
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn add_detector(&mut self, detector: Box<dyn GameDetector>) {
        self.detectors.push(detector);
    }

    pub fn list_all_installed(&self) -> Vec<Game> {
        self.detectors
            .iter()
            .flat_map(|d| d.list_installed())
            .collect()
    }

    pub fn get_active_game(&self) -> Option<Game> {
        // For now, return the first running game found
        self.detectors
            .iter()
            .flat_map(|d| d.list_running())
            .next()
    }
}
