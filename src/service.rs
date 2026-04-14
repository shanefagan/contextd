//! Core Context Service Implementation
//!
//! Handles the primary Varlink interface for game detection, hardware inventory,
//! and system diagnostics.

use crate::contextd::*;
use crate::detectors::diagnostics::manager::DiagnosticsManager;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use std::sync::{Arc, RwLock};

/// Service implementation for the com.performativenonsense.contextd interface
pub struct ContextService {
    pub game_manager: Arc<RwLock<GameManager>>,
    pub hardware_manager: Arc<RwLock<HardwareManager>>,
    pub diagnostics_manager: Arc<RwLock<DiagnosticsManager>>,
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
        call.reply(manager.list_all_devices())
    }

    /// Lists hardware with addressable RGB capabilities
    fn list_rgbdevices(&self, call: &mut dyn Call_ListRGBDevices) -> varlink::Result<()> {
        let mut manager = self.hardware_manager.write().unwrap();
        call.reply(manager.list_rgb_devices())
    }

    /// Returns hardware and software diagnostic info (GPU, Vulkan, RAM, etc.)
    fn get_diagnostics(&self, call: &mut dyn Call_GetDiagnostics) -> varlink::Result<()> {
        let mut manager = self.diagnostics_manager.write().unwrap();
        call.reply(manager.get_diagnostics())
    }
}
