use super::udev::UdevDetector;
use super::{Device, HardwareDetector};
use crate::config;
use crate::watch;
use std::time::{Duration, Instant};

struct Cached {
    devices: Vec<Device>,
    at: Instant,
    /// udev generation the scan saw; a newer one means it is stale.
    udev_generation: u64,
}

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
    device_cache: Option<Cached>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: vec![Box::new(UdevDetector::new())],
            device_cache: None,
        }
    }

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        self.list_all_devices_max_age(Duration::from_secs(config::get().ttls.hardware))
    }

    /// Devices from a scan at most `max_age` old, and newer than the last
    /// change the udev watcher saw.
    pub fn list_all_devices_max_age(&mut self, max_age: Duration) -> Vec<Device> {
        let generation = watch::UDEV.current();
        if let Some(c) = &self.device_cache
            && c.at.elapsed() < max_age
            && c.udev_generation == generation
        {
            return c.devices.clone();
        }

        let blacklist = &config::get().blacklist.devices;
        let devices: Vec<Device> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| !d.paths.iter().any(|p| blacklist.contains(p)))
            .collect();

        self.device_cache = Some(Cached {
            devices: devices.clone(),
            at: Instant::now(),
            udev_generation: generation,
        });
        devices
    }

    /// Devices known to have RGB lighting (class `rgb`): listed in
    /// OpenRGB's udev rules or tagged `ID_CONTEXTD_RGB=1`.
    pub fn list_rgb_devices(&mut self) -> Vec<Device> {
        self.list_all_devices()
            .into_iter()
            .filter(|d| d.classes.iter().any(|c| c == "rgb"))
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
    fn rgb_list_is_rgb_devices_only() {
        let mut mgr = manager(vec![
            dev("mouse", &["mouse", "hid"]),
            dev("kbd", &["keyboard", "hid", "rgb"]),
        ]);
        let rgb = mgr.list_rgb_devices();
        assert_eq!(rgb.len(), 1);
        assert_eq!(rgb[0].name, "kbd");
    }
}
