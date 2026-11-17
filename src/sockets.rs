//! Listening socket setup.
//!
//! In production the sockets come from systemd socket activation, so
//! systemd creates them with the right mode and group before contextd even
//! starts. When run by hand (development, tests) contextd binds them itself.

use anyhow::{Context, bail};
use std::fs;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

const SD_LISTEN_FDS_START: RawFd = 3;

/// Sockets passed in by systemd, keyed by `FileDescriptorName=`.
pub struct Activated {
    fds: Vec<(String, OwnedFd)>,
}

impl Activated {
    /// Takes ownership of the descriptors described by `LISTEN_PID`,
    /// `LISTEN_FDS` and `LISTEN_FDNAMES`, and clears those variables.
    ///
    /// Must be called before any other threads are started.
    pub fn from_env() -> Self {
        let for_us = std::env::var("LISTEN_PID")
            .ok()
            .and_then(|p| p.parse::<u32>().ok())
            == Some(std::process::id());
        let count = std::env::var("LISTEN_FDS")
            .ok()
            .and_then(|n| n.parse::<RawFd>().ok())
            .unwrap_or(0);
        let names = std::env::var("LISTEN_FDNAMES").unwrap_or_default();

        // SAFETY: called from main before any threads are spawned.
        unsafe {
            std::env::remove_var("LISTEN_PID");
            std::env::remove_var("LISTEN_FDS");
            std::env::remove_var("LISTEN_FDNAMES");
        }

        if !for_us || count <= 0 {
            return Self { fds: Vec::new() };
        }

        let mut names = names.split(':');
        let fds = (0..count)
            .map(|i| {
                let fd = SD_LISTEN_FDS_START + i;
                // SAFETY: systemd passes these descriptors to us for our
                // exclusive use; F_SETFD only toggles close-on-exec.
                unsafe {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
                let name = names.next().unwrap_or("unknown").to_string();
                // SAFETY: see above; each fd is taken exactly once.
                (name, unsafe { OwnedFd::from_raw_fd(fd) })
            })
            .collect();
        Self { fds }
    }

    pub fn is_empty(&self) -> bool {
        self.fds.is_empty()
    }

    fn take(&mut self, name: &str) -> Option<UnixListener> {
        let idx = self.fds.iter().position(|(n, _)| n == name)?;
        let (_, fd) = self.fds.remove(idx);
        Some(UnixListener::from(fd))
    }
}

/// Returns the listener for `name`: the socket-activated one if systemd
/// passed it, otherwise a newly bound socket at `path` with `mode`.
pub fn acquire(
    activated: &mut Activated,
    name: &str,
    path: &Path,
    mode: u32,
) -> anyhow::Result<UnixListener> {
    if let Some(listener) = activated.take(name) {
        listener.set_nonblocking(false)?;
        log::info!("Using socket-activated {} socket", name);
        return Ok(listener);
    }
    if !activated.is_empty() {
        log::warn!(
            "systemd passed sockets but none named {:?}; binding {} directly",
            name,
            path.display()
        );
    }
    bind(path, mode)
}

/// Binds a Unix socket at `path` and sets its mode, without a window in
/// which the socket is more permissive than `mode`.
///
/// Must be called before any other threads are started, because it changes
/// the process umask temporarily.
pub fn bind(path: &Path, mode: u32) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }

    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {
            if UnixStream::connect(path).is_ok() {
                bail!(
                    "{} is in use; is another contextd instance running?",
                    path.display()
                );
            }
            fs::remove_file(path)
                .with_context(|| format!("cannot remove stale socket {}", path.display()))?;
        }
        Ok(_) => bail!(
            "{} exists and is not a socket; refusing to replace it",
            path.display()
        ),
        Err(_) => {}
    }

    // Bind with owner-only permissions, then widen to the requested mode.
    // SAFETY: umask has no memory-safety requirements.
    let old = unsafe { libc::umask(0o177) };
    let result = UnixListener::bind(path);
    unsafe { libc::umask(old) };
    let listener = result.with_context(|| format!("cannot bind {}", path.display()))?;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("cannot chmod {}", path.display()))?;
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_with_requested_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/test.socket");
        let _l = bind(&path, 0o660).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o660);
    }

    #[test]
    fn refuses_live_socket_and_replaces_stale_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.socket");
        let live = bind(&path, 0o600).unwrap();
        assert!(bind(&path, 0o600).is_err());
        drop(live);
        assert!(bind(&path, 0o600).is_ok());
    }

    #[test]
    fn refuses_to_replace_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-socket");
        fs::write(&path, "x").unwrap();
        assert!(bind(&path, 0o600).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "x");
    }
}
