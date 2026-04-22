//! Core Context Service Implementation
//!
//! Handles the primary Varlink interface for game detection, hardware inventory,
//! and system diagnostics.

use crate::contextd::*;
use crate::detectors::controllers::manager::ControllerManager;
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use std::sync::{Arc, RwLock};

/// Service implementation for the com.performativenonsense.contextd interface
pub struct ContextService {
    pub game_manager: Arc<RwLock<GameManager>>,
    pub hardware_manager: Arc<RwLock<HardwareManager>>,
    pub diagnostics_manager: Arc<RwLock<DiagnosticsManager>>,
    pub controller_manager: Arc<RwLock<ControllerManager>>,
}

impl VarlinkInterface for ContextService {
    /// Returns the currently active game/high-performance application
    fn get_active_game(&self, call: &mut dyn Call_GetActiveGame) -> varlink::Result<()> {
        let mut manager = self.game_manager.write().unwrap();
        call.reply(manager.get_active_game())
    }

    /// Lists all detected games installed on the system
    fn list_installed_games(&self, call: &mut dyn Call_ListInstalledGames) -> varlink::Result<()> {
        let mut manager = self.game_manager.write().unwrap();
        call.reply(manager.list_all_installed())
    }

    /// Lists connected gaming-relevant hardware (mice, keyboards, controllers)
    fn list_devices(&self, call: &mut dyn Call_ListDevices) -> varlink::Result<()> {
        let mut manager = self.hardware_manager.write().unwrap();
        let mut devices = manager.list_all_devices();

        self.merge_controller_hints(&mut devices);
        call.reply(devices)
    }

    /// Lists hardware with addressable RGB capabilities
    fn list_rgbdevices(&self, call: &mut dyn Call_ListRGBDevices) -> varlink::Result<()> {
        let mut manager = self.hardware_manager.write().unwrap();
        let mut devices = manager.list_rgb_devices();

        self.merge_controller_hints(&mut devices);
        call.reply(devices)
    }

    /// Returns hardware and software diagnostic info (GPU, Vulkan, RAM, etc.)
    fn get_diagnostics(&self, call: &mut dyn Call_GetDiagnostics) -> varlink::Result<()> {
        let mut manager = self.diagnostics_manager.write().unwrap();
        call.reply(manager.get_diagnostics())
    }

    /// Registers a new controller hint from an external application
    fn register_controller(
        &self,
        call: &mut dyn Call_RegisterController,
        controller: Controller,
    ) -> varlink::Result<()> {
        let mut manager = self.controller_manager.write().unwrap();
        manager.register(controller);
        call.reply()
    }

    /// Unregisters an existing controller hint
    fn unregister_controller(
        &self,
        call: &mut dyn Call_UnregisterController,
        pid: i64,
    ) -> varlink::Result<()> {
        let mut manager = self.controller_manager.write().unwrap();
        manager.unregister(pid);
        call.reply()
    }

    /// Lists all active controller hints currently registered
    fn list_controllers(&self, call: &mut dyn Call_ListControllers) -> varlink::Result<()> {
        let mut manager = self.controller_manager.write().unwrap();
        call.reply(manager.list_all())
    }
}

impl ContextService {
    /// Merges registered controller hints into the device list based on device paths
    fn merge_controller_hints(&self, devices: &mut [Device]) {
        let mut manager = self.controller_manager.write().unwrap();
        let controllers = manager.list_all();

        for device in devices {
            device.controllers = controllers
                .iter()
                .filter(|c| c.interested_devices.contains(&device.path))
                .cloned()
                .collect();
        }
    }
}
