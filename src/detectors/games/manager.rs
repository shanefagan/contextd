use super::{Game, GameDetector};
use std::time::{Duration, Instant};

const CACHE_DURATION: Duration = Duration::from_secs(5);

pub struct GameManager {
    detectors: Vec<Box<dyn GameDetector>>,
    installed_cache: Option<(Vec<Game>, Instant)>,
    active_cache: Option<(Option<Game>, Instant)>,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            installed_cache: None,
            active_cache: None,
        }
    }

    pub fn add_detector(&mut self, detector: Box<dyn GameDetector>) {
        self.detectors.push(detector);
        self.installed_cache = None; // Invalidate cache
    }

    pub fn list_all_installed(&mut self) -> Vec<Game> {
        if let Some((cache, ts)) = &self.installed_cache {
            if ts.elapsed() < CACHE_DURATION {
                return cache.clone();
            }
        }

        let games: Vec<Game> = self.detectors
            .iter()
            .flat_map(|d| d.list_installed())
            .collect();
        
        self.installed_cache = Some((games.clone(), Instant::now()));
        games
    }

    pub fn get_active_game(&mut self) -> Option<Game> {
        if let Some((cache, ts)) = &self.active_cache {
            if ts.elapsed() < CACHE_DURATION {
                return cache.clone();
            }
        }

        let game = self.detectors
            .iter()
            .flat_map(|d| d.list_running())
            .next();
        
        self.active_cache = Some((game.clone(), Instant::now()));
        game
    }
}
impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}
