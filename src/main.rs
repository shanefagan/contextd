//! Context Daemon (contextd)
//!
//! A "dumb" system-level daemon that provides real-time hardware and software
//! context to user-level applications. Primarily focused on gaming use cases,
//! it identifies running games, lists peripherals, and relays an ambient
//! lighting context between applications.

use log::LevelFilter;
use std::path::PathBuf;
use std::sync::Mutex;
use varlink::VarlinkService;

mod auth;
mod config;
mod detectors;
mod procfs;
mod rgb;
mod server;
mod service;
mod sockets;
mod util;
mod watch;

/// Generated types for the core interface, shared with contextd-client.
pub use contextd_client::contextd;

use crate::detectors::controllers::manager::ControllerManager;
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::focus::FocusTracker;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::server::Limits;
use crate::service::ContextService;
use crate::sockets::Activated;

const VENDOR: &str = "com.performativenonsense";
const URL: &str = "https://github.com/shanefagan/contextd";
const DEFAULT_RUNTIME_DIR: &str = "/run/contextd";

/// Message size limits per socket. The control socket accepts a full
/// 16384-pixel matrix, which is roughly 700 KB of JSON.
const MAX_CORE_MESSAGE: usize = 64 * 1024;
const MAX_OBSERVER_MESSAGE: usize = 16 * 1024;
const MAX_CONTROL_MESSAGE: usize = 2 * 1024 * 1024;

const USAGE: &str = "\
Usage: contextd [OPTIONS]

Options:
  --rgb                 Serve the RGB observer and control sockets instead
                        of the core socket
  --config <PATH>       Configuration file (default /etc/contextd/config.toml)
  --runtime-dir <DIR>   Where to create sockets when not socket-activated
                        (default /run/contextd)
  -h, --help            Show this help
  -V, --version         Show the version
";

struct Args {
    rgb: bool,
    config: Option<PathBuf>,
    runtime_dir: PathBuf,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args {
        rgb: false,
        config: None,
        runtime_dir: PathBuf::from(DEFAULT_RUNTIME_DIR),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--rgb" => args.rgb = true,
            "--config" => {
                args.config = Some(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("--config needs a path"))?
                        .into(),
                )
            }
            "--runtime-dir" => {
                args.runtime_dir = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--runtime-dir needs a path"))?
                    .into()
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("contextd {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument {other:?}\n\n{USAGE}"),
        }
    }
    Ok(args)
}

/// Each connection has its own thread. glibc gives busy threads their own
/// malloc arena (up to 8 per core), which inflates RSS for no benefit here.
fn limit_malloc_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: mallopt is safe to call at any time; called before threads.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
}

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    if let Err(e) = run() {
        log::error!("{:#}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = parse_args()?;
    limit_malloc_arenas();

    log::info!(
        "Starting Context Daemon (contextd) v{}",
        env!("CARGO_PKG_VERSION")
    );
    config::init(args.config.as_deref())?;

    // Read the socket-activation variables before any thread starts:
    // Activated::from_env clears them from the environment.
    let mut activated = Activated::from_env();
    let public = args.runtime_dir.join("public");
    let private = args.runtime_dir.join("private");

    if args.rgb {
        log::info!("Starting Context Daemon in RGB Mode...");
        let observer_listener = sockets::acquire(
            &mut activated,
            "observer",
            &public.join("contextd-rgb-observer.socket"),
            0o666,
        )?;
        // Without socket activation the control socket is only reachable by
        // the daemon's own user and root. The shipped socket unit opens it
        // to the contextd-rgb group instead.
        let control_listener = sockets::acquire(
            &mut activated,
            "control",
            &private.join("contextd-rgb-control.socket"),
            0o660,
        )?;

        let rgb_service = rgb::service::RgbService::new();

        let observer_service = VarlinkService::new(
            VENDOR,
            "Context Daemon RGB Observer",
            env!("CARGO_PKG_VERSION"),
            URL,
            vec![Box::new(rgb::observer::new(Box::new(rgb_service.clone())))],
        );
        let control_service = VarlinkService::new(
            VENDOR,
            "Context Daemon RGB Control",
            env!("CARGO_PKG_VERSION"),
            URL,
            vec![Box::new(rgb::control::new(Box::new(rgb_service)))],
        );

        std::thread::Builder::new()
            .name("observer-accept".into())
            .spawn(move || {
                let limits = Limits::from_config(MAX_OBSERVER_MESSAGE);
                if let Err(e) =
                    server::serve("observer", observer_listener, observer_service, limits)
                {
                    log::error!("Observer server error: {:#}", e);
                }
                // Never keep running half a service: let systemd restart us.
                std::process::exit(1);
            })?;

        log::info!("RGB observer and control sockets ready");
        server::serve(
            "control",
            control_listener,
            control_service,
            Limits::from_config(MAX_CONTROL_MESSAGE),
        )?;
    } else {
        log::info!("Starting Context Daemon in Core Mode...");
        let listener = sockets::acquire(
            &mut activated,
            "core",
            &public.join("contextd.socket"),
            0o666,
        )?;

        let context_service = ContextService {
            games: Mutex::new(GameManager::new()),
            hardware: Mutex::new(HardwareManager::new()),
            diagnostics: Mutex::new(DiagnosticsManager::new()),
            controllers: Mutex::new(ControllerManager::new()),
            focus: Mutex::new(FocusTracker::new()),
        };

        let varlink_service = VarlinkService::new(
            VENDOR,
            "Context Daemon",
            env!("CARGO_PKG_VERSION"),
            URL,
            vec![Box::new(contextd::new(Box::new(context_service)))],
        );

        log::info!("Core socket ready");
        server::serve(
            "core",
            listener,
            varlink_service,
            Limits::from_config(MAX_CORE_MESSAGE),
        )?;
    }

    Ok(())
}
