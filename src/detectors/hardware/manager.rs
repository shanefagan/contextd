use super::udev::UdevDetector;
use super::{Device, HardwareDetector};
use crate::config::CONFIG;
use std::time::{Duration, Instant};

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
    device_cache: Option<(Vec<Device>, Instant)>,
    rgb_cache: Option<(Vec<Device>, Instant)>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: vec![Box::new(UdevDetector::new())],
            device_cache: None,
            rgb_cache: None,
        }
    }

    #[allow(dead_code)]
    pub fn add_detector(&mut self, detector: Box<dyn HardwareDetector>) {
        self.detectors.push(detector);
        self.invalidate_caches();
    }

    #[allow(dead_code)]
    pub fn invalidate_caches(&mut self) {
        self.device_cache = None;
        self.rgb_cache = None;
    }

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        let ttl = Duration::from_secs(CONFIG.ttls.hardware);
        if let Some((cache, ts)) = &self.device_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        let devices: Vec<Device> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| !CONFIG.blacklist.devices.contains(&d.path))
            .collect();

        self.device_cache = Some((devices.clone(), Instant::now()));
        devices
    }

    pub fn list_rgb_devices(&mut self) -> Vec<Device> {
        let ttl = Duration::from_secs(CONFIG.ttls.hardware);
        if let Some((cache, ts)) = &self.rgb_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        // For now, we consider all detected devices as candidates for RGB control
        // hints, though in the future we might filter for "hid" or "audio" classes.
        let devices = self.list_all_devices();

        self.rgb_cache = Some((devices.clone(), Instant::now()));
        devices
    }
}

impl Default for HardwareManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDetector {
        devices: Vec<Device>,
    }

    impl HardwareDetector for MockDetector {
        fn name(&self) -> &str {
            "Mock"
        }
        fn list_devices(&self) -> Vec<Device> {
            self.devices.clone()
        }
    }

    #[test]
    fn test_hardware_manager_cache() {
        let mut mgr = HardwareManager::new();
        mgr.detectors.clear(); // Clear default detectors for testing

        let dev = Device {
            name: "Test Mouse".to_string(),
            vendor: "TestVendor".to_string(),
            vendor_id: "1234".to_string(),
            product_id: "5678".to_string(),
            bus_type: "usb".to_string(),
            path: "/dev/input/event0".to_string(),
            classes: vec!["mouse".to_string()],
            has_uaccess: true,
            controllers: Vec::new(),
        };

        mgr.add_detector(Box::new(MockDetector {
            devices: vec![dev.clone()],
        }));

        let devices = mgr.list_all_devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Test Mouse");

        let devices2 = mgr.list_all_devices();
        assert_eq!(devices2.len(), 1);
    }
}
