//! Controller Manager
//!
//! Manages hints from external applications that are interested in or controlling
//! specific hardware devices.

use crate::contextd::Controller;
use std::collections::HashMap;
use sysinfo::{Pid, System};

/// Manages registration and lifecycle of hardware controller hints
pub struct ControllerManager {
    controllers: HashMap<i64, Controller>,
    sys: System,
}

impl ControllerManager {
    /// Creates a new ControllerManager
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_processes();
        Self {
            controllers: HashMap::new(),
            sys,
        }
    }

    /// Registers a new controller hint or updates an existing one
    pub fn register(&mut self, controller: Controller) {
        log::info!(
            "Registering controller hint: {} (PID: {})",
            controller.name,
            controller.pid
        );
        self.controllers.insert(controller.pid, controller);
    }

    /// Explicitly unregisters a controller hint
    pub fn unregister(&mut self, pid: i64) {
        log::info!("Unregistering controller hint for PID: {}", pid);
        self.controllers.remove(&pid);
    }

    /// Lists all active controller hints, pruning stale ones
    pub fn list_all(&mut self) -> Vec<Controller> {
        self.prune_stale();
        self.controllers.values().cloned().collect()
    }

    /// Removes controller hints for processes that are no longer running
    fn prune_stale(&mut self) {
        self.sys.refresh_processes();
        let initial_count = self.controllers.len();

        self.controllers.retain(|pid, controller| {
            let pid_val = *pid as usize;
            let exists = self.sys.process(Pid::from(pid_val)).is_some();
            if !exists {
                log::debug!(
                    "Pruning stale controller hint: {} (PID: {})",
                    controller.name,
                    pid
                );
            }
            exists
        });

        let pruned = initial_count - self.controllers.len();
        if pruned > 0 {
            log::info!("Pruned {} stale controller hints", pruned);
        }
    }
}

impl Default for ControllerManager {
    fn default() -> Self {
        Self::new()
    }
}
