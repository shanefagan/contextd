//! Varlink connection handling.
//!
//! Each connection gets its own thread, which lets long-lived subscriptions
//! and idle clients coexist with ordinary calls. Three limits keep one
//! local user from starving the daemon:
//!
//! - a cap on concurrent connections per socket, and per UID;
//! - a cap on the size of a single message;
//! - read and write timeouts, so idle or stuck clients are dropped.

use crate::auth::{self, Caller, PeerInfo};
use crate::util::lock;
use std::collections::HashMap;
use std::io::{self, BufReader, Read};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use varlink::{ConnectionHandler, VarlinkService};

/// Per-socket limits.
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_connections: usize,
    pub max_connections_per_uid: usize,
    /// Largest single Varlink message accepted, in bytes.
    pub max_message: usize,
    /// Connections that send nothing for this long are closed.
    pub idle_timeout: Duration,
    /// A write that cannot complete in this time closes the connection.
    pub write_timeout: Duration,
}

impl Limits {
    pub fn from_config(max_message: usize) -> Self {
        let cfg = &crate::config::get().server;
        Self {
            max_connections: cfg.max_connections,
            max_connections_per_uid: cfg.max_connections_per_uid,
            max_message,
            idle_timeout: Duration::from_secs(cfg.idle_timeout_secs.max(1)),
            write_timeout: Duration::from_secs(10),
        }
    }
}

/// Serves Varlink requests on `listener` until accepting fails fatally.
pub fn serve(
    name: &'static str,
    listener: UnixListener,
    service: VarlinkService,
    limits: Limits,
) -> anyhow::Result<()> {
    let service = Arc::new(service);
    let tracker = Arc::new(Tracker::default());

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                log::error!("[{}] Error accepting connection: {}", name, e);
                // Usually EMFILE/ENFILE; back off instead of spinning.
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        let peer = PeerInfo::from_stream(&stream);
        let uid = peer.as_ref().map(|p| p.uid).unwrap_or(u32::MAX);
        let Some(slot) = Tracker::acquire(&tracker, uid, &limits) else {
            tracker.warn_rejected(name, uid);
            continue; // dropping the stream closes it
        };

        if let Err(e) = stream
            .set_read_timeout(Some(limits.idle_timeout))
            .and_then(|_| stream.set_write_timeout(Some(limits.write_timeout)))
        {
            log::warn!("[{}] Cannot set socket timeouts: {}", name, e);
            continue;
        }

        let service = Arc::clone(&service);
        let max_message = limits.max_message;
        let spawned = std::thread::Builder::new()
            .name(format!("{name}-conn"))
            .stack_size(256 * 1024)
            .spawn(move || {
                let _slot = slot;
                auth::set_current(Some(Caller {
                    peer,
                    fd: stream.as_raw_fd(),
                }));
                let mut reader = BufReader::new(MessageLimit::new(&stream, max_message));
                let mut writer = &stream;
                if let Err(e) = service.handle(&mut reader, &mut writer, None) {
                    log::debug!("Connection closed: {}", e);
                }
                auth::set_current(None);
            });
        if let Err(e) = spawned {
            log::error!("[{}] Cannot spawn connection thread: {}", name, e);
        }
    }

    Ok(())
}

#[derive(Default)]
struct Tracker {
    state: Mutex<TrackerState>,
}

#[derive(Default)]
struct TrackerState {
    total: usize,
    per_uid: HashMap<u32, usize>,
    last_warning: Option<Instant>,
}

impl Tracker {
    fn acquire(this: &Arc<Self>, uid: u32, limits: &Limits) -> Option<Slot> {
        let mut s = lock(&this.state);
        let mine = s.per_uid.get(&uid).copied().unwrap_or(0);
        if s.total >= limits.max_connections || mine >= limits.max_connections_per_uid {
            return None;
        }
        s.total += 1;
        s.per_uid.insert(uid, mine + 1);
        Some(Slot {
            tracker: Arc::clone(this),
            uid,
        })
    }

    fn warn_rejected(&self, name: &str, uid: u32) {
        let mut s = lock(&self.state);
        let now = Instant::now();
        if s.last_warning
            .is_none_or(|t| now.duration_since(t) > Duration::from_secs(10))
        {
            s.last_warning = Some(now);
            log::warn!(
                "[{}] Connection limit reached (uid {}); rejecting connections",
                name,
                uid
            );
        }
    }
}

/// Releases a connection's place in the tracker when the connection ends,
/// including when its thread panics.
struct Slot {
    tracker: Arc<Tracker>,
    uid: u32,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut s = lock(&self.tracker.state);
        s.total = s.total.saturating_sub(1);
        if let Some(n) = s.per_uid.get_mut(&self.uid) {
            *n -= 1;
            if *n == 0 {
                s.per_uid.remove(&self.uid);
            }
        }
    }
}

/// A reader that fails once a single NUL-terminated message grows beyond
/// `max` bytes. The varlink crate buffers until it sees a NUL, so without
/// this a client could make the daemon allocate without bound.
pub struct MessageLimit<R> {
    inner: R,
    since_nul: usize,
    max: usize,
}

impl<R> MessageLimit<R> {
    pub fn new(inner: R, max: usize) -> Self {
        Self {
            inner,
            since_nul: 0,
            max,
        }
    }
}

impl<R: Read> Read for MessageLimit<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        for &b in &buf[..n] {
            if b == 0 {
                self.since_nul = 0;
            } else {
                self.since_nul += 1;
                if self.since_nul > self.max {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("message exceeds {} bytes", self.max),
                    ));
                }
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_limit_allows_many_small_messages() {
        let data = b"abc\0def\0ghi\0".repeat(1000);
        let mut r = MessageLimit::new(&data[..], 4);
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out.len(), data.len());
    }

    #[test]
    fn message_limit_rejects_long_message() {
        let data = [b'A'; 100];
        let mut r = MessageLimit::new(&data[..], 50);
        let mut out = Vec::new();
        assert!(r.read_to_end(&mut out).is_err());
    }

    #[test]
    fn tracker_enforces_and_releases_limits() {
        let limits = Limits {
            max_connections: 3,
            max_connections_per_uid: 2,
            max_message: 1,
            idle_timeout: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
        };
        let t = Arc::new(Tracker::default());
        let a1 = Tracker::acquire(&t, 1000, &limits).unwrap();
        let _a2 = Tracker::acquire(&t, 1000, &limits).unwrap();
        assert!(Tracker::acquire(&t, 1000, &limits).is_none(), "per-uid cap");
        let _b1 = Tracker::acquire(&t, 1001, &limits).unwrap();
        assert!(Tracker::acquire(&t, 1002, &limits).is_none(), "global cap");
        drop(a1);
        assert!(
            Tracker::acquire(&t, 1000, &limits).is_some(),
            "slot released"
        );
    }
}
