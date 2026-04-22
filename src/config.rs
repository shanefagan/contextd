//! Configuration management for contextd
//!
//! Handles loading and parsing the /etc/contextd/config.toml file.

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;

/// Global configuration instance
pub static CONFIG: Lazy<Config> = Lazy::new(Config::load);

/// Main configuration structure
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub ttls: TtlConfig,
    #[serde(default)]
    pub blacklist: BlacklistConfig,
}

/// TTL settings for various detectors (in seconds)
#[derive(Debug, Deserialize, Clone)]
pub struct TtlConfig {
    /// TTL for active game detection (default 5s)
    pub games: u64,
    /// TTL for hardware inventory (default 10s)
    pub hardware: u64,
    /// TTL for system diagnostics (default 300s / 5m)
    pub diagnostics: u64,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            games: 5,
            hardware: 10,
            diagnostics: 300,
        }
    }
}

/// Blacklist settings for ignoring specific items
#[derive(Debug, Deserialize, Clone, Default)]
pub struct BlacklistConfig {
    /// Process names to ignore in game detection
    pub processes: Vec<String>,
    /// Hardware paths (udev paths) to ignore
    pub devices: Vec<String>,
}

impl Config {
    /// Loads the configuration from /etc/contextd/config.toml,
    /// falling back to defaults if the file is missing or invalid.
    fn load() -> Self {
        let paths = ["/etc/contextd/config.toml", "config.toml"];

        for path in paths {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&content) {
                    log::info!("Loaded configuration from {}", path);
                    return config;
                } else {
                    log::warn!("Failed to parse configuration at {}, using defaults", path);
                }
            }
        }

        log::debug!("No configuration file found, using defaults");
        Self::default()
    }
}
