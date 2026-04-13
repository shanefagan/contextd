use super::{Device, HardwareDetector};
use std::time::{Duration, Instant};

const CACHE_DURATION: Duration = Duration::from_secs(10); // Hardware changes less often

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
    cache: Option<(Vec<Device>, Instant)>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            cache: None,
        }
    }

    pub fn add_detector(&mut self, detector: Box<dyn HardwareDetector>) {
        self.detectors.push(detector);
        self.cache = None;
    }

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        if let Some((cache, ts)) = &self.cache {
            if ts.elapsed() < CACHE_DURATION {
                return cache.clone();
            }
        }

        let devices: Vec<Device> = self.detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .collect();
        
        self.cache = Some((devices.clone(), Instant::now()));
        devices
    }
}
impl Default for HardwareManager {
    fn default() -> Self {
        Self::new()
    }
}
