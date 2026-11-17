//! Window focus, as reported by a helper in each graphical session.
//!
//! contextd runs outside the user's session and cannot see windows. A
//! session helper (`contextd-focus`) asks the compositor or X server which
//! process owns the focused window and calls `ReportFocus`. Reports are
//! kept per user, are only accepted for the reporting user's own
//! processes, and expire when the reporting process exits.

use crate::auth::PeerInfo;
use crate::detectors::controllers::manager::ProcessSource;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum FocusError {
    Invalid(String),
    NotOwner(i64),
}

#[derive(Debug, Clone)]
struct Report {
    /// Focused process and its start time; None if nothing is focused.
    focused: Option<(i32, u64)>,
    reporter: (i32, u64),
    at: Instant,
}

/// What is known about focus for one lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// No reporter is running: focus is unknown.
    Unknown,
    /// A reporter says nothing (or an unknown process) has focus.
    Nothing,
    /// This process owns the focused window.
    Pid(i32),
}

#[derive(Default)]
pub struct FocusTracker {
    by_uid: HashMap<u32, Report>,
}

impl FocusTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a focus report from `peer`.
    pub fn report(
        &mut self,
        peer: Option<&PeerInfo>,
        pid: Option<i64>,
        procs: &dyn ProcessSource,
    ) -> Result<(), FocusError> {
        let peer = peer.ok_or_else(|| FocusError::Invalid("cannot identify caller".into()))?;
        let reporter_start = procs
            .start_time(peer.pid)
            .ok_or_else(|| FocusError::Invalid("caller has exited".into()))?;

        let mut owner_uid = peer.uid;
        let focused = match pid {
            None | Some(0) => None,
            Some(pid) => {
                let pid32 = i32::try_from(pid)
                    .map_err(|_| FocusError::Invalid(format!("pid {pid} is out of range")))?;
                let status = procs
                    .status(pid32)
                    .ok_or_else(|| FocusError::Invalid(format!("pid {pid} does not exist")))?;
                if status.euid != peer.uid && peer.uid != 0 {
                    return Err(FocusError::NotOwner(pid));
                }
                // A root reporter speaks for the user who owns the window.
                owner_uid = status.euid;
                let start = procs
                    .start_time(pid32)
                    .ok_or_else(|| FocusError::Invalid(format!("pid {pid} has exited")))?;
                Some((pid32, start))
            }
        };

        self.by_uid.insert(
            owner_uid,
            Report {
                focused,
                reporter: (peer.pid, reporter_start),
                at: Instant::now(),
            },
        );
        Ok(())
    }

    /// Focus for `uid` if its session has a reporter, otherwise for the
    /// most recently reporting session, otherwise unknown.
    pub fn current(&mut self, uid: Option<u32>, procs: &dyn ProcessSource) -> Focus {
        self.by_uid
            .retain(|_, r| procs.start_time(r.reporter.0) == Some(r.reporter.1));

        let report = uid
            .and_then(|u| self.by_uid.get(&u))
            .or_else(|| self.by_uid.values().max_by_key(|r| r.at));
        match report {
            None => Focus::Unknown,
            Some(r) => match r.focused {
                Some((pid, start)) if procs.start_time(pid) == Some(start) => Focus::Pid(pid),
                _ => Focus::Nothing,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procfs::ProcStatus;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeProcs(RefCell<HashMap<i32, (i32, u32, u64)>>);

    impl FakeProcs {
        fn add(&self, pid: i32, uid: u32, start: u64) {
            self.0.borrow_mut().insert(pid, (1, uid, start));
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

    #[test]
    fn unknown_until_reported_and_after_reporter_exits() {
        let p = FakeProcs::default();
        p.add(10, 1000, 1); // helper
        p.add(20, 1000, 2); // game
        let mut f = FocusTracker::new();
        assert_eq!(f.current(Some(1000), &p), Focus::Unknown);
        f.report(Some(&peer(10, 1000)), Some(20), &p).unwrap();
        assert_eq!(f.current(Some(1000), &p), Focus::Pid(20));
        f.report(Some(&peer(10, 1000)), None, &p).unwrap();
        assert_eq!(f.current(Some(1000), &p), Focus::Nothing);
        p.kill(10);
        assert_eq!(f.current(Some(1000), &p), Focus::Unknown);
    }

    #[test]
    fn cannot_report_other_users_processes() {
        let p = FakeProcs::default();
        p.add(10, 1000, 1);
        p.add(30, 1001, 3);
        let mut f = FocusTracker::new();
        assert_eq!(
            f.report(Some(&peer(10, 1000)), Some(30), &p),
            Err(FocusError::NotOwner(30))
        );
        assert!(f.report(Some(&peer(10, 1000)), Some(999), &p).is_err());
    }

    #[test]
    fn callers_own_session_wins_then_most_recent() {
        let p = FakeProcs::default();
        p.add(10, 1000, 1);
        p.add(11, 1001, 1);
        p.add(20, 1000, 2);
        p.add(21, 1001, 2);
        let mut f = FocusTracker::new();
        f.report(Some(&peer(10, 1000)), Some(20), &p).unwrap();
        f.report(Some(&peer(11, 1001)), Some(21), &p).unwrap();
        assert_eq!(f.current(Some(1000), &p), Focus::Pid(20));
        assert_eq!(f.current(Some(1001), &p), Focus::Pid(21));
        assert_eq!(f.current(Some(0), &p), Focus::Pid(21), "most recent report");
    }

    #[test]
    fn focused_process_exit_means_nothing_focused() {
        let p = FakeProcs::default();
        p.add(10, 1000, 1);
        p.add(20, 1000, 2);
        let mut f = FocusTracker::new();
        f.report(Some(&peer(10, 1000)), Some(20), &p).unwrap();
        p.kill(20);
        p.add(20, 1000, 99); // PID reused
        assert_eq!(f.current(Some(1000), &p), Focus::Nothing);
    }
}
