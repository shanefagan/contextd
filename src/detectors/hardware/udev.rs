use super::{Device, HardwareDetector};

pub struct UdevDetector;

impl UdevDetector {
    pub fn new() -> Self {
        Self
    }
}

impl HardwareDetector for UdevDetector {
    fn name(&self) -> &str {
        "udev"
    }

    fn list_devices(&self) -> Vec<Device> {
        // TODO: Use udev crate to enumerate HID devices
        vec![]
    }
}
