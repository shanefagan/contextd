//! Game Manager
//!
//! Combines the launcher detectors. Installed libraries are cached for
//! `ttls.installed_games`; running games for `ttls.games`. Running games are
//! found with a single pass over `/proc`, reading only the environment
//! variables the detectors care about.

use super::heroic::HeroicDetector;
use super::lutris::LutrisDetector;
use super::steam::SteamDetector;
use super::{Game, GameDetector, GameRef, ProcessEnv};
use crate::config;
use crate::procfs;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// One process that carries at least one launcher variable.
#[derive(Debug, Clone)]
pub struct ProcessSample {
    pub pid: i32,
    pub start_time: u64,
    pub rss_pages: u64,
    pub env: ProcessEnv,
}

type ProcessScanner = Box<dyn Fn(&[&'static str]) -> Vec<ProcessSample> + Send + Sync>;

fn scan_processes(keys: &[&'static str]) -> Vec<ProcessSample> {
    procfs::pids()
        .filter_map(|pid| {
            let env: ProcessEnv = procfs::environ_subset(pid, keys).into_iter().collect();
            if env.is_empty() {
                return None;
            }
            Some(ProcessSample {
                pid,
                start_time: procfs::start_time(pid)?,
                rss_pages: procfs::rss_pages(pid).unwrap_or(0),
                env,
            })
        })
        .collect()
}

pub struct GameManager {
    detectors: Vec<Box<dyn GameDetector>>,
    scanner: ProcessScanner,
    installed_cache: Option<(Vec<Vec<Game>>, Instant)>,
    running_cache: Option<(Vec<Game>, Instant)>,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            // Order matters: the first detector that recognises a process
            // claims it. Lutris's variables are the most generic, so last.
            detectors: vec![
                Box::new(SteamDetector::new()),
                Box::new(HeroicDetector::new()),
                Box::new(LutrisDetector::new()),
            ],
            scanner: Box::new(scan_processes),
            installed_cache: None,
            running_cache: None,
        }
    }

    fn not_blacklisted(game: &Game) -> bool {
        !config::get().blacklist.games.contains(&game.name)
    }

    /// Installed games, grouped by detector index.
    fn installed_by_detector(&mut self) -> Vec<Vec<Game>> {
        let ttl = Duration::from_secs(config::get().ttls.installed_games);
        if let Some((cache, ts)) = &self.installed_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }
        let installed: Vec<Vec<Game>> = self.detectors.iter().map(|d| d.list_installed()).collect();
        self.installed_cache = Some((installed.clone(), Instant::now()));
        installed
    }

    pub fn installed(&mut self) -> Vec<Game> {
        self.installed_by_detector()
            .into_iter()
            .flatten()
            .filter(Self::not_blacklisted)
            .collect()
    }

    /// Running games, most recently launched first.
    pub fn running(&mut self) -> Vec<Game> {
        let ttl = Duration::from_secs(config::get().ttls.games);
        if let Some((cache, ts)) = &self.running_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        let installed = self.installed_by_detector();
        let mut keys: Vec<&'static str> = self
            .detectors
            .iter()
            .flat_map(|d| d.env_keys().iter().copied())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        let samples = (self.scanner)(&keys);

        struct Session {
            game: Game,
            started: u64,
            best_rss: u64,
        }
        let mut sessions: HashMap<(usize, GameRef), Session> = HashMap::new();

        for sample in samples {
            for (idx, detector) in self.detectors.iter().enumerate() {
                let Some(game_ref) = detector.identify(&sample.env) else {
                    continue;
                };
                let found = installed[idx].iter().find(|g| match &game_ref {
                    GameRef::Id(id) => g.id.as_deref() == Some(id.as_str()),
                    GameRef::Name(name) => &g.name == name,
                });
                let Some(game) = found else {
                    continue;
                };
                let session = sessions.entry((idx, game_ref)).or_insert_with(|| Session {
                    game: game.clone(),
                    started: sample.start_time,
                    best_rss: 0,
                });
                session.started = session.started.min(sample.start_time);
                // The game itself is usually the biggest process tagged
                // with the launcher variable; wrappers and reapers are tiny.
                if sample.rss_pages >= session.best_rss {
                    session.best_rss = sample.rss_pages;
                    session.game.pid = Some(sample.pid as i64);
                }
                break;
            }
        }

        let mut sessions: Vec<Session> = sessions.into_values().collect();
        sessions.sort_by(|a, b| {
            b.started
                .cmp(&a.started)
                .then(a.game.name.cmp(&b.game.name))
        });
        let running: Vec<Game> = sessions
            .into_iter()
            .map(|s| s.game)
            .filter(Self::not_blacklisted)
            .collect();

        self.running_cache = Some((running.clone(), Instant::now()));
        running
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
    }

    impl GameDetector for MockDetector {
        fn name(&self) -> &str {
            "Mock"
        }
        fn list_installed(&self) -> Vec<Game> {
            self.games.clone()
        }
        fn env_keys(&self) -> &'static [&'static str] {
            &["MOCK_ID"]
        }
        fn identify(&self, env: &ProcessEnv) -> Option<GameRef> {
            env.get("MOCK_ID").map(|id| GameRef::Id(id.clone()))
        }
    }

    fn game(id: &str, name: &str) -> Game {
        Game {
            name: name.into(),
            id: Some(id.into()),
            source: "Mock".into(),
            pid: None,
        }
    }

    fn sample(pid: i32, start: u64, rss: u64, id: &str) -> ProcessSample {
        ProcessSample {
            pid,
            start_time: start,
            rss_pages: rss,
            env: ProcessEnv::from([("MOCK_ID", id.to_string())]),
        }
    }

    fn manager(samples: Vec<ProcessSample>) -> GameManager {
        GameManager {
            detectors: vec![Box::new(MockDetector {
                games: vec![game("1", "Older"), game("2", "Newer")],
            })],
            scanner: Box::new(move |_| samples.clone()),
            installed_cache: None,
            running_cache: None,
        }
    }

    #[test]
    fn most_recent_launch_is_active_and_biggest_process_is_pid() {
        let mut m = manager(vec![
            sample(10, 100, 5, "1"),
            sample(20, 500, 1, "2"),   // wrapper for "Newer"
            sample(21, 510, 900, "2"), // the actual game
            sample(30, 50, 1, "999"),  // not installed: ignored
        ]);
        let running = m.running();
        assert_eq!(running.len(), 2);
        assert_eq!(running[0].name, "Newer");
        assert_eq!(running[0].pid, Some(21));
        assert_eq!(running[1].name, "Older");
    }

    #[test]
    fn installed_list_is_cached() {
        let mut m = manager(vec![]);
        assert_eq!(m.installed().len(), 2);
        m.detectors.clear();
        assert_eq!(m.installed().len(), 2, "served from cache");
    }
}
