use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub name: String,
    pub id: Option<String>,
    pub source: String,
    pub pid: Option<i32>,
}

pub trait GameDetector: Send + Sync {
    /// Unique name of the detector (e.g., "Steam", "Lutris")
    fn name(&self) -> &str;

    /// Returns a list of all games installed through this source
    fn list_installed(&self) -> Vec<Game>;

    /// Returns a list of games currently running from this source
    fn list_running(&self) -> Vec<Game>;
}

pub mod steam;
pub mod process;
pub mod manager;
