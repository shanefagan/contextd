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
        let ram_used = ((sys.total_memory() - sys.available_memory()) / 1024 / 1024) as i64;

        let cpu_count = sys.cpus().len() as i64;
        let cpu_model = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let os_release = format!(
            "{} {}",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or_default()
        );

        // Heuristic for Vulkan/OpenGL
        let vulkan_supported = Path::new("/usr/lib/libvulkan.so.1").exists()
            || Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so.1").exists()
            || Path::new("/usr/lib64/libvulkan.so.1").exists();

        let opengl_supported = Path::new("/usr/lib/libGL.so.1").exists()
            || Path::new("/usr/lib/x86_64-linux-gnu/libGL.so.1").exists()
            || Path::new("/usr/lib64/libGL.so.1").exists();

        let gpus = Self::detect_gpus();

        Diagnostics {
            vulkan_supported,
            opengl_supported,
            ram_total,
            ram_used,
            cpu_model,
            cpu_count,
            gpus,
            kernel_version,
            os_release,
            last_updated: 0,
        }
    }

    fn detect_gpus() -> Vec<Gpu> {
        let mut gpus = Vec::new();

        if let Ok(mut enumerator) = udev::Enumerator::new() {
            enumerator.match_subsystem("drm").unwrap();

            for device in enumerator.scan_devices().unwrap() {
                let sysname = device.sysname().to_string_lossy();
                if sysname.starts_with("card") && !sysname.contains('-') {
                    // Get vendor and model names from udev database
                    let vendor = device
                        .property_value("ID_VENDOR_FROM_DATABASE")
                        .or_else(|| device.property_value("ID_VENDOR"))
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());

                    let model = device
                        .property_value("ID_MODEL_FROM_DATABASE")
                        .or_else(|| device.property_value("ID_MODEL"))
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("GPU ({})", sysname));

                    let driver = device
                        .driver()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    // Try to get VRAM from sysfs (still somewhat vendor-specific paths)
                    let device_path = device.syspath().join("device");
                    let vram_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|b| b / 1024 / 1024)
                        .unwrap_or(0);

                    let vram_used = fs::read_to_string(device_path.join("mem_info_vram_used"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|b| b / 1024 / 1024);

                    gpus.push(Gpu {
                        name: model,
                        vendor,
                        driver,
                        vram_total,
                        vram_used,
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
                vram_used: None,
            });
        }

        gpus
    }
}
