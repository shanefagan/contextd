//! Core Context Service Implementation
//!
//! Handles the primary Varlink interface for game detection, hardware
//! inventory, system diagnostics and controller hints.

use crate::auth;
use crate::contextd::*;
use crate::detectors::controllers::manager::{ControllerManager, HintError, SystemProcesses};
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::focus::{Focus, FocusError, FocusTracker};
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::util::lock;
use crate::watch::{self, Notifier};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;

/// How often a game subscription re-checks for new or exited games.
/// Scans are shared through the manager's cache, so many subscribers
/// still cost one scan per interval.
const GAMES_POLL: Duration = Duration::from_secs(1);
/// How fresh device scans must be when the udev watcher is unavailable.
const DEVICES_POLL: Duration = Duration::from_secs(2);
/// Subscriptions check for a hung-up client at least this often.
const HANGUP_CHECK: Duration = Duration::from_secs(1);

/// Runs a subscription: sends `fetch()` now and again whenever its result
/// changes, re-checking when `notifier` fires or after `HANGUP_CHECK`.
/// Without "more" it behaves like the plain List call.
fn stream<T: Serialize>(
    wants_more: bool,
    notifier: &Notifier,
    mut fetch: impl FnMut() -> T,
    mut send: impl FnMut(T, bool) -> varlink::Result<()>,
) -> varlink::Result<()> {
    let value = fetch();
    if !wants_more {
        return send(value, false);
    }
    let mut last = serde_json::to_string(&value).unwrap_or_default();
    send(value, true)?;
    let mut seen = notifier.current();
    loop {
        seen = notifier.wait_past(seen, HANGUP_CHECK);
        if auth::current_connection_closed() {
            return Ok(());
        }
        let value = fetch();
        let json = serde_json::to_string(&value).unwrap_or_default();
        if json != last {
            last = json;
            if let Err(e) = send(value, true) {
                log::debug!("Subscriber disconnected: {}", e);
                return Ok(());
            }
        }
    }
}

/// Service implementation for the com.performativenonsense.contextd interface
pub struct ContextService {
    pub games: Mutex<GameManager>,
    pub hardware: Mutex<HardwareManager>,
    pub diagnostics: Mutex<DiagnosticsManager>,
    pub controllers: Mutex<ControllerManager>,
    pub focus: Mutex<FocusTracker>,
}

impl VarlinkInterface for ContextService {
    fn get_active_game(&self, call: &mut dyn Call_GetActiveGame) -> varlink::Result<()> {
        let game = self.running_games().into_iter().next();
        call.reply(game)
    }

    fn list_running_games(&self, call: &mut dyn Call_ListRunningGames) -> varlink::Result<()> {
        call.reply(self.running_games())
    }

    fn subscribe_running_games(
        &self,
        call: &mut dyn Call_SubscribeRunningGames,
    ) -> varlink::Result<()> {
        let wants_more = call.wants_more();
        stream(
            wants_more,
            &watch::GAMES,
            || self.running_games_max_age(GAMES_POLL),
            |games, more| {
                call.set_continues(more);
                call.reply(games)
            },
        )
    }

    fn subscribe_devices(&self, call: &mut dyn Call_SubscribeDevices) -> varlink::Result<()> {
        let wants_more = call.wants_more();
        // With the udev watcher, scans are redone exactly when udev's
        // database changes; without it, poll.
        let max_age = if wants_more && watch::ensure_udev_watcher() {
            Duration::from_secs(crate::config::get().ttls.hardware.max(60))
        } else {
            DEVICES_POLL
        };
        stream(
            wants_more,
            &watch::DEVICE_LIST,
            || {
                let mut devices = lock(&self.hardware).list_all_devices_max_age(max_age);
                self.merge_controller_hints(&mut devices);
                devices
            },
            |devices, more| {
                call.set_continues(more);
                call.reply(devices)
            },
        )
    }

    fn report_focus(
        &self,
        call: &mut dyn Call_ReportFocus,
        pid: Option<i64>,
    ) -> varlink::Result<()> {
        let peer = auth::current_peer();
        let result = lock(&self.focus).report(peer.as_ref(), pid, &SystemProcesses);
        if result.is_ok() {
            watch::GAMES.bump();
        }
        match result {
            Ok(()) => call.reply(),
            Err(FocusError::Invalid(reason)) => call.reply_invalid_focus(reason),
            Err(FocusError::NotOwner(pid)) => call.reply_not_owner(pid),
        }
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
        if result.is_ok() {
            watch::DEVICE_LIST.bump();
        }
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
        if result.is_ok() {
            watch::DEVICE_LIST.bump();
        }
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
    /// Running games, using the focus reported for the caller's session.
    fn running_games(&self) -> Vec<Game> {
        let ttl = Duration::from_secs(crate::config::get().ttls.games);
        self.running_games_max_age(ttl)
    }

    fn running_games_max_age(&self, max_age: Duration) -> Vec<Game> {
        let uid = auth::current_peer().map(|p| p.uid);
        let focus: Focus = lock(&self.focus).current(uid, &SystemProcesses);
        lock(&self.games).running_focused_max_age(focus, &SystemProcesses, max_age)
    }

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
