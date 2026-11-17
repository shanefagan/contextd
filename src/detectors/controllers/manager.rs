//! Controller Manager
//!
//! Keeps hints from applications that manage specific hardware, so other
//! tools can see who is in charge of a device.
//!
//! Hints are self-attested, but contextd checks who is making them: the PID
//! in a hint must be the caller or one of its ancestors, owned by the same
//! user, and only that user (or root) can remove it. Hints are dropped when
//! the process exits, using the process start time to survive PID reuse.

use crate::auth::PeerInfo;
use crate::contextd::Controller;
use crate::procfs::{self, ProcStatus};
use std::collections::HashMap;

pub const MAX_HINTS_TOTAL: usize = 256;
pub const MAX_HINTS_PER_UID: usize = 32;
const MAX_NAME: usize = 128;
const MAX_VERSION: usize = 64;
const MAX_TEXT: usize = 512;
const MAX_LIST: usize = 64;
const MAX_LIST_ITEM: usize = 256;
/// How far up the process tree a caller may register on behalf of.
const MAX_ANCESTOR_DEPTH: usize = 16;

/// Why a hint was rejected.
#[derive(Debug, PartialEq, Eq)]
pub enum HintError {
    Invalid(String),
    NotOwner(i64),
    Limit(&'static str, usize),
}

/// Process information the manager needs; abstracted for tests.
pub trait ProcessSource {
    fn status(&self, pid: i32) -> Option<ProcStatus>;
    fn start_time(&self, pid: i32) -> Option<u64>;
}

/// Reads from `/proc`.
pub struct SystemProcesses;

impl ProcessSource for SystemProcesses {
    fn status(&self, pid: i32) -> Option<ProcStatus> {
        procfs::status(pid)
    }
    fn start_time(&self, pid: i32) -> Option<u64> {
        procfs::start_time(pid)
    }
}

struct Entry {
    controller: Controller,
    owner_uid: u32,
    start_time: u64,
}

/// Manages registration and lifecycle of hardware controller hints
#[derive(Default)]
pub struct ControllerManager {
    controllers: HashMap<i64, Entry>,
}

impl ControllerManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new controller hint or updates the caller's existing one.
    ///
    /// `controller.pid` may be 0, meaning "the calling process". Sandboxed
    /// callers (Flatpak) must use 0, because their own PID differs from the
    /// one contextd sees.
    pub fn register(
        &mut self,
        peer: Option<&PeerInfo>,
        mut controller: Controller,
        procs: &dyn ProcessSource,
    ) -> Result<(), HintError> {
        let peer = peer.ok_or_else(|| HintError::Invalid("cannot identify caller".into()))?;
        validate(&controller)?;

        let pid = if controller.pid == 0 {
            peer.pid
        } else {
            i32::try_from(controller.pid).map_err(|_| {
                HintError::Invalid(format!("pid {} is out of range", controller.pid))
            })?
        };
        if !is_self_or_ancestor(procs, peer, pid) {
            return Err(HintError::Invalid(format!(
                "pid {pid} is not the caller or one of its ancestors owned by uid {}; pass pid 0 to use the calling process",
                peer.uid
            )));
        }
        let start_time = procs
            .start_time(pid)
            .ok_or_else(|| HintError::Invalid(format!("pid {pid} has exited")))?;

        self.prune_stale(procs);
        let key = pid as i64;
        let is_update = self.controllers.contains_key(&key);
        if !is_update {
            if self.controllers.len() >= MAX_HINTS_TOTAL {
                return Err(HintError::Limit("hints_total", MAX_HINTS_TOTAL));
            }
            let mine = self
                .controllers
                .values()
                .filter(|e| e.owner_uid == peer.uid)
                .count();
            if mine >= MAX_HINTS_PER_UID {
                return Err(HintError::Limit("hints_per_uid", MAX_HINTS_PER_UID));
            }
        }

        controller.pid = key;
        log::info!(
            "Registering controller hint: {} (PID: {}, uid {})",
            controller.name,
            pid,
            peer.uid
        );
        self.controllers.insert(
            key,
            Entry {
                controller,
                owner_uid: peer.uid,
                start_time,
            },
        );
        Ok(())
    }

    /// Removes a hint. Removing a hint that does not exist succeeds.
    pub fn unregister(&mut self, peer: Option<&PeerInfo>, pid: i64) -> Result<(), HintError> {
        let peer = peer.ok_or_else(|| HintError::Invalid("cannot identify caller".into()))?;
        let pid = if pid == 0 { peer.pid as i64 } else { pid };
        if let Some(entry) = self.controllers.get(&pid) {
            if entry.owner_uid != peer.uid && peer.uid != 0 {
                return Err(HintError::NotOwner(pid));
            }
            log::info!("Unregistering controller hint for PID: {}", pid);
            self.controllers.remove(&pid);
        }
        Ok(())
    }

    /// Lists all active controller hints, pruning stale ones.
    pub fn list_all(&mut self, procs: &dyn ProcessSource) -> Vec<Controller> {
        self.prune_stale(procs);
        let mut list: Vec<Controller> = self
            .controllers
            .values()
            .map(|e| e.controller.clone())
            .collect();
        list.sort_by_key(|c| c.pid);
        list
    }

    /// Removes hints whose process has exited or whose PID was reused.
    fn prune_stale(&mut self, procs: &dyn ProcessSource) {
        let before = self.controllers.len();
        self.controllers.retain(|pid, entry| {
            let alive = procs.start_time(*pid as i32) == Some(entry.start_time);
            if !alive {
                log::debug!(
                    "Pruning stale controller hint: {} (PID: {})",
                    entry.controller.name,
                    pid
                );
            }
            alive
        });
        let pruned = before - self.controllers.len();
        if pruned > 0 {
            log::info!("Pruned {} stale controller hints", pruned);
        }
    }
}

fn is_self_or_ancestor(procs: &dyn ProcessSource, peer: &PeerInfo, pid: i32) -> bool {
    let mut current = peer.pid;
    for _ in 0..MAX_ANCESTOR_DEPTH {
        if current <= 0 {
            return false;
        }
        let Some(status) = procs.status(current) else {
            return false;
        };
        if status.euid != peer.uid {
            return false;
        }
        if current == pid {
            return true;
        }
        current = status.ppid;
    }
    false
}

fn validate(c: &Controller) -> Result<(), HintError> {
    fn check(field: &str, value: &str, max: usize) -> Result<(), HintError> {
        if value.len() > max {
            return Err(HintError::Invalid(format!(
                "{field} is longer than {max} bytes"
            )));
        }
        Ok(())
    }
    fn check_list(field: &str, list: &[String]) -> Result<(), HintError> {
        if list.len() > MAX_LIST {
            return Err(HintError::Invalid(format!(
                "{field} has more than {MAX_LIST} entries"
            )));
        }
        list.iter()
            .try_for_each(|item| check(field, item, MAX_LIST_ITEM))
    }

    if c.name.trim().is_empty() {
        return Err(HintError::Invalid("name is empty".into()));
    }
    check("name", &c.name, MAX_NAME)?;
    if let Some(v) = &c.version {
        check("version", v, MAX_VERSION)?;
    }
    if let Some(v) = &c.description {
        check("description", v, MAX_TEXT)?;
    }
    if let Some(v) = &c.website {
        check("website", v, MAX_TEXT)?;
    }
    check_list("capabilities", &c.capabilities)?;
    check_list("interested_devices", &c.interested_devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// pid -> (ppid, uid, start_time)
    #[derive(Default)]
    struct FakeProcs(RefCell<HashMap<i32, (i32, u32, u64)>>);

    impl FakeProcs {
        fn add(&self, pid: i32, ppid: i32, uid: u32, start: u64) {
            self.0.borrow_mut().insert(pid, (ppid, uid, start));
        }
        fn kill(&self, pid: i32) {
            self.0.borrow_mut().remove(&pid);
        }
    }

    impl ProcessSource for FakeProcs {
        fn status(&self, pid: i32) -> Option<ProcStatus> {
            self.0
                .borrow()
                .get(&pid)
                .map(|&(ppid, euid, _)| ProcStatus { ppid, euid })
        }
        fn start_time(&self, pid: i32) -> Option<u64> {
            self.0.borrow().get(&pid).map(|&(_, _, s)| s)
        }
    }

    fn peer(pid: i32, uid: u32) -> PeerInfo {
        PeerInfo {
            pid,
            uid,
            gid: uid,
            unit: None,
        }
    }

    fn hint(name: &str, pid: i64) -> Controller {
        Controller {
            name: name.into(),
            version: None,
            pid,
            description: None,
            website: None,
            capabilities: vec![],
            interested_devices: vec!["/dev/hidraw0".into()],
        }
    }

    fn setup() -> FakeProcs {
        let p = FakeProcs::default();
        p.add(1, 0, 0, 1);
        p.add(100, 1, 1000, 10); // user's shell
        p.add(101, 100, 1000, 11); // varlinkctl started by the shell
        p.add(200, 1, 1001, 20); // another user's process
        p
    }

    #[test]
    fn caller_can_register_itself_and_its_parent() {
        let p = setup();
        let mut m = ControllerManager::new();
        m.register(Some(&peer(101, 1000)), hint("self", 101), &p)
            .unwrap();
        m.register(Some(&peer(101, 1000)), hint("parent", 100), &p)
            .unwrap();
        assert_eq!(m.list_all(&p).len(), 2);
    }

    #[test]
    fn pid_zero_means_caller() {
        let p = setup();
        let mut m = ControllerManager::new();
        m.register(Some(&peer(101, 1000)), hint("sandboxed", 0), &p)
            .unwrap();
        assert_eq!(m.list_all(&p)[0].pid, 101);
    }

    #[test]
    fn cannot_register_for_unrelated_or_other_users_processes() {
        let p = setup();
        let mut m = ControllerManager::new();
        let me = peer(101, 1000);
        assert!(matches!(
            m.register(Some(&me), hint("x", 200), &p),
            Err(HintError::Invalid(_))
        ));
        assert!(matches!(
            m.register(Some(&me), hint("x", 1), &p),
            Err(HintError::Invalid(_))
        ));
    }

    #[test]
    fn only_owner_or_root_can_unregister() {
        let p = setup();
        let mut m = ControllerManager::new();
        m.register(Some(&peer(100, 1000)), hint("mine", 100), &p)
            .unwrap();
        assert_eq!(
            m.unregister(Some(&peer(200, 1001)), 100),
            Err(HintError::NotOwner(100))
        );
        assert_eq!(m.list_all(&p).len(), 1);
        m.unregister(Some(&peer(5, 0)), 100).unwrap();
        assert!(m.list_all(&p).is_empty());
    }

    #[test]
    fn other_user_cannot_overwrite_hint() {
        let p = setup();
        let mut m = ControllerManager::new();
        m.register(Some(&peer(100, 1000)), hint("legit", 100), &p)
            .unwrap();
        assert!(
            m.register(Some(&peer(200, 1001)), hint("impostor", 100), &p)
                .is_err()
        );
        assert_eq!(m.list_all(&p)[0].name, "legit");
    }

    #[test]
    fn exited_and_reused_pids_are_pruned() {
        let p = setup();
        let mut m = ControllerManager::new();
        m.register(Some(&peer(101, 1000)), hint("short-lived", 101), &p)
            .unwrap();
        p.kill(101);
        p.add(101, 1, 1001, 99); // PID reused by someone else
        assert!(m.list_all(&p).is_empty());
    }

    #[test]
    fn oversized_fields_are_rejected() {
        let p = setup();
        let mut m = ControllerManager::new();
        let mut h = hint(&"n".repeat(MAX_NAME + 1), 100);
        assert!(m.register(Some(&peer(100, 1000)), h.clone(), &p).is_err());
        h.name = "ok".into();
        h.interested_devices = vec!["/dev/x".into(); MAX_LIST + 1];
        assert!(m.register(Some(&peer(100, 1000)), h, &p).is_err());
    }

    #[test]
    fn per_uid_limit_applies() {
        let p = FakeProcs::default();
        let mut m = ControllerManager::new();
        for i in 0..=MAX_HINTS_PER_UID as i32 {
            p.add(1000 + i, 1, 1000, 5);
            let r = m.register(Some(&peer(1000 + i, 1000)), hint("h", 0), &p);
            if (i as usize) < MAX_HINTS_PER_UID {
                r.unwrap();
            } else {
                assert_eq!(r, Err(HintError::Limit("hints_per_uid", MAX_HINTS_PER_UID)));
            }
        }
    }
}
