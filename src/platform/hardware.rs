//! Hardware profiling — CPU, RAM, GPU detection.

use crate::platform::simd::SimdCapability;
use serde::{Deserialize, Serialize};
use sysinfo::System;

/// GPU information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub vram_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    Apple,
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

/// Complete hardware profile of the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_name: String,
    pub cpu_arch: String,
    pub cpu_cores_physical: usize,
    pub cpu_cores_logical: usize,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub simd: SimdCapability,
    pub gpu: Option<GpuInfo>,
    pub os: String,
    pub os_version: String,
}

/// Hardware profiler that detects system capabilities.
pub struct HardwareProfiler;

impl HardwareProfiler {
    pub fn new() -> Self {
        Self
    }

    /// Detect the full hardware profile of the current system.
    pub fn detect(&self) -> HardwareProfile {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let cpu_cores_physical = System::physical_core_count().unwrap_or(1);
        let cpu_cores_logical = sys.cpus().len();
        let total_ram_bytes = sys.total_memory();
        let available_ram_bytes = sys.available_memory();

        let simd = SimdCapability::detect();
        let gpu = Self::detect_gpu();

        HardwareProfile {
            cpu_name,
            cpu_arch: std::env::consts::ARCH.to_string(),
            cpu_cores_physical,
            cpu_cores_logical,
            total_ram_bytes,
            available_ram_bytes,
            simd,
            gpu,
            os: std::env::consts::OS.to_string(),
            os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        }
    }

    /// Detect GPU presence.
    fn detect_gpu() -> Option<GpuInfo> {
        // On macOS with Apple Silicon, the GPU is integrated
        #[cfg(target_os = "macos")]
        {
            if std::env::consts::ARCH == "aarch64" {
                return Some(GpuInfo {
                    name: "Apple Silicon GPU".to_string(),
                    vendor: GpuVendor::Apple,
                    vram_bytes: None, // Shared memory on Apple Silicon
                });
            }
        }

        // TODO: NVIDIA GPU detection via nvidia-smi or NVML
        // TODO: AMD GPU detection
        None
    }

    /// Recommend maximum model size based on available RAM.
    pub fn recommend_max_model_bytes(&self, profile: &HardwareProfile) -> u64 {
        // Reserve ~2GB for OS and Fuse overhead
        let overhead = 2 * 1024 * 1024 * 1024u64;
        profile.available_ram_bytes.saturating_sub(overhead)
    }
}

impl Default for HardwareProfiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detection() {
        let profiler = HardwareProfiler::new();
        let profile = profiler.detect();

        assert!(profile.total_ram_bytes > 0, "RAM should be detected");
        // available_ram_bytes may be 0 on some platforms (e.g., containers)
        // so we only check total_ram_bytes is positive
        assert!(
            profile.cpu_cores_physical > 0,
            "Should have at least 1 core"
        );
        assert!(
            profile.cpu_cores_logical >= profile.cpu_cores_physical,
            "Logical cores >= physical"
        );
        assert!(!profile.cpu_arch.is_empty(), "CPU arch should be detected");
        assert!(!profile.os.is_empty(), "OS should be detected");
        assert!(!profile.cpu_name.is_empty(), "CPU name should be detected");
    }

    #[test]
    fn test_simd_detected() {
        let profiler = HardwareProfiler::new();
        let profile = profiler.detect();

        assert!(
            !profile.simd.features.is_empty(),
            "SIMD features should be detected"
        );
    }

    #[test]
    fn test_recommend_max_model_bytes() {
        let profiler = HardwareProfiler::new();
        let profile = profiler.detect();
        let max = profiler.recommend_max_model_bytes(&profile);

        // Should be less than total RAM
        assert!(max < profile.total_ram_bytes);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_apple_gpu_detected() {
        if std::env::consts::ARCH == "aarch64" {
            let profiler = HardwareProfiler::new();
            let profile = profiler.detect();
            assert!(
                profile.gpu.is_some(),
                "Apple Silicon GPU should be detected"
            );
            let gpu = profile.gpu.unwrap();
            assert_eq!(gpu.vendor, GpuVendor::Apple);
        }
    }
}
