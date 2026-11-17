//! Caller identity for contextd.
//!
//! Each connection is served on its own thread. The server records who is
//! on the other end of the socket (from `SO_PEERCRED`) in a thread-local so
//! that method handlers can make access decisions.

use std::cell::RefCell;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;

/// `SO_PEERPIDFD` (Linux 6.5+). Not exported by the libc crate for every
/// target yet; the value is the same on all architectures we build for.
#[cfg(not(any(target_arch = "mips", target_arch = "mips64", target_arch = "sparc64")))]
const SO_PEERPIDFD: libc::c_int = 77;

thread_local! {
    static CURRENT: RefCell<Option<Caller>> = const { RefCell::new(None) };
}

/// The connection currently being served on this thread.
#[derive(Debug, Clone)]
pub struct Caller {
    pub peer: Option<PeerInfo>,
    pub fd: RawFd,
}

/// Information about the connected peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
    /// The system service the peer runs in, if any. Only units under a
    /// system slice are reported; user-session units are `None`.
    pub unit: Option<String>,
}

impl PeerInfo {
    /// Extracts peer credentials from a connected Unix socket.
    pub fn from_stream(stream: &UnixStream) -> Option<Self> {
        let fd = stream.as_raw_fd();
        let mut ucred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: ucred and len are valid for writes of the sizes given.
        let res = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut ucred as *mut _ as *mut libc::c_void,
                &mut len,
            )
        };
        if res != 0 {
            return None;
        }

        let pidfd = peer_pidfd(fd);
        let mut unit = if ucred.pid > 0 {
            fs::read_to_string(format!("/proc/{}/cgroup", ucred.pid))
                .ok()
                .and_then(|c| system_unit_from_cgroup(&c))
        } else {
            None
        };

        // SO_PEERCRED records the PID at connect() time. If that process has
        // exited since, the PID may now belong to someone else and the
        // cgroup we just read would be theirs. The pidfd pins the original
        // process, so if it is gone we discard the unit.
        if let Some(pidfd) = &pidfd
            && !pidfd_alive(pidfd)
        {
            unit = None;
        }

        Some(Self {
            pid: ucred.pid,
            uid: ucred.uid,
            gid: ucred.gid,
            unit,
        })
    }
}

#[cfg(not(any(target_arch = "mips", target_arch = "mips64", target_arch = "sparc64")))]
fn peer_pidfd(fd: RawFd) -> Option<OwnedFd> {
    let mut pidfd: libc::c_int = -1;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: pidfd and len are valid for writes of the sizes given.
    let res = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            SO_PEERPIDFD,
            &mut pidfd as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if res == 0 && pidfd >= 0 {
        // SAFETY: the kernel just handed us this descriptor; we own it.
        Some(unsafe { OwnedFd::from_raw_fd(pidfd) })
    } else {
        None
    }
}

#[cfg(any(target_arch = "mips", target_arch = "mips64", target_arch = "sparc64"))]
fn peer_pidfd(_fd: RawFd) -> Option<OwnedFd> {
    None
}

/// True if the process behind `pidfd` still exists. Signal 0 performs only
/// the existence and permission checks; EPERM still means "exists".
fn pidfd_alive(pidfd: &OwnedFd) -> bool {
    // SAFETY: plain syscall with a valid fd and no pointers besides NULL.
    let res = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            0,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    res == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Returns the system service a process belongs to, given the contents of
/// its `/proc/<pid>/cgroup`.
///
/// Only the cgroup v2 hierarchy is used. The unit is the first path
/// component after the leading slices, and it must be a `.service`.
/// Anything under `user.slice` is rejected: users can create units with any
/// name inside their own session (`systemd-run --user --unit=openrgb ...`),
/// so a name match there proves nothing.
pub fn system_unit_from_cgroup(content: &str) -> Option<String> {
    let path = content.lines().find_map(|l| l.strip_prefix("0::"))?;
    for component in path.split('/').filter(|c| !c.is_empty()) {
        if component.ends_with(".slice") {
            if component == "user.slice" {
                return None;
            }
            continue;
        }
        return component
            .ends_with(".service")
            .then(|| component.to_string());
    }
    None
}

/// Checks whether `peer` may use a restricted method.
///
/// With an empty `authorized_units` list every peer is allowed: access is
/// then controlled by who can open the socket at all. Otherwise root, or a
/// peer running in one of the listed system services, is allowed. On
/// failure the error carries a description of the caller for the reply.
pub fn verify_unit_access(
    authorized_units: &[String],
    peer: Option<&PeerInfo>,
) -> Result<(), String> {
    if authorized_units.is_empty() {
        return Ok(());
    }
    let Some(peer) = peer else {
        return Err("unknown".to_string());
    };
    if peer.uid == 0 {
        return Ok(());
    }
    match &peer.unit {
        Some(unit) if authorized_units.iter().any(|u| u == unit) => Ok(()),
        Some(unit) => Err(unit.clone()),
        None => Err(format!("uid {} (not a system service)", peer.uid)),
    }
}

/// Records the connection served on this thread.
pub fn set_current(caller: Option<Caller>) {
    CURRENT.with(|c| *c.borrow_mut() = caller);
}

/// The peer of the connection served on this thread.
pub fn current_peer() -> Option<PeerInfo> {
    CURRENT.with(|c| c.borrow().as_ref().and_then(|c| c.peer.clone()))
}

/// True if the client of the connection served on this thread has hung up.
/// Used by long-running subscriptions, which otherwise only notice a dead
/// client when the next write fails.
pub fn current_connection_closed() -> bool {
    let Some(fd) = CURRENT.with(|c| c.borrow().as_ref().map(|c| c.fd)) else {
        return false;
    };
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLRDHUP,
        revents: 0,
    };
    // SAFETY: pfd is a valid pollfd array of length 1.
    let n = unsafe { libc::poll(&mut pfd, 1, 0) };
    n > 0 && pfd.revents & (libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(uid: u32, unit: Option<&str>) -> PeerInfo {
        PeerInfo {
            pid: 1234,
            uid,
            gid: uid,
            unit: unit.map(str::to_string),
        }
    }

    #[test]
    fn system_service_is_recognised() {
        assert_eq!(
            system_unit_from_cgroup("0::/system.slice/openrgb.service\n").as_deref(),
            Some("openrgb.service")
        );
    }

    #[test]
    fn delegated_subcgroup_reports_owning_service() {
        assert_eq!(
            system_unit_from_cgroup("0::/system.slice/openrgb.service/worker\n").as_deref(),
            Some("openrgb.service")
        );
    }

    #[test]
    fn custom_slice_is_fine() {
        assert_eq!(
            system_unit_from_cgroup("0::/system.slice/rgb.slice/openrgb.service\n").as_deref(),
            Some("openrgb.service")
        );
    }

    #[test]
    fn user_units_cannot_impersonate_system_services() {
        let spoof = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/openrgb.service\n";
        assert_eq!(system_unit_from_cgroup(spoof), None);
        let scope = "0::/user.slice/user-1000.slice/session-2.scope\n";
        assert_eq!(system_unit_from_cgroup(scope), None);
    }

    #[test]
    fn scopes_and_root_are_not_services() {
        assert_eq!(system_unit_from_cgroup("0::/init.scope\n"), None);
        assert_eq!(system_unit_from_cgroup("0::/\n"), None);
        assert_eq!(
            system_unit_from_cgroup("1:name=systemd:/system.slice/x.service\n"),
            None
        );
    }

    #[test]
    fn empty_whitelist_allows_everyone() {
        assert!(verify_unit_access(&[], None).is_ok());
    }

    #[test]
    fn whitelist_is_enforced() {
        let list = vec!["openrgb.service".to_string()];
        assert!(verify_unit_access(&list, Some(&peer(1000, Some("openrgb.service")))).is_ok());
        assert!(verify_unit_access(&list, Some(&peer(1000, Some("other.service")))).is_err());
        assert!(verify_unit_access(&list, Some(&peer(1000, None))).is_err());
        assert!(verify_unit_access(&list, None).is_err());
        assert!(verify_unit_access(&list, Some(&peer(0, None))).is_ok());
    }

    #[test]
    fn reads_peer_credentials() {
        let (a, _b) = UnixStream::pair().unwrap();
        let p = PeerInfo::from_stream(&a).unwrap();
        assert_eq!(p.pid as u32, std::process::id());
        assert_eq!(p.uid, unsafe { libc::geteuid() });
    }
}
