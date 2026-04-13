use super::{Device, HardwareDetector};
use udev::Enumerator;

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
        let mut devices = Vec::new();

        // 1. Enumerate input devices (Keyboards, Mice, Joysticks)
        let mut input_enum = Enumerator::new().unwrap();
        input_enum.match_subsystem("input").unwrap();
        for device in input_enum.scan_devices().unwrap() {
            if let Some(devname) = device.devnode() {
                let is_mouse = device.property_value("ID_INPUT_MOUSE").is_some();
                let is_kbd = device.property_value("ID_INPUT_KEYBOARD").is_some();
                let is_joy = device.property_value("ID_INPUT_JOYSTICK").is_some();

                if is_mouse || is_kbd || is_joy {
                    let mut classes = Vec::new();
                    if is_mouse { classes.push("mouse".to_string()); }
                    if is_kbd { classes.push("keyboard".to_string()); }
                    if is_joy { classes.push("controller".to_string()); }

                    devices.push(self.create_device(&device, devname.to_string_lossy().to_string(), classes));
                }
            }
        }

        // 2. Enumerate sound devices (Headsets, Speakers, Mics)
        let mut sound_enum = Enumerator::new().unwrap();
        sound_enum.match_subsystem("sound").unwrap();
        for device in sound_enum.scan_devices().unwrap() {
            // We only care about base cards, not individual PCM/control nodes for the list
            if device.sysname().to_string_lossy().starts_with("card") {
                let classes = vec!["audio".to_string()];
                devices.push(self.create_device(&device, device.syspath().to_string_lossy().to_string(), classes));
            }
        }

        // 3. Enumerate hidraw devices (Specialized controllers like Beacn, StreamDeck, Lighting)
        let mut hid_enum = Enumerator::new().unwrap();
        hid_enum.match_subsystem("hidraw").unwrap();
        for device in hid_enum.scan_devices().unwrap() {
            if let Some(devname) = device.devnode() {
                // We just want to know it's a HID device
                let classes = vec!["hid".to_string()];
                devices.push(self.create_device(&device, devname.to_string_lossy().to_string(), classes));
            }
        }

        // Deduplicate
        devices.sort_by(|a, b| {
            let key_a = format!("{}:{}", a.vendor_id, a.product_id);
            let key_b = format!("{}:{}", b.vendor_id, b.product_id);
            key_a.cmp(&key_b)
        });

        let mut filtered = Vec::new();
        if !devices.is_empty() {
            let mut last_key = String::new();
            for dev in devices {
                let current_key = format!("{}:{}", dev.vendor_id, dev.product_id);
                if current_key == "0000:0000" || current_key != last_key {
                    filtered.push(dev);
                    last_key = current_key;
                } else if let Some(existing) = filtered.last_mut() {
                    for class in dev.classes {
                        if !existing.classes.contains(&class) {
                            existing.classes.push(class);
                        }
                    }
                    if dev.has_uaccess {
                        existing.has_uaccess = true;
                    }
                }
            }
        }

        filtered
    }
}

impl UdevDetector {
    pub(crate) fn create_device(&self, device: &udev::Device, path: String, classes: Vec<String>) -> Device {
        let name = device.property_value("ID_MODEL")
            .or_else(|| device.property_value("NAME"))
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Device".to_string());

        let vendor = device.property_value("ID_VENDOR")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Generic".to_string());

        let vendor_id = device.property_value("ID_VENDOR_ID")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "0000".to_string());

        let product_id = device.property_value("ID_MODEL_ID")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "0000".to_string());

        let bus_type = device.property_value("ID_BUS")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut has_uaccess = device.property_value("TAGS")
            .map(|s| s.to_string_lossy().contains("uaccess"))
            .unwrap_or(false);
        
        // Also check parents (sometimes the tag is on the USB device node but not the child interface)
        if !has_uaccess {
            let mut parent = device.parent();
            while let Some(p) = parent {
                if p.property_value("TAGS").map(|s| s.to_string_lossy().contains("uaccess")).unwrap_or(false) {
                    has_uaccess = true;
                    break;
                }
                parent = p.parent();
            }
        }

        let mut classes = classes;
        let model_lower = name.to_lowercase();
        if model_lower.contains("wheel") {
            classes.push("wheel".to_string());
        }
        if model_lower.contains("stick") || model_lower.contains("hotas") || model_lower.contains("throttle") || model_lower.contains("yoke") {
            classes.push("flight_stick".to_string());
        }

        Device {
            name,
            vendor,
            vendor_id,
            product_id,
            bus_type,
            path,
            classes,
            has_uaccess,
        }
    }
}
