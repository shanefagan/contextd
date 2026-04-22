//! Peer authentication and identity for contextd
//!
//! Provides utilities to identify the calling process on a Unix socket
//! and map it to a systemd unit for granular access control.

use std::cell::RefCell;
use std::fs;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

thread_local! {
    /// Stores the credentials of the peer currently being handled by this thread.
    static CURRENT_PEER: RefCell<Option<PeerInfo>> = const { RefCell::new(None) };
}

/// Information about the connected peer
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PeerInfo {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
    pub unit: Option<String>,
}

impl PeerInfo {
    /// Extracts peer credentials from a UnixStream
    pub fn from_stream(stream: &UnixStream) -> Option<Self> {
        let fd = stream.as_raw_fd();
        let mut ucred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

        let res = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut ucred as *mut _ as *mut _,
                &mut len,
            )
        };

        if res == 0 {
            let pid = ucred.pid;
            let unit = Self::get_unit_for_pid(pid);
            Some(Self {
                pid,
                uid: ucred.uid,
                gid: ucred.gid,
                unit,
            })
        } else {
            None
        }
    }

    /// Attempts to find the systemd unit name for a given PID by reading its cgroup.
    fn get_unit_for_pid(pid: i32) -> Option<String> {
        let cgroup_path = format!("/proc/{}/cgroup", pid);
        if let Ok(content) = fs::read_to_string(cgroup_path) {
            // In cgroup v2, the format is "0::/path/to/unit"
            for line in content.lines() {
                if let Some(path) = line.strip_prefix("0::") {
                    // Typical paths:
                    // /system.slice/contextd.service
                    // /user.slice/user-1000.slice/user@1000.service/app.slice/app-name.scope

                    // We want the most specific .service or .scope name
                    let parts: Vec<&str> = path.split('/').collect();
                    for part in parts.iter().rev() {
                        if part.ends_with(".service") || part.ends_with(".scope") {
                            return Some(part.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

/// Checks if the current calling peer is authorized based on a list of systemd units.
/// Returns Ok(()) if authorized (or if the list is empty), otherwise returns Err(unit_name).
pub fn verify_unit_access(authorized_units: &[String]) -> Result<(), String> {
    if authorized_units.is_empty() {
        return Ok(());
    }

    let peer = get_current_peer();
    let unit = peer
        .as_ref()
        .and_then(|p| p.unit.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("unknown");

    if authorized_units.iter().any(|u| u == unit) {
        Ok(())
    } else {
        Err(unit.to_string())
    }
}

/// Sets the current peer info for the local thread.
pub fn set_current_peer(info: Option<PeerInfo>) {
    CURRENT_PEER.with(|p| *p.borrow_mut() = info);
}

/// Gets the current peer info for the local thread.
pub fn get_current_peer() -> Option<PeerInfo> {
    CURRENT_PEER.with(|p| p.borrow().clone())
}
