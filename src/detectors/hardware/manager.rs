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

    pub fn list_all_devices(&mut self) -> Vec<Device> {
        if let Some((cache, ts)) = &self.cache {
            if ts.elapsed() < CACHE_DURATION {
                return cache.clone();
            }
        }

        let devices: Vec<Device> = self.detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| self.is_gaming_device(d))
            .collect();
        
        self.cache = Some((devices.clone(), Instant::now()));
        devices
    }

    pub fn list_rgb_devices(&mut self) -> Vec<Device> {
        if let Some((cache, ts)) = &self.rgb_cache {
            if ts.elapsed() < CACHE_DURATION {
                return cache.clone();
            }
        }

        let devices: Vec<Device> = self.detectors
            .iter()
            .flat_map(|d| d.list_devices())
            .filter(|d| self.is_rgb_device(d))
            .collect();
        
        self.rgb_cache = Some((devices.clone(), Instant::now()));
        devices
    }

    fn is_gaming_device(&self, dev: &Device) -> bool {
        // Exclude security keys (Yubico)
        if dev.vendor_id == "1050" { return false; }

        // Exclude obvious lighting controllers from main list
        let name = dev.name.to_lowercase();
        if name.contains("lighting") || name.contains("aura") || name.contains("fan") || name.contains("rgb") {
            return false;
        }

        // Must have uaccess for many gaming devices, or be a classic input
        let is_classic = dev.classes.contains(&"mouse".to_string()) || 
                         dev.classes.contains(&"keyboard".to_string()) || 
                         dev.classes.contains(&"controller".to_string()) ||
                         dev.classes.contains(&"headset".to_string()) ||
                         dev.classes.contains(&"wheel".to_string()) ||
                         dev.classes.contains(&"flight_stick".to_string());
        
        // Ensure audio devices actually have user-level permissions (filters out motherboard HDMI/PCI noise)
        if dev.classes.contains(&"audio".to_string()) && !dev.has_uaccess {
            return false;
        }

        is_classic
    }

    fn is_rgb_device(&self, dev: &Device) -> bool {
        let name = dev.name.to_lowercase();
        name.contains("rgb") || 
        name.contains("lighting") || 
        name.contains("led") || 
        name.contains("fan") ||
        name.contains("aura") ||
        name.contains("glow") ||
        name.contains("litra")
    }
}
impl Default for HardwareManager {
    fn default() -> Self {
        Self::new()
    }
}
