use super::udev::UdevDetector;
use super::{Device, HardwareDetector};
use crate::config;
use std::time::{Duration, Instant};

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
    device_cache: Option<(Vec<Device>, Instant)>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: vec![Box::new(UdevDetector::new())],
            device_cache: None,
        }
    }

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        let ttl = Duration::from_secs(config::get().ttls.hardware);
        if let Some((cache, ts)) = &self.device_cache
            && ts.elapsed() < ttl
        {
            return cache.clone();
        }

        let blacklist = &config::get().blacklist.devices;
        let devices: Vec<Device> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| !d.paths.iter().any(|p| blacklist.contains(p)))
            .collect();

        self.device_cache = Some((devices.clone(), Instant::now()));
        devices
    }

    /// Devices with a raw HID node, which is how almost all RGB peripherals
    /// are driven. contextd cannot tell which of them actually have LEDs.
    pub fn list_rgb_devices(&mut self) -> Vec<Device> {
        self.list_all_devices()
            .into_iter()
            .filter(|d| d.classes.iter().any(|c| c == "hid"))
            .collect()
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

    fn dev(name: &str, classes: &[&str]) -> Device {
        Device {
            name: name.to_string(),
            vendor: "TestVendor".to_string(),
            vendor_id: "1234".to_string(),
            product_id: "5678".to_string(),
            bus_type: "usb".to_string(),
            path: format!("/dev/{name}"),
            paths: vec![format!("/dev/{name}")],
            classes: classes.iter().map(|c| c.to_string()).collect(),
            has_uaccess: true,
            controllers: Vec::new(),
        }
    }

    fn manager(devices: Vec<Device>) -> HardwareManager {
        HardwareManager {
            detectors: vec![Box::new(MockDetector { devices })],
            device_cache: None,
        }
    }

    #[test]
    fn test_hardware_manager_cache() {
        let mut mgr = manager(vec![dev("mouse", &["mouse"])]);
        assert_eq!(mgr.list_all_devices().len(), 1);
        mgr.detectors.clear();
        assert_eq!(mgr.list_all_devices().len(), 1, "served from cache");
    }

    #[test]
    fn rgb_list_is_hid_devices_only() {
        let mut mgr = manager(vec![
            dev("mouse", &["mouse"]),
            dev("kbd", &["keyboard", "hid"]),
        ]);
        let rgb = mgr.list_rgb_devices();
        assert_eq!(rgb.len(), 1);
        assert_eq!(rgb[0].name, "kbd");
    }
}
