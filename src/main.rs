mod detectors;
mod contextd {
    include!(concat!(env!("OUT_DIR"), "/contextd.rs"));
}
mod service;

use std::sync::{Arc, RwLock};
use varlink::VarlinkService;

use crate::detectors::games::steam::SteamDetector;
use crate::detectors::games::heroic::HeroicDetector;
use crate::detectors::games::lutris::LutrisDetector;
use crate::detectors::games::process::ProcessDetector;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::udev::UdevDetector;
use crate::detectors::hardware::manager::HardwareManager;
use crate::service::ContextService;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    log::info!("Starting Context Daemon (contextd)...");

    // Initialize Managers
    let mut game_manager = GameManager::new();
    game_manager.add_detector(Box::new(SteamDetector::new()));
    game_manager.add_detector(Box::new(HeroicDetector::new()));
    game_manager.add_detector(Box::new(LutrisDetector::new()));
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
    
    // Start Hardware Hotplug Monitor
    let hw_manager_for_monitor = Arc::clone(&hardware_manager);
    std::thread::spawn(move || {
        use udev::{MonitorBuilder, EventType};
        if let Ok(monitor) = MonitorBuilder::new() {
            if let Ok(m) = monitor.match_subsystem("input")
                .and_then(|mb| mb.match_subsystem("sound"))
                .and_then(|mb| mb.match_subsystem("hidraw"))
                .and_then(|mb| mb.listen()) 
            {
                log::info!("Started hardware hotplug monitor.");
                for event in m.iter() {
                    let action = event.action().unwrap_or_default().to_string_lossy();
                    if action == "add" || action == "remove" {
                        log::info!("Hardware changed: {} {}", action, event.sysname().to_string_lossy());
                        if let Ok(mut mgr) = hw_manager_for_monitor.write() {
                            mgr.invalidate();
                        }
                    }
                }
            }
        }
    });

    // Initialize Varlink Service
    let service = ContextService {
        game_manager: Arc::clone(&game_manager),
        hardware_manager: Arc::clone(&hardware_manager),
    };

    let varlink_service = VarlinkService::new(
        "io.github.contextd",
        "Context Daemon",
        "0.1.0",
        "https://github.com/shane/contextd",
        vec![Box::new(contextd::new(Box::new(service)))],
    );

    let address = "unix:/run/contextd/contextd.socket";
    
    log::info!("Listening on {}", address);

    // Remove existing socket if it exists
    let _ = std::fs::remove_file("/run/contextd/contextd.socket");

    // Spawn a thread to fix socket permissions once it's created
    std::thread::spawn(|| {
        use std::os::unix::fs::PermissionsExt;
        let path = "/run/contextd/contextd.socket";
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
