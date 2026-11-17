//! Which devices have RGB lighting.
//!
//! contextd does not keep its own device database. It reuses OpenRGB's
//! udev rules, which list the USB vendor/product IDs of every device
//! OpenRGB supports (one `ATTRS{idVendor}=="..", ATTRS{idProduct}==".."`
//! rule per device), wherever they are installed. Packagers and users can
//! also mark a device with the udev property `ID_CONTEXTD_RGB=1`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// udev rules directories, in the order udev reads them.
pub const RULES_DIRS: &[&str] = &[
    "/etc/udev/rules.d",
    "/run/udev/rules.d",
    "/usr/local/lib/udev/rules.d",
    "/usr/lib/udev/rules.d",
    "/lib/udev/rules.d",
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RgbIds {
    exact: HashSet<(u16, u16)>,
    vendors: HashSet<u16>,
}

impl RgbIds {
    /// Loads IDs from every rules file whose name contains "openrgb".
    pub fn load(dirs: &[PathBuf]) -> Self {
        let mut ids = Self::default();
        for dir in dirs {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if is_openrgb_rules(&path)
                    && let Ok(content) = fs::read_to_string(&path)
                {
                    ids.add_rules(&content);
                }
            }
        }
        ids
    }

    pub fn len(&self) -> usize {
        self.exact.len() + self.vendors.len()
    }

    pub fn contains(&self, vendor: u16, product: u16) -> bool {
        self.vendors.contains(&vendor) || self.exact.contains(&(vendor, product))
    }

    /// Adds the IDs from one rules file. Lines with a vendor but no
    /// product match the whole vendor.
    pub fn add_rules(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some(vendor) = attr_hex(line, "idVendor") else {
                continue;
            };
            match attr_hex(line, "idProduct") {
                Some(product) => {
                    self.exact.insert((vendor, product));
                }
                None => {
                    self.vendors.insert(vendor);
                }
            }
        }
    }
}

fn is_openrgb_rules(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "rules")
        && path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().contains("openrgb"))
}

/// The 4-digit hex value in `ATTRS{key}=="xxxx"` or `ATTR{key}=="xxxx"`.
fn attr_hex(line: &str, key: &str) -> Option<u16> {
    let needle = format!("{{{key}}}==\"");
    let start = line.find(&needle)? + needle.len();
    let value = &line[start..];
    let end = value.find('"')?;
    u16::from_str_radix(&value[..end], 16).ok()
}

/// Parses a 4-digit hex ID such as udev's `ID_VENDOR_ID` or sysfs
/// `idVendor`.
pub fn parse_hex_id(s: &str) -> Option<u16> {
    u16::from_str_radix(s.trim(), 16).ok()
}

/// Vendor and product from a HID_ID property (`BUS:VVVVVVVV:PPPPPPPP`),
/// which Bluetooth and I2C HID devices have instead of USB IDs.
pub fn parse_hid_id(s: &str) -> Option<(u16, u16)> {
    let mut parts = s.split(':').skip(1);
    let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
    let product = u32::from_str_radix(parts.next()?, 16).ok()?;
    Some((vendor as u16, product as u16))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = r#"
#---------------------------------------------------------------#
#  OpenRGB udev rules                                            #
#---------------------------------------------------------------#
SUBSYSTEMS=="usb|hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b2d", TAG+="uaccess", TAG+="Corsair_K95_RGB_Platinum"
# Older releases used ATTR and SUBSYSTEM
SUBSYSTEM=="usb", ATTR{idVendor}=="2516", ATTR{idProduct}=="0051", GROUP="plugdev", MODE="0660", TAG+="uaccess"
SUBSYSTEMS=="usb", ATTRS{idVendor}=="0b05", TAG+="uaccess"
# SUBSYSTEMS=="usb", ATTRS{idVendor}=="dead", ATTRS{idProduct}=="beef"
KERNEL=="i2c-[0-99]*", TAG+="uaccess"
"#;

    #[test]
    fn parses_openrgb_rules() {
        let mut ids = RgbIds::default();
        ids.add_rules(RULES);
        assert!(ids.contains(0x1b1c, 0x1b2d));
        assert!(ids.contains(0x2516, 0x0051));
        assert!(ids.contains(0x0b05, 0x1234), "vendor-wide rule");
        assert!(!ids.contains(0xdead, 0xbeef), "comments are ignored");
        assert!(!ids.contains(0x1b1c, 0x0001));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn loads_only_openrgb_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("60-openrgb.rules"), RULES).unwrap();
        fs::write(
            dir.path().join("70-other.rules"),
            "ATTRS{idVendor}==\"aaaa\", ATTRS{idProduct}==\"bbbb\"",
        )
        .unwrap();
        let ids = RgbIds::load(&[dir.path().to_path_buf(), "/nonexistent".into()]);
        assert!(ids.contains(0x1b1c, 0x1b2d));
        assert!(!ids.contains(0xaaaa, 0xbbbb));
    }

    #[test]
    fn parses_hid_ids() {
        assert_eq!(
            parse_hid_id("0005:0000046D:0000B023"),
            Some((0x046d, 0xb023))
        );
        assert_eq!(parse_hid_id("garbage"), None);
        assert_eq!(parse_hex_id("1b1c"), Some(0x1b1c));
    }
}
