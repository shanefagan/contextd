pub use crate::game_linkd::Device;

pub trait HardwareDetector: Send + Sync {
    fn name(&self) -> &str;
    fn list_devices(&self) -> Vec<Device>;
}

pub mod udev;
pub mod manager;
