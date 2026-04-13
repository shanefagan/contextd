use std::sync::{Arc, RwLock};
use crate::detectors::games::manager::GameManager;
use crate::detectors::hardware::manager::HardwareManager;
use crate::game_linkd::*;

pub struct GameLinkdService {
    pub game_manager: Arc<RwLock<GameManager>>,
    pub hardware_manager: Arc<RwLock<HardwareManager>>,
}

impl VarlinkInterface for GameLinkdService {
    fn get_active_game(&self, call: &mut dyn Call_GetActiveGame) -> varlink::Result<()> {
        let manager = self.game_manager.read().unwrap();
        let game = manager.get_active_game().map(|g| Game {
            name: g.name,
            id: g.id,
            source: g.source,
            pid: g.pid.map(|p| p as i64),
        });
        call.reply(game)
    }

    fn list_installed_games(&self, call: &mut dyn Call_ListInstalledGames) -> varlink::Result<()> {
        let manager = self.game_manager.read().unwrap();
        let games = manager.list_all_installed().into_iter().map(|g| Game {
            name: g.name,
            id: g.id,
            source: g.source,
            pid: g.pid.map(|p| p as i64),
        }).collect();
        call.reply(games)
    }

    fn list_devices(&self, call: &mut dyn Call_ListDevices) -> varlink::Result<()> {
        let manager = self.hardware_manager.read().unwrap();
        let devices = manager.list_all_devices().into_iter().map(|d| Device {
            name: d.name,
            vendor_id: d.vendor_id,
            product_id: d.product_id,
            bus_type: d.bus_type,
            path: d.path,
            classes: d.classes,
        }).collect();
        call.reply(devices)
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
