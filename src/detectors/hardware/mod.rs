use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub vendor_id: String,
    pub product_id: String,
    pub bus_type: String,
    pub path: String,
    pub classes: Vec<String>,
}

pub trait HardwareDetector: Send + Sync {
    fn name(&self) -> &str;
    fn list_devices(&self) -> Vec<Device>;
}

pub mod udev;
pub mod manager;
