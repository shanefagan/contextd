//! Context Daemon (contextd) - Main Entry Point
//!
//! contextd is a lightweight Linux daemon that exposes system context (active games,
//! hardware inventory, diagnostics) via Varlink interfaces. It can run in either
//! "Core" mode (system context) or "RGB" mode (passive lighting context).

#[allow(clippy::all, nonstandard_style, unused_imports, dead_code)]
#[path = "contextd.rs"]
mod contextd;
mod detectors;

mod service;

mod rgb;

use std::sync::{Arc, RwLock};
use varlink::VarlinkService;

use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::games::heroic::HeroicDetector;
use crate::detectors::games::lutris::LutrisDetector;
use crate::detectors::games::manager::GameManager;
use crate::detectors::games::steam::SteamDetector;
use crate::detectors::hardware::manager::HardwareManager;
use crate::detectors::hardware::udev::UdevDetector;
use crate::service::ContextService;

/// Helper to fix socket permissions and ownership
fn spawn_permission_fixer(path: String, mode: u32, use_rgb_group: bool) {
    std::thread::spawn(move || {
        use std::os::unix::fs::PermissionsExt;
        for _ in 0..50 {
            if std::path::Path::new(&path).exists() {
                log::debug!("Fixing permissions for {}: mode {:o}", path, mode);

                if use_rgb_group {
                    let group_name = std::ffi::CString::new("contextd-rgb").unwrap();
                    let group_info = unsafe { libc::getgrnam(group_name.as_ptr()) };
                    if !group_info.is_null() {
                        let gid = unsafe { (*group_info).gr_gid };
                        let path_cstr = std::ffi::CString::new(path.clone()).unwrap();
                        unsafe {
                            libc::chown(path_cstr.as_ptr(), u32::MAX, gid);
                        }
                    } else {
                        log::warn!("Group 'contextd-rgb' not found");
                    }
                }

                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

fn main() -> anyhow::Result<()> {
    // Initialize logging with info level by default
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Context Daemon starting...");

    // Check for RGB mode flag
    let args: Vec<String> = std::env::args().collect();
    let is_rgb_mode = args.iter().any(|arg| arg == "--rgb");

    // Initialize Global Managers (only needed for Core mode, but cheap to init)
    let mut game_manager = GameManager::new();
    game_manager.add_detector(Box::new(SteamDetector::new()));
    game_manager.add_detector(Box::new(HeroicDetector::new()));
    game_manager.add_detector(Box::new(LutrisDetector::new()));
    let game_manager = Arc::new(RwLock::new(game_manager));

    let mut hardware_manager = HardwareManager::new();
    hardware_manager.add_detector(Box::new(UdevDetector::new()));
    let hardware_manager = Arc::new(RwLock::new(hardware_manager));
    let diagnostics_manager = Arc::new(RwLock::new(DiagnosticsManager::new()));

    if is_rgb_mode {
        log::info!("Starting Context Daemon in RGBA Mode (Dual-Socket)...");
        let rgb_service = rgb::service::RgbService::new();

        let obs_addr = "unix:/run/contextd/contextd-rgb-observer.socket";
        let ctrl_addr = "unix:/run/contextd/contextd-rgb-control.socket";

        // Cleanup stale sockets
        let _ = std::fs::remove_file(obs_addr.trim_start_matches("unix:"));
        let _ = std::fs::remove_file(ctrl_addr.trim_start_matches("unix:"));

        // 1. Start Observer Server (Public - 0666)
        let observer_interface = vec![Box::new(rgb::observer::new(Box::new(rgb_service.clone())))
            as Box<dyn varlink::Interface + Send + Sync>];
        let observer_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Observer",
            "0.1.0",
            "https://github.com/shanefagan/contextd",
            observer_interface,
        );
        // Temporarily using 0666 for both to simplify testing
        spawn_permission_fixer(
            obs_addr.trim_start_matches("unix:").to_string(),
            0o666,
            false,
        );

        let obs_addr_str = obs_addr.to_string();
        std::thread::spawn(move || {
            let config = varlink::ListenConfig {
                initial_worker_threads: 1,
                max_worker_threads: 128,
                idle_timeout: 0,
                ..Default::default()
            };
            if let Err(e) = varlink::listen(observer_service, &obs_addr_str, &config) {
                log::error!("Observer server failed: {}", e);
            }
        });

        // 2. Start Control Server (Public - 0666)
        let control_interface = vec![Box::new(rgb::control::new(Box::new(rgb_service)))
            as Box<dyn varlink::Interface + Send + Sync>];
        let control_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Control",
            "0.1.0",
            "https://github.com/shanefagan/contextd",
            control_interface,
        );
        spawn_permission_fixer(
            ctrl_addr.trim_start_matches("unix:").to_string(),
            0o666,
            false,
        );

        log::info!("Observer listening on {}", obs_addr);
        log::info!("Control listening on {}", ctrl_addr);

        let config = varlink::ListenConfig {
            initial_worker_threads: 1,
            max_worker_threads: 128,
            idle_timeout: 0,
            ..Default::default()
        };
        varlink::listen(control_service, ctrl_addr, &config)?;
    } else {
        log::info!("Starting Context Daemon in Core Mode...");
        let address = "unix:/run/contextd/contextd.socket";
        let _ = std::fs::remove_file(address.trim_start_matches("unix:"));

        let service = ContextService {
            game_manager: Arc::clone(&game_manager),
            hardware_manager: Arc::clone(&hardware_manager),
            diagnostics_manager: Arc::clone(&diagnostics_manager),
        };
        let interfaces: Vec<Box<dyn varlink::Interface + Send + Sync>> =
            vec![Box::new(contextd::new(Box::new(service)))];

        let varlink_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Daemon",
            "0.1.0",
            "https://github.com/shanefagan/contextd",
            interfaces,
        );

        spawn_permission_fixer(
            address.trim_start_matches("unix:").to_string(),
            0o666,
            false,
        );
        log::info!("Core listening on {}", address);

        let config = varlink::ListenConfig {
            initial_worker_threads: 1,
            max_worker_threads: 128,
            idle_timeout: 0,
            ..Default::default()
        };
        varlink::listen(varlink_service, address, &config)?;
    }

    Ok(())
}
