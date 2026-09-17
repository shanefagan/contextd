//! Context Daemon (contextd)
//!
//! A "dumb" system-level daemon that provides real-time hardware and software
//! context to user-level applications. Primarily focused on gaming use cases,
//! it identifies active games, monitors peripherals, and manages system lighting.

use log::LevelFilter;
use std::sync::{Arc, RwLock};
use varlink::VarlinkService;

mod auth;
mod config;
mod detectors;
mod rgb;
mod server;
mod service;

use crate::detectors::controllers::manager::ControllerManager;
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::server::{DynamicInterface, run_server};
use crate::service::ContextService;

/// Core interface generated from contextd.varlink
#[allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
pub mod contextd {
    include!(concat!(env!("OUT_DIR"), "/contextd.rs"));
}

/// Helper to fix socket permissions and ownership
fn spawn_permission_fixer(path: String, mode: u32, use_rgb_group: bool) {
    std::thread::spawn(move || {
        // Wait briefly for the socket to be created
        std::thread::sleep(std::time::Duration::from_millis(100));
        let path_buf = std::path::Path::new(&path);
        if path_buf.exists() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));

            // If we have an 'rgb' group, we should probably chown it
            // For now, we rely on the 0666 mode for public accessibility
            if use_rgb_group {
                log::debug!("Setting RGB group permissions for {}", path);
            }
        }
    });
}

fn main() -> anyhow::Result<()> {
    // 1. Logging Initialization
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    log::info!(
        "Starting Context Daemon (contextd) v{}",
        env!("CARGO_PKG_VERSION")
    );

    // 2. Resource Initialization
    let controller_manager = Arc::new(RwLock::new(ControllerManager::new()));
    let game_manager = Arc::new(RwLock::new(GameManager::new()));
    let hardware_manager = Arc::new(RwLock::new(HardwareManager::new()));
    let diagnostics_manager = Arc::new(RwLock::new(DiagnosticsManager::new()));

    // 3. Service Mode Selection
    let args: Vec<String> = std::env::args().collect();
    let is_rgb_mode = args.contains(&"--rgb".to_string());

    if is_rgb_mode {
        log::info!("Starting Context Daemon in RGB Mode...");
        let _ = std::fs::create_dir_all("/run/contextd/private");
        let _ = std::fs::create_dir_all("/run/contextd/public");

        let rgb_service = rgb::service::RgbService::new();

        // Observer Interface (Public)
        let obs_interface: Box<dyn varlink::Interface + Send + Sync> =
            Box::new(rgb::observer::new(Box::new(rgb_service.clone())));
        let obs_addr = "unix:/run/contextd/public/contextd-rgb-observer.socket";
        let _ = std::fs::remove_file("/run/contextd/public/contextd-rgb-observer.socket");

        // Control Interface (Private)
        let ctrl_interface: Box<dyn varlink::Interface + Send + Sync> =
            Box::new(rgb::control::new(Box::new(rgb_service)));
        let ctrl_addr = "unix:/run/contextd/private/contextd-rgb-control.socket";
        let _ = std::fs::remove_file("/run/contextd/private/contextd-rgb-control.socket");

        let observer_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Daemon RGB Observer",
            env!("CARGO_PKG_VERSION"),
            "https://github.com/shanefagan/contextd",
            vec![Box::new(DynamicInterface {
                interface: obs_interface,
                description: include_str!("rgb/observer.varlink"),
            })],
        );

        let control_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Daemon RGB Control",
            env!("CARGO_PKG_VERSION"),
            "https://github.com/shanefagan/contextd",
            vec![Box::new(DynamicInterface {
                interface: ctrl_interface,
                description: include_str!("rgb/control.varlink"),
            })],
        );

        // Spawn observer in a separate thread
        let obs_addr_clone = obs_addr.to_string();
        std::thread::spawn(move || {
            spawn_permission_fixer(
                obs_addr_clone.trim_start_matches("unix:").to_string(),
                0o666,
                false,
            );
            if let Err(e) = run_server(observer_service, &obs_addr_clone) {
                log::error!("Observer server error: {}", e);
            }
        });

        spawn_permission_fixer(
            ctrl_addr.trim_start_matches("unix:").to_string(),
            0o666,
            true, // Use RGB group context
        );

        log::info!("Observer listening on {}", obs_addr);
        log::info!("Control listening on {}", ctrl_addr);

        // For simplicity, we only run one blocking listener in the main thread.
        // In RGB mode, the Control interface is the primary listener.
        run_server(control_service, ctrl_addr)?;
    } else {
        log::info!("Starting Context Daemon in Core Mode...");
        let _ = std::fs::create_dir_all("/run/contextd/public");

        let context_service = ContextService {
            game_manager,
            hardware_manager,
            diagnostics_manager,
            controller_manager,
        };

        let interface: Box<dyn varlink::Interface + Send + Sync> =
            Box::new(contextd::new(Box::new(context_service)));
        let address = "unix:/run/contextd/public/contextd.socket";
        let _ = std::fs::remove_file("/run/contextd/public/contextd.socket");

        let varlink_service = VarlinkService::new(
            "com.performativenonsense",
            "Context Daemon",
            env!("CARGO_PKG_VERSION"),
            "https://github.com/shanefagan/contextd",
            vec![Box::new(DynamicInterface {
                interface,
                description: include_str!("contextd.varlink"),
            })],
        );

        spawn_permission_fixer(
            address.trim_start_matches("unix:").to_string(),
            0o666,
            false,
        );
        log::info!("Core listening on {}", address);

        run_server(varlink_service, address)?;
    }

    Ok(())
}
