pub use crate::contextd::Game;

pub trait GameDetector: Send + Sync {
    /// Unique name of the detector (e.g., "Steam", "Lutris")
    #[allow(dead_code)]
    fn name(&self) -> &str;


    /// Returns a list of all games installed through this source
    fn list_installed(&self) -> Vec<Game>;

    /// Returns a list of games currently running from this source
    fn list_running(&self) -> Vec<Game>;
}

pub mod heroic;
pub mod lutris;
pub mod manager;
pub mod process;
pub mod steam;
