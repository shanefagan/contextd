use crate::contextd::{Diagnostics, Gpu};
use std::fs;
use std::path::{Path, PathBuf};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

pub struct SystemDiagnostics;

/// Directories the dynamic loader commonly searches, including the
/// Debian-style multiarch directory for the architecture we run on.
fn library_dirs() -> Vec<PathBuf> {
    let triplet = match std::env::consts::ARCH {
        "x86_64" => "x86_64-linux-gnu",
        "aarch64" => "aarch64-linux-gnu",
        "x86" => "i386-linux-gnu",
        "arm" => "arm-linux-gnueabihf",
        "riscv64" => "riscv64-linux-gnu",
        "powerpc64" => "powerpc64le-linux-gnu",
        other => other,
    };
    ["/usr/lib64", "/usr/lib", "/lib64", "/lib"]
        .iter()
        .map(PathBuf::from)
        .chain([
            PathBuf::from("/usr/lib").join(triplet),
            PathBuf::from("/lib").join(triplet),
        ])
        .collect()
}

fn has_library(name: &str) -> bool {
    library_dirs().iter().any(|d| d.join(name).exists())
}

/// Parses "1.3.280" into comparable numbers.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|p| p.trim().parse().unwrap_or(0))
        .collect()
}

/// Picks the highest Vulkan API version from ICD manifests.
fn highest_api_version<'a>(manifests: impl IntoIterator<Item = &'a str>) -> Option<String> {
    manifests
        .into_iter()
        .filter_map(|content| {
            let v: serde_json::Value = serde_json::from_str(content).ok()?;
            Some(v.get("ICD")?.get("api_version")?.as_str()?.to_string())
        })
        .max_by_key(|v| version_key(v))
}

impl SystemDiagnostics {
    pub fn get_diagnostics() -> Diagnostics {
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::nothing().with_ram())
                .with_cpu(CpuRefreshKind::nothing()),
        );

        let ram_total = (sys.total_memory() / 1024 / 1024) as i64;
        let cpu_count = sys.cpus().len() as i64;
        let cpu_model = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());

        let vulkan_supported = has_library("libvulkan.so.1");
        let vulkan_version = if vulkan_supported {
            Self::detect_vulkan_version()
        } else {
            None
        };
        let opengl_supported = has_library("libGL.so.1") || has_library("libOpenGL.so.0");

        Diagnostics {
            vulkan_supported,
            vulkan_version,
            opengl_supported,
            ram_total,
            cpu_model,
            cpu_count,
            gpus: Self::detect_gpus(),
            kernel_version,
            last_updated: 0,
        }
    }

    fn detect_vulkan_version() -> Option<String> {
        let contents: Vec<String> = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"]
            .iter()
            .filter_map(|d| fs::read_dir(d).ok())
            .flatten()
            .flatten()
            .filter_map(|e| fs::read_to_string(e.path()).ok())
            .collect();
        highest_api_version(contents.iter().map(String::as_str))
    }

    fn detect_gpus() -> Vec<Gpu> {
        let mut gpus = Vec::new();

        let devices = udev::Enumerator::new().and_then(|mut e| {
            e.match_subsystem("drm")?;
            e.scan_devices().map(|it| it.collect::<Vec<_>>())
        });
        let devices = devices.unwrap_or_else(|e| {
            log::warn!("udev enumeration of drm devices failed: {}", e);
            Vec::new()
        });

        for device in devices {
            let sysname = device.sysname().to_string_lossy();
            if !sysname.starts_with("card") || sysname.contains('-') {
                continue;
            }
            let pci_device = device.parent();
            let read = |name: &str| {
                fs::read_to_string(device.syspath().join("device").join(name))
                    .map(|s| s.trim().to_lowercase())
                    .unwrap_or_default()
            };
            let vendor_id = read("vendor");
            let pci_prop = |key: &str| {
                pci_device
                    .as_ref()
                    .and_then(|p| p.property_value(key))
                    .map(|s| s.to_string_lossy().into_owned())
            };

            let vendor = match vendor_id.as_str() {
                "0x1002" => "AMD".to_string(),
                "0x10de" => "NVIDIA".to_string(),
                "0x8086" => "Intel".to_string(),
                _ => pci_prop("ID_VENDOR_FROM_DATABASE")
                    .or_else(|| pci_prop("ID_VENDOR"))
                    .unwrap_or_else(|| vendor_id.clone()),
            };

            let name = pci_prop("ID_MODEL_FROM_DATABASE")
                .or_else(|| pci_prop("ID_MODEL"))
                .unwrap_or_else(|| format!("GPU ({}:{})", vendor_id, read("device")));

            let driver = device
                .driver()
                .or_else(|| pci_device.as_ref().and_then(|p| p.driver()))
                .map(|s| s.to_string_lossy().into_owned())
                .or_else(|| {
                    fs::read_link(device.syspath().join("device/driver"))
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                })
                .unwrap_or_else(|| "unknown".to_string());

            // Only amdgpu exposes this; 0 means unknown.
            let vram_total = read("mem_info_vram_total")
                .parse::<i64>()
                .map(|b| b / 1024 / 1024)
                .unwrap_or(0);

            gpus.push(Gpu {
                name,
                vendor,
                driver,
                vram_total,
            });
        }

        // No DRM node: maybe NVIDIA's proprietary driver without modeset.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_versions_compare_numerically() {
        let a = r#"{"ICD": {"api_version": "1.3.99"}}"#;
        let b = r#"{"ICD": {"api_version": "1.3.280"}}"#;
        let c = r#"not json"#;
        assert_eq!(highest_api_version([a, b, c]).as_deref(), Some("1.3.280"));
    }
}
