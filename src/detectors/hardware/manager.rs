use super::{Device, HardwareDetector};

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn add_detector(&mut self, detector: Box<dyn HardwareDetector>) {
        self.detectors.push(detector);
    }

    pub fn list_all_devices(&self) -> Vec<Device> {
        self.detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .collect()
    }
}
