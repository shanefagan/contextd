//! udev-based hardware inventory.
//!
//! Input, sound and hidraw nodes are grouped by the physical device they
//! belong to: the USB device for USB hardware, the HID device for Bluetooth
//! and I2C HID, or the node itself otherwise. That keeps a keyboard's
//! several event and hidraw nodes together while two identical controllers
//! stay separate.
//!
//! Devices whose USB or HID vendor/product ID appears in OpenRGB's udev
//! rules, or that carry the udev property `ID_CONTEXTD_RGB=1`, get the
//! class `rgb`.

use super::rgb_ids::{self, RgbIds};
use super::{Device, HardwareDetector};
use std::path::PathBuf;
use udev::Enumerator;

pub struct UdevDetector {
    rules_dirs: Vec<PathBuf>,
}

impl UdevDetector {
    pub fn new() -> Self {
        Self {
            rules_dirs: rgb_ids::RULES_DIRS.iter().map(PathBuf::from).collect(),
        }
    }
}

/// The node of one device that we want to report, before grouping.
struct Node {
    group: PathBuf,
    device: Device,
    rgb: bool,
}

/// What the parent chain says about the physical device.
struct Physical {
    group: PathBuf,
    /// USB idVendor/idProduct, or the IDs from HID_ID
    ids: Option<(u16, u16)>,
    /// `ID_CONTEXTD_RGB=1` on the node or any ancestor
    rgb_tagged: bool,
}

fn prop(device: &udev::Device, key: &str) -> Option<String> {
    device
        .property_value(key)
        .map(|s| s.to_string_lossy().into_owned())
}

fn has_uaccess_tag(device: &udev::Device) -> bool {
    // CURRENT_TAGS reflects the rules as they are now; TAGS is sticky and
    // keeps tags that were removed by a later rule change.
    let tags = prop(device, "CURRENT_TAGS").or_else(|| prop(device, "TAGS"));
    tags.is_some_and(|t| t.split(':').any(|tag| tag == "uaccess"))
}

fn rgb_tagged(device: &udev::Device) -> bool {
    prop(device, "ID_CONTEXTD_RGB").is_some_and(|v| v == "1")
}

/// Walks up from a node to the physical device it belongs to: the USB
/// device if there is one, otherwise the outermost HID device (Bluetooth,
/// I2C), otherwise the node itself.
fn physical(device: &udev::Device) -> Physical {
    let mut hid: Option<(PathBuf, Option<(u16, u16)>)> = None;
    let mut tagged = rgb_tagged(device);
    let mut current = device.parent();
    while let Some(p) = current {
        tagged |= rgb_tagged(&p);
        let subsystem = p.subsystem().map(|s| s.to_string_lossy().into_owned());
        let devtype = p.devtype().map(|s| s.to_string_lossy().into_owned());
        match (subsystem.as_deref(), devtype.as_deref()) {
            (Some("usb"), Some("usb_device")) => {
                let attr = |name: &str| {
                    p.attribute_value(name)
                        .and_then(|v| rgb_ids::parse_hex_id(&v.to_string_lossy()))
                };
                return Physical {
                    group: p.syspath().to_path_buf(),
                    ids: attr("idVendor").zip(attr("idProduct")),
                    rgb_tagged: tagged,
                };
            }
            (Some("hid"), _) => {
                let ids = prop(&p, "HID_ID").and_then(|h| rgb_ids::parse_hid_id(&h));
                hid = Some((p.syspath().to_path_buf(), ids));
            }
            _ => {}
        }
        current = p.parent();
    }
    let (group, ids) = hid.unwrap_or_else(|| (device.syspath().to_path_buf(), None));
    Physical {
        group,
        ids,
        rgb_tagged: tagged,
    }
}

fn scan(subsystem: &str) -> Vec<udev::Device> {
    let devices = Enumerator::new().and_then(|mut e| {
        e.match_subsystem(subsystem)?;
        e.scan_devices().map(|it| it.collect::<Vec<_>>())
    });
    match devices {
        Ok(d) => d,
        Err(e) => {
            log::warn!("udev enumeration of {} failed: {}", subsystem, e);
            Vec::new()
        }
    }
}

impl HardwareDetector for UdevDetector {
    fn name(&self) -> &str {
        "udev"
    }

    fn list_devices(&self) -> Vec<Device> {
        let rgb = RgbIds::load(&self.rules_dirs);
        log::debug!(
            "Loaded {} RGB device IDs from OpenRGB udev rules",
            rgb.len()
        );
        let mut nodes = Vec::new();
        let mut add = |device: &udev::Device, path: String, classes: Vec<String>| {
            let physical = physical(device);
            let mut d = create_device(device, path, classes);
            if let Some((vendor, product)) = physical.ids
                && d.vendor_id == "0000"
            {
                d.vendor_id = format!("{vendor:04x}");
                d.product_id = format!("{product:04x}");
            }
            let is_rgb =
                physical.rgb_tagged || physical.ids.is_some_and(|(v, p)| rgb.contains(v, p));
            nodes.push(Node {
                group: physical.group,
                device: d,
                rgb: is_rgb,
            });
        };

        // 1. Input devices (keyboards, mice, joysticks)
        for device in scan("input") {
            let Some(devnode) = device.devnode() else {
                continue;
            };
            let mut classes = Vec::new();
            if device.property_value("ID_INPUT_MOUSE").is_some() {
                classes.push("mouse".to_string());
            }
            if device.property_value("ID_INPUT_KEYBOARD").is_some() {
                classes.push("keyboard".to_string());
            }
            if device.property_value("ID_INPUT_JOYSTICK").is_some() {
                classes.push("controller".to_string());
            }
            if classes.is_empty() {
                continue;
            }
            add(&device, devnode.to_string_lossy().into_owned(), classes);
        }

        // 2. Sound cards (headsets, speakers, mics); not PCM/control nodes
        for device in scan("sound") {
            if !device.sysname().to_string_lossy().starts_with("card") {
                continue;
            }
            add(
                &device,
                device.syspath().to_string_lossy().into_owned(),
                vec!["audio".to_string()],
            );
        }

        // 3. hidraw nodes (lighting controllers, Stream Decks, mixers)
        for device in scan("hidraw") {
            let Some(devnode) = device.devnode() else {
                continue;
            };
            add(
                &device,
                devnode.to_string_lossy().into_owned(),
                vec!["hid".to_string()],
            );
        }

        group_nodes(nodes)
    }
}

/// Merges nodes that share a physical device, keeping the first node's
/// identity and path as the primary ones.
fn group_nodes(nodes: Vec<Node>) -> Vec<Device> {
    let mut order: Vec<PathBuf> = Vec::new();
    let mut merged: std::collections::HashMap<PathBuf, Device> = Default::default();

    for mut node in nodes {
        if node.rgb && !node.device.classes.iter().any(|c| c == "rgb") {
            node.device.classes.push("rgb".to_string());
        }
        match merged.get_mut(&node.group) {
            Some(existing) => {
                for class in node.device.classes {
                    if !existing.classes.contains(&class) {
                        existing.classes.push(class);
                    }
                }
                for path in node.device.paths {
                    if !existing.paths.contains(&path) {
                        existing.paths.push(path);
                    }
                }
                existing.has_uaccess |= node.device.has_uaccess;
                if existing.vendor_id == "0000" && node.device.vendor_id != "0000" {
                    existing.vendor_id = node.device.vendor_id;
                    existing.product_id = node.device.product_id;
                }
            }
            None => {
                order.push(node.group.clone());
                merged.insert(node.group, node.device);
            }
        }
    }

    let mut devices: Vec<Device> = order
        .into_iter()
        .filter_map(|g| merged.remove(&g))
        .collect();
    devices.sort_by(|a, b| (&a.name, &a.path).cmp(&(&b.name, &b.path)));
    devices
}

fn create_device(device: &udev::Device, path: String, classes: Vec<String>) -> Device {
    let name = prop(device, "ID_MODEL_FROM_DATABASE")
        .or_else(|| prop(device, "ID_MODEL"))
        .or_else(|| prop(device, "NAME").map(|n| n.trim_matches('"').to_string()))
        .or_else(|| {
            device
                .parent()
                .and_then(|p| prop(&p, "NAME").map(|n| n.trim_matches('"').to_string()))
        })
        .unwrap_or_else(|| "Unknown Device".to_string());

    let vendor = prop(device, "ID_VENDOR_FROM_DATABASE")
        .or_else(|| prop(device, "ID_VENDOR"))
        .unwrap_or_else(|| "Generic".to_string());
    let vendor_id = prop(device, "ID_VENDOR_ID").unwrap_or_else(|| "0000".to_string());
    let product_id = prop(device, "ID_MODEL_ID").unwrap_or_else(|| "0000".to_string());
    let bus_type = prop(device, "ID_BUS").unwrap_or_default();

    // The tag is sometimes on the USB device rather than the child interface.
    let mut has_uaccess = has_uaccess_tag(device);
    let mut parent = device.parent();
    while !has_uaccess && let Some(p) = parent {
        has_uaccess = has_uaccess_tag(&p);
        parent = p.parent();
    }

    let mut classes = classes;
    let model_lower = name.to_lowercase();
    if model_lower.contains("wheel") {
        classes.push("wheel".to_string());
    }
    if ["stick", "hotas", "throttle", "yoke"]
        .iter()
        .any(|w| model_lower.contains(w))
    {
        classes.push("flight_stick".to_string());
    }

    Device {
        name,
        vendor,
        vendor_id,
        product_id,
        bus_type,
        paths: vec![path.clone()],
        path,
        classes,
        has_uaccess,
        controllers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, path: &str, class: &str, vid: &str) -> Device {
        Device {
            name: name.into(),
            vendor: "V".into(),
            vendor_id: vid.into(),
            product_id: "0001".into(),
            bus_type: "usb".into(),
            path: path.into(),
            paths: vec![path.into()],
            classes: vec![class.into()],
            has_uaccess: false,
            controllers: vec![],
        }
    }

    #[test]
    fn identical_devices_stay_separate_and_nodes_merge() {
        let nodes = vec![
            Node {
                group: "/sys/usb/1-1".into(),
                device: dev("Pad", "/dev/input/event3", "controller", "045e"),
                rgb: false,
            },
            Node {
                group: "/sys/usb/1-2".into(),
                device: dev("Pad", "/dev/input/event4", "controller", "045e"),
                rgb: false,
            },
            Node {
                group: "/sys/usb/1-1".into(),
                device: dev("Pad", "/dev/hidraw2", "hid", "045e"),
                rgb: true,
            },
        ];
        let devices = group_nodes(nodes);
        assert_eq!(devices.len(), 2, "two identical pads stay two devices");
        let first = devices
            .iter()
            .find(|d| d.path == "/dev/input/event3")
            .unwrap();
        assert_eq!(first.paths, vec!["/dev/input/event3", "/dev/hidraw2"]);
        assert_eq!(first.classes, vec!["controller", "hid", "rgb"]);
        let second = devices
            .iter()
            .find(|d| d.path == "/dev/input/event4")
            .unwrap();
        assert!(!second.classes.contains(&"rgb".to_string()));
    }

    #[test]
    fn enumerates_without_panicking() {
        // Works with or without udev data; must never panic.
        let _ = UdevDetector::new().list_devices();
    }
}
