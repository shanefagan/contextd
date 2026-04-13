use super::{Device, HardwareDetector};
use std::time::{Duration, Instant};

const CACHE_DURATION: Duration = Duration::from_secs(10); // Hardware changes less often

pub struct HardwareManager {
    detectors: Vec<Box<dyn HardwareDetector>>,
    cache: Option<(Vec<Device>, Instant)>,
    rgb_cache: Option<(Vec<Device>, Instant)>,
}

impl HardwareManager {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            cache: None,
            rgb_cache: None,
        }
    }

    pub fn add_detector(&mut self, detector: Box<dyn HardwareDetector>) {
        self.detectors.push(detector);
        self.cache = None;
        self.rgb_cache = None;
    }

    #[allow(dead_code)]
    pub fn invalidate(&mut self) {
        log::debug!("Hardware cache invalidated due to hotplug event");
        self.cache = None;
        self.rgb_cache = None;
    }

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        if let Some((cache, ts)) = &self.cache && ts.elapsed() < CACHE_DURATION {
            return cache.clone();
        }

        let devices: Vec<Device> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| self.is_gaming_device(d))
            .collect();

        self.cache = Some((devices.clone(), Instant::now()));
        devices
    }

    pub fn list_rgb_devices(&mut self) -> Vec<Device> {
        if let Some((cache, ts)) = &self.rgb_cache && ts.elapsed() < CACHE_DURATION {
            return cache.clone();
        }

        let devices: Vec<Device> = self
            .detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| self.is_rgb_device(d))
            .collect();

        self.rgb_cache = Some((devices.clone(), Instant::now()));
        devices
    }

    fn is_gaming_device(&self, dev: &Device) -> bool {
        // Exclude security keys (Yubico)
        if dev.vendor_id == "1050" {
            return false;
        }

        // Exclude obvious lighting controllers from main list
        let name = dev.name.to_lowercase();
        if name.contains("lighting")
            || name.contains("aura")
            || name.contains("fan")
            || name.contains("rgb")
        {
            return false;
        }

        // Must have uaccess for many gaming devices, or be a classic input
        let is_classic = dev.classes.contains(&"mouse".to_string())
            || dev.classes.contains(&"keyboard".to_string())
            || dev.classes.contains(&"controller".to_string())
            || dev.classes.contains(&"audio".to_string())
            || dev.classes.contains(&"wheel".to_string())
            || dev.classes.contains(&"flight_stick".to_string());

        // Ensure audio devices actually have user-level permissions (filters out motherboard HDMI/PCI noise)
        if dev.classes.contains(&"audio".to_string()) && !dev.has_uaccess {
            return false;
        }

        is_classic
    }

    fn is_rgb_device(&self, dev: &Device) -> bool {
        let name = dev.name.to_lowercase();
        name.contains("rgb")
            || name.contains("lighting")
            || name.contains("led")
            || name.contains("fan")
            || name.contains("aura")
            || name.contains("glow")
            || name.contains("litra")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_device(name: &str, vendor_id: &str, classes: Vec<&str>, uaccess: bool) -> Device {
        Device {
            name: name.to_string(),
            vendor: "TestVendor".to_string(),
            vendor_id: vendor_id.to_string(),
            product_id: "0001".to_string(),
            bus_type: "usb".to_string(),
            path: "/dev/test".to_string(),
            classes: classes.into_iter().map(|s| s.to_string()).collect(),
            has_uaccess: uaccess,
        }
    }

    #[test]
    fn test_is_gaming_device() {
        let mgr = HardwareManager::new();

        // Standard Mouse
        let mouse = mock_device("Gaming Mouse", "1234", vec!["mouse"], true);
        assert!(mgr.is_gaming_device(&mouse));

        // YubiKey (Security Key) - Should be blocked
        let yubikey = mock_device("YubiKey", "1050", vec!["keyboard"], true);
        assert!(!mgr.is_gaming_device(&yubikey));

        // RGB Fan - Should be blocked from main list
        let fan = mock_device("LianLi Fan RGB", "9999", vec!["hid"], true);
        assert!(!mgr.is_gaming_device(&fan));

        // Audio Device (No UAccess) - Should be blocked (filtered noise)
        let audio_noise = mock_device("HDMI Audio", "1002", vec!["audio"], false);
        assert!(!mgr.is_gaming_device(&audio_noise));

        // BEACN Mic (Audio with UAccess)
        let beacn = mock_device("BEACN Mic", "33ae", vec!["audio"], true);
        assert!(mgr.is_gaming_device(&beacn));
    }

    #[test]
    fn test_is_rgb_device() {
        let mgr = HardwareManager::new();

        let rgb_strip = mock_device("Lighting Node PRO", "1b1c", vec!["hid"], true);
        assert!(mgr.is_rgb_device(&rgb_strip));

        let fan = mock_device("Cooler RGB Fan", "0000", vec!["hid"], true);
        assert!(mgr.is_rgb_device(&fan));

        let mouse = mock_device("Plain Mouse", "0000", vec!["mouse"], true);
        assert!(!mgr.is_rgb_device(&mouse));
    }
}
impl Default for HardwareManager {
    fn default() -> Self {
        Self::new()
    }
}
