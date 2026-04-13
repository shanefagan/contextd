use std::sync::{Arc, RwLock};
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::contextd::*;

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

    fn subscribe(&self, call: &mut dyn Call_Subscribe) -> varlink::Result<()> {
        if call.is_oneway() {
            return Ok(());
        }
        
        // For now, just send one dummy event to show it works
        call.set_continues(true);
        call.reply("Started".to_string(), None, None)?;
        
        // In a real implementation, we would keep the call object in a list
        // and send events as they happen.
        Ok(())
    }
}
