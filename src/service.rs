use crate::contextd::*;
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use std::sync::{Arc, RwLock};

pub struct ContextService {
    pub game_manager: Arc<RwLock<GameManager>>,
    pub hardware_manager: Arc<RwLock<HardwareManager>>,
}

impl VarlinkInterface for ContextService {
    fn get_active_game(&self, call: &mut dyn Call_GetActiveGame) -> varlink::Result<()> {
        let mut manager = self.game_manager.write().unwrap();
        call.reply(manager.get_active_game())
    }

    fn list_installed_games(&self, call: &mut dyn Call_ListInstalledGames) -> varlink::Result<()> {
        let mut manager = self.game_manager.write().unwrap();
        call.reply(manager.list_all_installed())
    }

    fn list_devices(&self, call: &mut dyn Call_ListDevices) -> varlink::Result<()> {
        let mut manager = self.hardware_manager.write().unwrap();
        call.reply(manager.list_all_devices())
    }

    fn list_rgbdevices(&self, call: &mut dyn Call_ListRGBDevices) -> varlink::Result<()> {
        let mut manager = self.hardware_manager.write().unwrap();
        call.reply(manager.list_rgb_devices())
    }
}
