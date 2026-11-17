//! Change notification for push subscriptions.
//!
//! A [`Notifier`] is a generation counter that subscribers can wait on.
//! The udev watcher bumps one when the device database changes, so the
//! device list is rescanned only when something actually changed.

use crate::util::lock;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
use std::time::Duration;

pub struct Notifier {
    generation: Mutex<u64>,
    cvar: Condvar,
}

impl Notifier {
    pub const fn new() -> Self {
        Self {
            generation: Mutex::new(0),
            cvar: Condvar::new(),
        }
    }

    pub fn current(&self) -> u64 {
        *lock(&self.generation)
    }

    pub fn bump(&self) {
        *lock(&self.generation) += 1;
        self.cvar.notify_all();
    }

    /// Waits until the generation differs from `seen` or `timeout` passes,
    /// and returns the current generation.
    pub fn wait_past(&self, seen: u64, timeout: Duration) -> u64 {
        let guard = lock(&self.generation);
        let (guard, _) = self
            .cvar
            .wait_timeout_while(guard, timeout, |g| *g == seen)
            .unwrap_or_else(PoisonError::into_inner);
        *guard
    }
}

/// udev's device database changed: cached device scans are stale.
pub static UDEV: Notifier = Notifier::new();
/// The device list clients see may have changed (udev or hints).
pub static DEVICE_LIST: Notifier = Notifier::new();
/// The running-games list clients see may have changed (focus reports).
pub static GAMES: Notifier = Notifier::new();

const UDEV_DATA: &str = "/run/udev/data";
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Starts the udev database watcher the first time it is needed. Returns
/// whether device changes are being watched; if not, callers poll.
pub fn ensure_udev_watcher() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| match watch_dir(Path::new(UDEV_DATA)) {
        Ok(fd) => {
            let spawned = std::thread::Builder::new()
                .name("udev-watch".into())
                .stack_size(64 * 1024)
                .spawn(move || {
                    run_watcher(fd, || {
                        UDEV.bump();
                        DEVICE_LIST.bump();
                    })
                });
            spawned.is_ok()
        }
        Err(e) => {
            log::warn!(
                "Cannot watch {} ({}); polling for device changes",
                UDEV_DATA,
                e
            );
            false
        }
    })
}

/// An inotify descriptor watching `dir` for entries appearing, changing
/// or disappearing. udev writes one file per device there.
fn watch_dir(dir: &Path) -> std::io::Result<OwnedFd> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: plain syscall; the returned fd is owned below.
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fd was just created and is not shared.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes())?;
    let mask = libc::IN_CREATE
        | libc::IN_DELETE
        | libc::IN_CLOSE_WRITE
        | libc::IN_MOVED_TO
        | libc::IN_MOVED_FROM;
    // SAFETY: path is a valid NUL-terminated string.
    if unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

/// Calls `on_change` once per burst of events. Returns if reading fails.
fn run_watcher(fd: OwnedFd, on_change: impl Fn()) {
    let mut buf = [0u8; 4096];
    loop {
        // SAFETY: buf is valid for writes of its length.
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n <= 0 {
            log::warn!("udev watcher stopped: {}", std::io::Error::last_os_error());
            return;
        }
        // A plug event touches several files; wait for the burst to end.
        std::thread::sleep(DEBOUNCE);
        drain(&fd, &mut buf);
        on_change();
    }
}

fn drain(fd: &OwnedFd, buf: &mut [u8]) {
    loop {
        let mut pfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pfd is a valid pollfd array of length 1.
        if unsafe { libc::poll(&mut pfd, 1, 0) } <= 0 {
            return;
        }
        // SAFETY: buf is valid for writes of its length.
        if unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) } <= 0 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn notifier_wakes_waiters() {
        let n = Arc::new(Notifier::new());
        let seen = n.current();
        assert_eq!(
            n.wait_past(seen, Duration::from_millis(10)),
            seen,
            "times out"
        );
        let n2 = Arc::clone(&n);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            n2.bump();
        });
        assert_eq!(n.wait_past(seen, Duration::from_secs(5)), seen + 1);
    }

    #[test]
    fn watcher_reports_bursts_once() {
        let dir = tempfile::tempdir().unwrap();
        let fd = watch_dir(dir.path()).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        std::thread::spawn(move || {
            run_watcher(fd, move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
        });
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("c189:{i}")), "x").unwrap();
        }
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "one notification per burst"
        );
        std::fs::remove_file(dir.path().join("c189:0")).unwrap();
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn missing_directory_is_an_error() {
        assert!(watch_dir(Path::new("/nonexistent/udev/data")).is_err());
    }
}
