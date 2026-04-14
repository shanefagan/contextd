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

        let is_flatpak = Path::new("/.flatpak-info").exists();
        let is_snap = std::env::var("SNAP").is_ok();

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
            is_flatpak,
            is_snap,
            last_updated: 0,
        }
    }

    fn detect_gpus() -> Vec<Gpu> {
        let mut gpus = Vec::new();

        // Basic GPU detection via DRM sysfs
        let drm_path = "/sys/class/drm";
        if let Ok(entries) = fs::read_dir(drm_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("card") && !name.contains('-') {
                    // This is a base card node
                    let device_path = entry.path().join("device");

                    let vendor = fs::read_to_string(device_path.join("vendor"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "Unknown".to_string());

                    let device_id = fs::read_to_string(device_path.join("device"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "Unknown".to_string());

                    // Try to get VRAM (AMD style)
                    let vram_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|b| b / 1024 / 1024)
                        .unwrap_or(0);

                    let vram_used = fs::read_to_string(device_path.join("mem_info_vram_used"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|b| b / 1024 / 1024);

                    // Map vendor IDs to names
                    let vendor_name = match vendor.as_str() {
                        "0x1002" => "AMD",
                        "0x10de" => "NVIDIA",
                        "0x8086" => "Intel",
                        _ => &vendor,
                    }
                    .to_string();

                    let driver = fs::read_link(device_path.join("driver"))
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_else(|| "unknown".to_string());

                    gpus.push(Gpu {
                        name: format!("GPU ({})", device_id),
                        vendor: vendor_name,
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
                vram_total: 0, // Harder to get without nvml
                vram_used: None,
            });
        }

        gpus
    }
}
