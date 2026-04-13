mod detectors;
mod game_linkd {
    include!(concat!(env!("OUT_DIR"), "/game_linkd.rs"));
}
mod service;

use std::sync::{Arc, RwLock};
use varlink::VarlinkService;

use crate::detectors::games::steam::SteamDetector;
use crate::detectors::games::heroic::HeroicDetector;
use crate::detectors::games::process::ProcessDetector;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::udev::UdevDetector;
use crate::detectors::hardware::manager::HardwareManager;
use crate::service::GameLinkdService;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    log::info!("Starting Game Linkd...");

    // Initialize Managers
    let mut game_manager = GameManager::new();
    game_manager.add_detector(Box::new(SteamDetector::new()));
    game_manager.add_detector(Box::new(HeroicDetector::new()));
    game_manager.add_detector(Box::new(ProcessDetector::new()));
    let game_manager = Arc::new(RwLock::new(game_manager));

    let mut hardware_manager = HardwareManager::new();
    hardware_manager.add_detector(Box::new(UdevDetector::new()));
    
    // Discovery log
    let devices = hardware_manager.list_all_devices();
    log::info!("Detected {} gaming peripherals:", devices.len());
    for dev in &devices {
        log::info!("  - {} by {} (uaccess: {})", dev.name, dev.vendor, dev.has_uaccess);
    }

    let hardware_manager = Arc::new(RwLock::new(hardware_manager));

    // Initialize Varlink Service
    let service = GameLinkdService {
        game_manager: Arc::clone(&game_manager),
        hardware_manager: Arc::clone(&hardware_manager),
    };

    let varlink_service = VarlinkService::new(
        "io.github.game-linkd",
        "Game Linkd Daemon",
        "0.1.0",
        "https://github.com/shane/game_linkd",
        vec![Box::new(game_linkd::new(Box::new(service)))],
    );

    let address = "unix:/run/game-linkd/game-linkd.socket";
    
    log::info!("Listening on {}", address);

    // Remove existing socket if it exists
    let _ = std::fs::remove_file("/run/game-linkd/game-linkd.socket");

    // Spawn a thread to fix socket permissions once it's created
    std::thread::spawn(|| {
        use std::os::unix::fs::PermissionsExt;
        let path = "/run/game-linkd/game-linkd.socket";
        for _ in 0..50 { // try for 5 seconds
            if std::path::Path::new(path).exists() {
                log::debug!("Fixing socket permissions...");
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    // varlink::listen(service, address, initial_worker_threads, max_worker_threads, idle_timeout)
    varlink::listen(varlink_service, address, 1, 10, 0)?;

    Ok(())
}
