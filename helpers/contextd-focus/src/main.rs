//! contextd-focus: tells contextd which process owns the focused window.
//!
//! contextd runs as a system service and cannot see the desktop. This
//! helper runs inside the user's graphical session (as a systemd user
//! service), watches focus changes and calls `ReportFocus` on the core
//! socket. contextd then reports the focused game from `GetActiveGame`.
//!
//! Backends, picked from the environment:
//! - Hyprland (`HYPRLAND_INSTANCE_SIGNATURE`): the compositor's IPC socket.
//! - Sway (`SWAYSOCK`): the i3-compatible IPC socket.
//! - X11 / XWayland (`DISPLAY`): `_NET_ACTIVE_WINDOW`, with the owning PID
//!   from the X-Resource extension (correct across PID namespaces) or
//!   `_NET_WM_PID`. On KDE and GNOME Wayland this sees XWayland windows,
//!   which is where Proton and most native games run.

mod hyprland;
mod sway;
mod x11;

use anyhow::{Context, bail};
use contextd_client::CoreClient;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Re-send the current focus this often, so a restarted contextd learns
/// it without waiting for the next focus change.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// A source of focus changes.
pub trait FocusSource {
    /// PID of the process owning the focused window, if any.
    fn current(&mut self) -> anyhow::Result<Option<u32>>;
    /// Blocks until focus may have changed.
    fn wait(&mut self) -> anyhow::Result<()>;
}

const USAGE: &str = "\
Usage: contextd-focus [--once]

Reports the process that owns the focused window to contextd.

  --once          Print the focused PID (or \"none\") and exit
  -h, --help      Show this help
  -V, --version   Show the version

Environment: CONTEXTD_CORE_SOCKET overrides the core socket path.
";

fn backend() -> anyhow::Result<Box<dyn FocusSource>> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return Ok(Box::new(hyprland::Hyprland::from_env()?));
    }
    if let Some(sock) = std::env::var_os("SWAYSOCK") {
        return Ok(Box::new(sway::Sway::new(sock.into())?));
    }
    if std::env::var_os("DISPLAY").is_some() {
        return Ok(Box::new(x11::X11::connect()?));
    }
    bail!(
        "no supported session found: need HYPRLAND_INSTANCE_SIGNATURE, SWAYSOCK or DISPLAY \
         (import them into the systemd user environment)"
    )
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run() {
        log::error!("{:#}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let mut once = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--once" => once = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("contextd-focus {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            other => bail!("unknown argument {other:?}\n\n{USAGE}"),
        }
    }

    let mut source = backend().context("cannot watch window focus")?;
    if once {
        match source.current()? {
            Some(pid) => println!("{pid}"),
            None => println!("none"),
        }
        return Ok(());
    }

    let client = Arc::new(CoreClient::new(std::env::var("CONTEXTD_CORE_SOCKET").ok()));
    let last: Arc<Mutex<Option<Option<u32>>>> = Arc::new(Mutex::new(None));

    {
        let client = Arc::clone(&client);
        let last = Arc::clone(&last);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(HEARTBEAT);
                let current = *last.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(pid) = current {
                    report(&client, pid);
                }
            }
        });
    }

    log::info!("Watching window focus");
    loop {
        let pid = source.current()?;
        let changed = {
            let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *last != Some(pid);
            *last = Some(pid);
            changed
        };
        if changed {
            log::debug!("Focus: {:?}", pid);
            report(&client, pid);
        }
        source.wait()?;
    }
}

fn report(client: &CoreClient, pid: Option<u32>) {
    if let Err(e) = client.report_focus(pid.map(i64::from)) {
        log::warn!("Could not report focus to contextd: {}", e);
    }
}
