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
        let mut enumerator = Enumerator::new().unwrap();

        // Enumerate input devices
        enumerator.match_subsystem("input").unwrap();

        for device in enumerator.scan_devices().unwrap() {
            // We want the event nodes or similar that represent actual input capabilities
            if let Some(devname) = device.devnode() {
                let is_mouse = device.property_value("ID_INPUT_MOUSE").is_some();
                let is_kbd = device.property_value("ID_INPUT_KEYBOARD").is_some();
                let is_joy = device.property_value("ID_INPUT_JOYSTICK").is_some();

                if is_mouse || is_kbd || is_joy {
                    let mut classes = Vec::new();
                    if is_mouse { classes.push("mouse".to_string()); }
                    if is_kbd { classes.push("keyboard".to_string()); }
                    if is_joy { classes.push("controller".to_string()); }

                    let name = device.property_value("ID_MODEL")
                        .or_else(|| device.property_value("NAME"))
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown Device".to_string());

                    let vendor = device.property_value("ID_VENDOR")
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Generic".to_string());

                    let vendor_id = device.property_value("ID_VENDOR_ID")
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let product_id = device.property_value("ID_MODEL_ID")
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let bus_type = device.property_value("ID_BUS")
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Check for uaccess tag in the TAGS property (usually colon or space separated)
                    let has_uaccess = device.property_value("TAGS")
                        .map(|s| s.to_string_lossy().contains("uaccess"))
                        .unwrap_or(false);

                    devices.push(Device {
                        name,
                        vendor,
                        vendor_id,
                        product_id,
                        bus_type,
                        path: devname.to_string_lossy().to_string(),
                        classes,
                        has_uaccess,
                    });
                }
            }
        }

        // Deduplicate
        // Gaming devices often have multiple event nodes
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
                if current_key != last_key {
                    filtered.push(dev);
                    last_key = current_key;
                } else {
                    if let Some(existing) = filtered.last_mut() {
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
        }

        filtered
    }
}
