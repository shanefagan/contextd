//! Configuration management for contextd.
//!
//! The configuration is read once at startup from `/etc/contextd/config.toml`
//! (or the path given with `--config`). Every section and field is optional
//! and falls back to its default, but a file that exists and does not parse
//! is a fatal error: silently falling back to defaults would also drop any
//! access restrictions the administrator configured.

use anyhow::{Context, anyhow};
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::OnceLock;

pub const DEFAULT_CONFIG_PATH: &str = "/etc/contextd/config.toml";

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the active configuration. Before [`init`] runs (in unit tests,
/// for example) this is the default configuration.
pub fn get() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

/// Loads the configuration. With `path == None` the default location is used
/// and a missing file means "use defaults"; an explicitly given path must
/// exist.
pub fn init(path: Option<&Path>) -> anyhow::Result<&'static Config> {
    let explicit = path.is_some();
    let path = path.unwrap_or(Path::new(DEFAULT_CONFIG_PATH));

    let config = match fs::read_to_string(path) {
        Ok(content) => {
            let config = Config::parse(&content)
                .with_context(|| format!("invalid configuration in {}", path.display()))?;
            log::info!("Loaded configuration from {}", path.display());
            config
        }
        Err(e) if e.kind() == ErrorKind::NotFound && !explicit => {
            log::info!("No configuration at {}, using defaults", path.display());
            Config::default()
        }
        Err(e) => {
            return Err(e).with_context(|| format!("cannot read {}", path.display()));
        }
    };

    CONFIG
        .set(config)
        .map_err(|_| anyhow!("configuration was already initialised"))?;
    Ok(get())
}

/// Main configuration structure
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ttls: TtlConfig,
    pub blacklist: BlacklistConfig,
    pub auth: AuthConfig,
    pub server: ServerConfig,
}

impl Config {
    pub fn parse(content: &str) -> anyhow::Result<Self> {
        let config: Config = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        for unit in &self.auth.authorized_units {
            if !unit.ends_with(".service") || unit.contains('/') {
                return Err(anyhow!(
                    "auth.authorized_units: {unit:?} is not a systemd service name (expected e.g. \"openrgb.service\")"
                ));
            }
        }
        if self.server.max_connections == 0 || self.server.max_connections_per_uid == 0 {
            return Err(anyhow!("server connection limits must be at least 1"));
        }
        Ok(())
    }
}

/// Authorization for the RGB control socket.
///
/// The control socket's file permissions are the primary gate: in the
/// shipped socket unit it is mode 0660 and owned by the `contextd-rgb`
/// group, so only root and members of that group can connect at all.
/// `authorized_units` narrows that further.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    /// System services (units under `system.slice`) allowed to call
    /// `SetLightingContext`, e.g. `["openrgb.service"]`. When empty, every
    /// process that can open the control socket is allowed. User-session
    /// units never match, because any user can create a unit with any name
    /// inside their own session.
    pub authorized_units: Vec<String>,
}

/// TTL settings for the caches (in seconds)
#[derive(Debug, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct TtlConfig {
    /// Running-game detection (default 5s)
    pub games: u64,
    /// Installed-game library scan (default 60s)
    pub installed_games: u64,
    /// Hardware inventory (default 10s)
    pub hardware: u64,
    /// System diagnostics (default 300s)
    pub diagnostics: u64,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            games: 5,
            installed_games: 60,
            hardware: 10,
            diagnostics: 300,
        }
    }
}

/// Items to hide from results.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct BlacklistConfig {
    /// Game names to ignore (exact match). `processes` is accepted as an
    /// older name for this field.
    #[serde(alias = "processes")]
    pub games: Vec<String>,
    /// Device nodes or sysfs paths to ignore. A device is hidden if any of
    /// its paths matches.
    pub devices: Vec<String>,
}

/// Limits that protect the daemon from misbehaving local clients.
#[derive(Debug, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Maximum concurrent connections per socket (default 128)
    pub max_connections: usize,
    /// Maximum concurrent connections per socket from one UID (default 16)
    pub max_connections_per_uid: usize,
    /// Close connections that send nothing for this long (default 120s).
    /// Does not apply while a subscription is streaming.
    pub idle_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 128,
            max_connections_per_uid: 16,
            idle_timeout_secs: 120,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_sample_config_parses() {
        let sample = include_str!("../examples/config.sample.toml");
        let c = Config::parse(sample).expect("sample config must parse");
        assert_eq!(c.ttls.games, 5);
    }

    #[test]
    fn empty_file_is_defaults() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.ttls.hardware, 10);
        assert!(c.auth.authorized_units.is_empty());
    }

    #[test]
    fn partial_sections_keep_other_defaults() {
        let c = Config::parse("[ttls]\ngames = 2\n").unwrap();
        assert_eq!(c.ttls.games, 2);
        assert_eq!(c.ttls.hardware, 10);
        assert_eq!(c.ttls.diagnostics, 300);
    }

    #[test]
    fn whitelist_survives_partial_ttls() {
        let c =
            Config::parse("[auth]\nauthorized_units = [\"openrgb.service\"]\n[ttls]\ngames = 2\n")
                .unwrap();
        assert_eq!(c.auth.authorized_units, vec!["openrgb.service"]);
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(Config::parse("[auth]\nauthorised_units = [\"x.service\"]\n").is_err());
        assert!(Config::parse("[nonsense]\n").is_err());
    }

    #[test]
    fn bad_unit_names_are_errors() {
        assert!(Config::parse("[auth]\nauthorized_units = [\"openrgb\"]\n").is_err());
        assert!(
            Config::parse("[auth]\nauthorized_units = [\"user.slice/openrgb.service\"]\n").is_err()
        );
    }

    #[test]
    fn legacy_processes_key_still_works() {
        let c = Config::parse("[blacklist]\nprocesses = [\"Calculator\"]\n").unwrap();
        assert_eq!(c.blacklist.games, vec!["Calculator"]);
    }
}
