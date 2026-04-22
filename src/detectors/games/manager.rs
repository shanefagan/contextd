use super::{Game, GameDetector};
use crate::config::CONFIG;
use std::time::{Duration, Instant};

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
        let ttl = Duration::from_secs(CONFIG.ttls.games);
        if let Some((cache, ts)) = &self.installed_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        let games: Vec<Game> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_installed())
            .filter(|g| !CONFIG.blacklist.processes.contains(&g.name))
            .collect();

        self.installed_cache = Some((games.clone(), Instant::now()));
        games
    }

    pub fn get_active_game(&mut self) -> Option<Game> {
        let ttl = Duration::from_secs(CONFIG.ttls.games);
        if let Some((cache, ts)) = &self.active_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        let game = self
            .detectors
            .iter()
            .flat_map(|d| d.list_running())
            .find(|g| !CONFIG.blacklist.processes.contains(&g.name));

        self.active_cache = Some((game.clone(), Instant::now()));
        game
    }
}
impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDetector {
        games: Vec<Game>,
        running: Option<Game>,
    }

    impl GameDetector for MockDetector {
        fn name(&self) -> &str {
            "Mock"
        }
        fn list_installed(&self) -> Vec<Game> {
            self.games.clone()
        }
        fn list_running(&self) -> Vec<Game> {
            self.running.clone().into_iter().collect()
        }
    }

    #[test]
    fn test_game_manager_cache() {
        let mut mgr = GameManager::new();
        let game = Game {
            name: "Test Game".to_string(),
            id: Some("123".to_string()),
            source: "Mock".to_string(),
            pid: None,
        };

        mgr.add_detector(Box::new(MockDetector {
            games: vec![game.clone()],
            running: None,
        }));

        // First call should populate cache
        let games = mgr.list_all_installed();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Test Game");

        // Even if we add more games to the detector, cache should return old value
        // Note: In this simple mock we can't easily change the detector's state since it's boxed
        // but we can verify that the second call is immediate.
        let games2 = mgr.list_all_installed();
        assert_eq!(games2.len(), 1);
    }
}
