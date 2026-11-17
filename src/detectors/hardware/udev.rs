//! udev-based hardware inventory.
//!
//! Input, sound and hidraw nodes are grouped by the physical device they
//! belong to: the USB device for USB hardware, the HID device for Bluetooth
//! and I2C HID, or the node itself otherwise. That keeps a keyboard's
//! several event and hidraw nodes together while two identical controllers
//! stay separate.

use super::{Device, HardwareDetector};
use std::path::PathBuf;
use udev::Enumerator;

pub struct UdevDetector;

impl UdevDetector {
    pub fn new() -> Self {
        Self
    }
}

/// The node of one device that we want to report, before grouping.
struct Node {
    group: PathBuf,
    device: Device,
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

/// The syspath that identifies the physical device `device` belongs to.
fn physical_group(device: &udev::Device) -> PathBuf {
    let mut hid_ancestor = None;
    let mut current = device.parent();
    while let Some(p) = current {
        let subsystem = p.subsystem().map(|s| s.to_string_lossy().into_owned());
        let devtype = p.devtype().map(|s| s.to_string_lossy().into_owned());
        match (subsystem.as_deref(), devtype.as_deref()) {
            (Some("usb"), Some("usb_device")) => return p.syspath().to_path_buf(),
            (Some("hid"), _) if hid_ancestor.is_none() => {
                hid_ancestor = Some(p.syspath().to_path_buf());
            }
            _ => {}
        }
        current = p.parent();
    }
    hid_ancestor.unwrap_or_else(|| device.syspath().to_path_buf())
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
        let mut nodes = Vec::new();

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
            nodes.push(Node {
                group: physical_group(&device),
                device: create_device(&device, devnode.to_string_lossy().into_owned(), classes),
            });
        }

        // 2. Sound cards (headsets, speakers, mics); not PCM/control nodes
        for device in scan("sound") {
            if !device.sysname().to_string_lossy().starts_with("card") {
                continue;
            }
            nodes.push(Node {
                group: physical_group(&device),
                device: create_device(
                    &device,
                    device.syspath().to_string_lossy().into_owned(),
                    vec!["audio".to_string()],
                ),
            });
        }

        // 3. hidraw nodes (lighting controllers, Stream Decks, mixers)
        for device in scan("hidraw") {
            let Some(devnode) = device.devnode() else {
                continue;
            };
            nodes.push(Node {
                group: physical_group(&device),
                device: create_device(
                    &device,
                    devnode.to_string_lossy().into_owned(),
                    vec!["hid".to_string()],
                ),
            });
        }

        group_nodes(nodes)
    }
}

/// Merges nodes that share a physical device, keeping the first node's
/// identity and path as the primary ones.
fn group_nodes(nodes: Vec<Node>) -> Vec<Device> {
    let mut order: Vec<PathBuf> = Vec::new();
    let mut merged: std::collections::HashMap<PathBuf, Device> = Default::default();

    for node in nodes {
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
            },
            Node {
                group: "/sys/usb/1-2".into(),
                device: dev("Pad", "/dev/input/event4", "controller", "045e"),
            },
            Node {
                group: "/sys/usb/1-1".into(),
                device: dev("Pad", "/dev/hidraw2", "hid", "045e"),
            },
        ];
        let devices = group_nodes(nodes);
        assert_eq!(devices.len(), 2, "two identical pads stay two devices");
        let first = devices
            .iter()
            .find(|d| d.path == "/dev/input/event3")
            .unwrap();
        assert_eq!(first.paths, vec!["/dev/input/event3", "/dev/hidraw2"]);
        assert_eq!(first.classes, vec!["controller", "hid"]);
    }

    #[test]
    fn enumerates_without_panicking() {
        // Works with or without udev data; must never panic.
        let _ = UdevDetector::new().list_devices();
    }
}
