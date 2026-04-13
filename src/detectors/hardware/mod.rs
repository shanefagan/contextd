pub use crate::contextd::Device;

pub trait HardwareDetector: Send + Sync {
    fn name(&self) -> &str;
    fn list_devices(&self) -> Vec<Device>;
}

pub mod manager;
pub mod udev;
