use crate::contextd::{Diagnostics, Gpu};
use std::fs;
use std::path::Path;
use sysinfo::System;

pub struct SystemDiagnostics;

impl SystemDiagnostics {
    pub fn get_diagnostics() -> Diagnostics {
        let mut sys = System::new_all();
        sys.refresh_all();

        let ram_total = (sys.total_memory() / 1024 / 1024) as i64;

        let cpu_count = sys.cpus().len() as i64;
        let cpu_model = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());

        // Heuristic for Vulkan/OpenGL
        let vulkan_supported = Path::new("/usr/lib/libvulkan.so.1").exists()
            || Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so.1").exists()
            || Path::new("/usr/lib64/libvulkan.so.1").exists();

        let vulkan_version = if vulkan_supported {
            Self::detect_vulkan_version()
        } else {
            None
        };

        let opengl_supported = Path::new("/usr/lib/libGL.so.1").exists()
            || Path::new("/usr/lib/x86_64-linux-gnu/libGL.so.1").exists()
            || Path::new("/usr/lib64/libGL.so.1").exists();

        let gpus = Self::detect_gpus();

        Diagnostics {
            vulkan_supported,
            vulkan_version,
            opengl_supported,
            ram_total,
            cpu_model,
            cpu_count,
            gpus,
            kernel_version,
            last_updated: 0,
        }
    }

    fn detect_vulkan_version() -> Option<String> {
        let icd_path = "/usr/share/vulkan/icd.d";
        let mut versions = Vec::new();
        if let Ok(entries) = fs::read_dir(icd_path) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path())
                    && let Some(pos) = content.find("\"api_version\"")
                {
                    let sub = &content[pos..];
                    if let Some(start) = sub.find(':') {
                        let val = &sub[start + 1..];
                        if let Some(s) = val.find('"') {
                            let end_val = &val[s + 1..];
                            if let Some(e) = end_val.find('"') {
                                versions.push(end_val[..e].to_string());
                            }
                        }
                    }
                }
            }
        }
        // Very basic sort, works for most Vulkan versions (1.x.y)
        versions.sort();
        versions.last().cloned()
    }

    fn detect_gpus() -> Vec<Gpu> {
        let mut gpus = Vec::new();

        if let Ok(mut enumerator) = udev::Enumerator::new() {
            enumerator.match_subsystem("drm").unwrap();

            for device in enumerator.scan_devices().unwrap() {
                let sysname = device.sysname().to_string_lossy();
                if sysname.starts_with("card") && !sysname.contains('-') {
                    // Try to get properties from the device itself or its parent (PCI device)
                    let pci_device = device.parent();

                    // Extract Vendor mapping for common vendors
                    let vendor_id = fs::read_to_string(device.syspath().join("device/vendor"))
                        .map(|s| s.trim().to_lowercase())
                        .unwrap_or_default();

                    let vendor = match vendor_id.as_str() {
                        "0x1002" => "AMD".to_string(),
                        "0x10de" => "NVIDIA".to_string(),
                        "0x8086" => "Intel".to_string(),
                        _ => {
                            let mut v = None;
                            if let Some(ref p) = pci_device {
                                v = p
                                    .property_value("ID_VENDOR_FROM_DATABASE")
                                    .or_else(|| p.property_value("ID_VENDOR"))
                                    .map(|s| s.to_string_lossy().to_string());
                            }
                            v.unwrap_or_else(|| vendor_id.clone())
                        }
                    };

                    let model = {
                        let mut m = None;
                        if let Some(ref p) = pci_device {
                            m = p
                                .property_value("ID_MODEL_FROM_DATABASE")
                                .or_else(|| p.property_value("ID_MODEL"))
                                .map(|s| s.to_string_lossy().to_string());
                        }
                        m.unwrap_or_else(|| {
                            let dev_id = fs::read_to_string(device.syspath().join("device/device"))
                                .map(|s| s.trim().to_string())
                                .unwrap_or_default();
                            format!("GPU ({}:{})", vendor_id, dev_id)
                        })
                    };

                    let driver = device
                        .driver()
                        .or_else(|| pci_device.as_ref().and_then(|p| p.driver()))
                        .map(|s| s.to_string_lossy().to_string())
                        .or_else(|| {
                            fs::read_link(device.syspath().join("device/driver"))
                                .ok()
                                .and_then(|p| {
                                    p.file_name().map(|n| n.to_string_lossy().to_string())
                                })
                        })
                        .unwrap_or_else(|| "unknown".to_string());

                    // Try to get VRAM from sysfs
                    let device_path = device.syspath().join("device");
                    let vram_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|b| b / 1024 / 1024)
                        .unwrap_or(0);

                    gpus.push(Gpu {
                        name: model,
                        vendor,
                        driver,
                        vram_total,
                    });
                }
            }
        }

        // If no GPUs found via DRM, maybe it's NVIDIA with proprietary driver
        if gpus.is_empty() && Path::new("/proc/driver/nvidia").exists() {
            gpus.push(Gpu {
                name: "NVIDIA GPU".to_string(),
                vendor: "NVIDIA".to_string(),
                driver: "nvidia".to_string(),
                vram_total: 0,
            });
        }

        gpus
    }
}
