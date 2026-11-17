//! Core Context Service Implementation
//!
//! Handles the primary Varlink interface for game detection, hardware
//! inventory, system diagnostics and controller hints.

use crate::auth;
use crate::contextd::*;
use crate::detectors::controllers::manager::{ControllerManager, HintError, SystemProcesses};
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::util::lock;
use std::sync::Mutex;

/// Service implementation for the com.performativenonsense.contextd interface
pub struct ContextService {
    pub games: Mutex<GameManager>,
    pub hardware: Mutex<HardwareManager>,
    pub diagnostics: Mutex<DiagnosticsManager>,
    pub controllers: Mutex<ControllerManager>,
}

impl VarlinkInterface for ContextService {
    fn get_active_game(&self, call: &mut dyn Call_GetActiveGame) -> varlink::Result<()> {
        let game = lock(&self.games).running().into_iter().next();
        call.reply(game)
    }

    fn list_running_games(&self, call: &mut dyn Call_ListRunningGames) -> varlink::Result<()> {
        let games = lock(&self.games).running();
        call.reply(games)
    }

    fn list_installed_games(&self, call: &mut dyn Call_ListInstalledGames) -> varlink::Result<()> {
        let games = lock(&self.games).installed();
        call.reply(games)
    }

    fn list_devices(&self, call: &mut dyn Call_ListDevices) -> varlink::Result<()> {
        let mut devices = lock(&self.hardware).list_all_devices();
        self.merge_controller_hints(&mut devices);
        call.reply(devices)
    }

    fn list_rgbdevices(&self, call: &mut dyn Call_ListRGBDevices) -> varlink::Result<()> {
        let mut devices = lock(&self.hardware).list_rgb_devices();
        self.merge_controller_hints(&mut devices);
        call.reply(devices)
    }

    fn get_diagnostics(&self, call: &mut dyn Call_GetDiagnostics) -> varlink::Result<()> {
        let diagnostics = lock(&self.diagnostics).get_diagnostics();
        call.reply(diagnostics)
    }

    fn register_controller(
        &self,
        call: &mut dyn Call_RegisterController,
        controller: Controller,
    ) -> varlink::Result<()> {
        let peer = auth::current_peer();
        let result = lock(&self.controllers).register(peer.as_ref(), controller, &SystemProcesses);
        match result {
            Ok(()) => call.reply(),
            Err(e) => reply_hint_error(call, e),
        }
    }

    fn unregister_controller(
        &self,
        call: &mut dyn Call_UnregisterController,
        pid: i64,
    ) -> varlink::Result<()> {
        let peer = auth::current_peer();
        let result = lock(&self.controllers).unregister(peer.as_ref(), pid);
        match result {
            Ok(()) => call.reply(),
            Err(e) => reply_hint_error(call, e),
        }
    }

    fn list_controllers(&self, call: &mut dyn Call_ListControllers) -> varlink::Result<()> {
        let controllers = lock(&self.controllers).list_all(&SystemProcesses);
        call.reply(controllers)
    }
}

fn reply_hint_error(call: &mut dyn VarlinkCallError, e: HintError) -> varlink::Result<()> {
    match e {
        HintError::Invalid(reason) => call.reply_invalid_controller(reason),
        HintError::NotOwner(pid) => call.reply_not_owner(pid),
        HintError::Limit(limit, max) => call.reply_limit_exceeded(limit.to_string(), max as i64),
    }
}

impl ContextService {
    /// Attaches registered controller hints to every device they name.
    fn merge_controller_hints(&self, devices: &mut [Device]) {
        let controllers = lock(&self.controllers).list_all(&SystemProcesses);
        for device in devices {
            device.controllers = controllers
                .iter()
                .filter(|c| {
                    c.interested_devices
                        .iter()
                        .any(|p| device.paths.contains(p) || *p == device.path)
                })
                .cloned()
                .collect();
        }
    }
}
