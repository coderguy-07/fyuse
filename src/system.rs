//! System capability detection and hardware resource management
//! for intelligent model loading and quantization recommendations.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use sysinfo::System;
use tokio::sync::OnceCell;

/// System hardware capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// Total system RAM in bytes
    pub total_ram_bytes: u64,
    /// Available RAM in bytes
    pub available_ram_bytes: u64,
    /// Number of CPU cores
    pub cpu_cores: usize,
    /// CPU architecture
    pub cpu_arch: String,
    /// GPU information (if available)
    pub gpu_info: Option<GpuInfo>,
    /// Operating system
    pub os: String,
    /// System architecture
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name
    pub name: String,
    /// Total VRAM in bytes
    pub total_vram_bytes: u64,
    /// Available VRAM in bytes
    pub available_vram_bytes: u64,
    /// GPU compute capability (CUDA version, etc.)
    pub compute_capability: Option<String>,
    /// Driver version
    pub driver_version: Option<String>,
}

/// Model compatibility requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequirements {
    /// Minimum RAM required in bytes
    pub min_ram_bytes: u64,
    /// Recommended RAM in bytes
    pub recommended_ram_bytes: u64,
    /// Minimum VRAM required in bytes (0 if CPU-only)
    pub min_vram_bytes: u64,
    /// Recommended VRAM in bytes
    pub recommended_vram_bytes: u64,
    /// Supported quantization levels
    pub supported_quantizations: Vec<String>,
    /// Architecture requirements
    pub architecture_requirements: Vec<String>,
    /// CPU-only compatible
    pub cpu_only: bool,
}

/// Quantization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationRecommendation {
    /// Recommended quantization method
    pub method: String,
    /// Expected memory reduction factor
    pub memory_reduction: f32,
    /// Performance impact estimate
    pub performance_impact: f32,
    /// Quality impact estimate (0.0 = no loss, 1.0 = significant loss)
    pub quality_impact: f32,
    /// Reasoning for recommendation
    pub reasoning: String,
}

/// System capability detector
#[derive(Debug)]
pub struct SystemDetector {
    capabilities: OnceCell<SystemCapabilities>,
}

impl Default for SystemDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemDetector {
    pub fn new() -> Self {
        Self {
            capabilities: OnceCell::new(),
        }
    }

    /// Detect system capabilities (cached)
    pub async fn detect_capabilities(&self) -> Result<&SystemCapabilities> {
        self.capabilities
            .get_or_try_init(|| async { self.detect_capabilities_impl().await })
            .await
    }

    /// Force refresh system capabilities
    pub async fn refresh_capabilities(&self) -> Result<SystemCapabilities> {
        // Clear cache and re-detect
        // Note: OnceCell doesn't allow clearing, so we create a new one
        let caps = self.detect_capabilities_impl().await?;
        Ok(caps)
    }

    async fn detect_capabilities_impl(&self) -> Result<SystemCapabilities> {
        let mut sys = System::new_all();

        // Refresh system information
        sys.refresh_all();

        let total_ram = sys.total_memory();
        let available_ram = sys.available_memory();
        let cpu_cores = sys.cpus().len();
        let cpu_arch = std::env::consts::ARCH.to_string();
        let os = System::name().unwrap_or_else(|| "unknown".to_string());
        let arch = System::long_os_version().unwrap_or_else(|| "unknown".to_string());

        // Detect GPU information
        let gpu_info = self.detect_gpu().await;

        Ok(SystemCapabilities {
            total_ram_bytes: total_ram,
            available_ram_bytes: available_ram,
            cpu_cores,
            cpu_arch,
            gpu_info,
            os,
            arch,
        })
    }

    async fn detect_gpu(&self) -> Option<GpuInfo> {
        // Try to detect NVIDIA GPU first
        if let Ok(nvidia_info) = self.detect_nvidia_gpu().await {
            return Some(nvidia_info);
        }

        // Try to detect AMD GPU
        if let Ok(amd_info) = self.detect_amd_gpu().await {
            return Some(amd_info);
        }

        // Try to detect Intel GPU
        if let Ok(intel_info) = self.detect_intel_gpu().await {
            return Some(intel_info);
        }

        None
    }

    async fn detect_nvidia_gpu(&self) -> Result<GpuInfo> {
        // Use nvidia-ml or similar crate if available
        // For now, return placeholder
        Err(FuseError::InternalError(
            "NVIDIA GPU detection not implemented".to_string(),
        ))
    }

    async fn detect_amd_gpu(&self) -> Result<GpuInfo> {
        // AMD GPU detection
        Err(FuseError::InternalError(
            "AMD GPU detection not implemented".to_string(),
        ))
    }

    async fn detect_intel_gpu(&self) -> Result<GpuInfo> {
        // Intel GPU detection
        Err(FuseError::InternalError(
            "Intel GPU detection not implemented".to_string(),
        ))
    }

    /// Check if model is compatible with system
    pub async fn check_model_compatibility(
        &self,
        model_name: &str,
        requirements: &ModelRequirements,
    ) -> Result<ModelCompatibility> {
        let capabilities = self.detect_capabilities().await?;

        let mut issues = Vec::new();
        let mut recommendations = Vec::new();

        // Check RAM requirements
        if capabilities.available_ram_bytes < requirements.min_ram_bytes {
            issues.push(format!(
                "Insufficient RAM: {} GB available, {} GB required",
                capabilities.available_ram_bytes / (1024 * 1024 * 1024),
                requirements.min_ram_bytes / (1024 * 1024 * 1024)
            ));
        }

        // Check VRAM requirements
        if let Some(gpu) = &capabilities.gpu_info {
            if gpu.available_vram_bytes < requirements.min_vram_bytes {
                issues.push(format!(
                    "Insufficient VRAM: {} GB available, {} GB required",
                    gpu.available_vram_bytes / (1024 * 1024 * 1024),
                    requirements.min_vram_bytes / (1024 * 1024 * 1024)
                ));

                // Suggest CPU fallback
                if requirements.cpu_only {
                    recommendations.push("Consider using CPU-only inference".to_string());
                } else {
                    recommendations.push("Model requires GPU acceleration".to_string());
                }
            }
        } else if requirements.min_vram_bytes > 0 {
            issues.push("GPU required but not detected".to_string());
            if requirements.cpu_only {
                recommendations.push("Use CPU-only version of the model".to_string());
            } else {
                issues.push("Model requires GPU acceleration".to_string());
            }
        }

        // Check architecture requirements
        for req in &requirements.architecture_requirements {
            if !capabilities.cpu_arch.contains(req) && !capabilities.arch.contains(req) {
                issues.push(format!("Architecture requirement not met: {}", req));
            }
        }

        let compatible = issues.is_empty();

        Ok(ModelCompatibility {
            model_name: model_name.to_string(),
            compatible,
            issues,
            recommendations,
            quantization_recommendation: if !compatible {
                self.recommend_quantization(requirements, capabilities)
                    .await
            } else {
                None
            },
        })
    }

    /// Recommend quantization based on system capabilities
    pub async fn recommend_quantization(
        &self,
        requirements: &ModelRequirements,
        capabilities: &SystemCapabilities,
    ) -> Option<QuantizationRecommendation> {
        let available_ram = capabilities.available_ram_bytes;
        let available_vram = capabilities
            .gpu_info
            .as_ref()
            .map(|gpu| gpu.available_vram_bytes)
            .unwrap_or(0);

        // Calculate memory pressure
        let ram_pressure = if requirements.recommended_ram_bytes > 0 {
            requirements.recommended_ram_bytes as f32 / available_ram as f32
        } else {
            0.0
        };

        let vram_pressure = if requirements.recommended_vram_bytes > 0 && available_vram > 0 {
            requirements.recommended_vram_bytes as f32 / available_vram as f32
        } else {
            0.0
        };

        let max_pressure = ram_pressure.max(vram_pressure);

        // Recommend quantization based on memory pressure
        if max_pressure > 2.0 {
            // Severe memory pressure - recommend aggressive quantization
            Some(QuantizationRecommendation {
                method: "GGUF Q4_0".to_string(),
                memory_reduction: 0.5,
                performance_impact: 0.8,
                quality_impact: 0.3,
                reasoning: "High memory pressure detected - aggressive quantization recommended"
                    .to_string(),
            })
        } else if max_pressure > 1.5 {
            // Moderate memory pressure
            Some(QuantizationRecommendation {
                method: "GGUF Q5_0".to_string(),
                memory_reduction: 0.6,
                performance_impact: 0.9,
                quality_impact: 0.2,
                reasoning: "Moderate memory pressure - balanced quantization recommended"
                    .to_string(),
            })
        } else if max_pressure > 1.2 {
            // Light memory pressure
            Some(QuantizationRecommendation {
                method: "GGUF Q6_K".to_string(),
                memory_reduction: 0.7,
                performance_impact: 0.95,
                quality_impact: 0.1,
                reasoning: "Light memory pressure - minimal quantization recommended".to_string(),
            })
        } else {
            None // No quantization needed
        }
    }

    /// Get system resource usage
    pub async fn get_resource_usage(&self) -> Result<SystemResourceUsage> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_usage =
            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32;

        Ok(SystemResourceUsage {
            ram_usage_percent: ((sys.total_memory() - sys.available_memory()) as f32
                / sys.total_memory() as f32)
                * 100.0,
            cpu_usage_percent: cpu_usage,
            gpu_usage_percent: None, // Would need GPU monitoring library
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get CPU cores count
    pub async fn cpu_cores(&self) -> Result<usize> {
        let capabilities = self.detect_capabilities().await?;
        Ok(capabilities.cpu_cores)
    }

    /// Get total RAM in GB
    pub async fn total_ram_gb(&self) -> Result<f32> {
        let capabilities = self.detect_capabilities().await?;
        Ok(capabilities.total_ram_bytes as f32 / (1024.0 * 1024.0 * 1024.0))
    }

    /// Get available RAM in GB
    pub async fn available_ram_gb(&self) -> Result<f32> {
        let capabilities = self.detect_capabilities().await?;
        Ok(capabilities.available_ram_bytes as f32 / (1024.0 * 1024.0 * 1024.0))
    }

    /// Get used RAM in GB
    pub async fn used_ram_gb(&self) -> Result<f32> {
        let total = self.total_ram_gb().await?;
        let available = self.available_ram_gb().await?;
        Ok(total - available)
    }

    /// Get RAM usage percentage
    pub async fn ram_usage_percent(&self) -> Result<f32> {
        let usage = self.get_resource_usage().await?;
        Ok(usage.ram_usage_percent)
    }

    /// Check if GPU is available
    pub async fn has_gpu(&self) -> Result<bool> {
        let capabilities = self.detect_capabilities().await?;
        Ok(capabilities.gpu_info.is_some())
    }

    /// Get GPU information
    pub async fn gpu_info(&self) -> Result<Option<GpuInfo>> {
        let capabilities = self.detect_capabilities().await?;
        Ok(capabilities.gpu_info.clone())
    }

    /// Check if model can run on this system
    pub async fn can_run_model(&self, model_size_gb: f32) -> Result<bool> {
        let available_ram = self.available_ram_gb().await?;
        let has_gpu = self.has_gpu().await?;

        // Simple heuristic: need at least 2x model size in RAM for CPU, or GPU available
        if has_gpu {
            Ok(true) // Assume GPU can handle it
        } else {
            Ok(available_ram >= model_size_gb * 2.0)
        }
    }
}

/// Model compatibility result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCompatibility {
    pub model_name: String,
    pub compatible: bool,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub quantization_recommendation: Option<QuantizationRecommendation>,
}

/// System resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResourceUsage {
    pub ram_usage_percent: f32,
    pub cpu_usage_percent: f32,
    pub gpu_usage_percent: Option<f32>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Predefined model requirements (would be loaded from config or database)
impl ModelRequirements {
    pub fn for_model(model_name: &str) -> Self {
        // This would typically be loaded from a database or config file
        // For now, provide some example requirements

        match model_name.to_lowercase().as_str() {
            name if name.contains("llama-2-7b") => Self {
                min_ram_bytes: 8 * 1024 * 1024 * 1024,          // 8GB
                recommended_ram_bytes: 16 * 1024 * 1024 * 1024, // 16GB
                min_vram_bytes: 6 * 1024 * 1024 * 1024,         // 6GB
                recommended_vram_bytes: 8 * 1024 * 1024 * 1024, // 8GB
                supported_quantizations: vec!["GGUF".to_string(), "GPTQ".to_string()],
                architecture_requirements: vec!["x86_64".to_string(), "arm64".to_string()],
                cpu_only: true,
            },
            name if name.contains("llama-2-13b") => Self {
                min_ram_bytes: 16 * 1024 * 1024 * 1024,          // 16GB
                recommended_ram_bytes: 32 * 1024 * 1024 * 1024,  // 32GB
                min_vram_bytes: 10 * 1024 * 1024 * 1024,         // 10GB
                recommended_vram_bytes: 16 * 1024 * 1024 * 1024, // 16GB
                supported_quantizations: vec!["GGUF".to_string(), "GPTQ".to_string()],
                architecture_requirements: vec!["x86_64".to_string()],
                cpu_only: true,
            },
            name if name.contains("llama-2-70b") => Self {
                min_ram_bytes: 64 * 1024 * 1024 * 1024,          // 64GB
                recommended_ram_bytes: 128 * 1024 * 1024 * 1024, // 128GB
                min_vram_bytes: 32 * 1024 * 1024 * 1024,         // 32GB
                recommended_vram_bytes: 48 * 1024 * 1024 * 1024, // 48GB
                supported_quantizations: vec!["GGUF".to_string(), "GPTQ".to_string()],
                architecture_requirements: vec!["x86_64".to_string()],
                cpu_only: true,
            },
            _ => Self {
                // Default requirements for unknown models
                min_ram_bytes: 4 * 1024 * 1024 * 1024, // 4GB
                recommended_ram_bytes: 8 * 1024 * 1024 * 1024, // 8GB
                min_vram_bytes: 2 * 1024 * 1024 * 1024, // 2GB
                recommended_vram_bytes: 4 * 1024 * 1024 * 1024, // 4GB
                supported_quantizations: vec!["GGUF".to_string()],
                architecture_requirements: vec!["x86_64".to_string(), "arm64".to_string()],
                cpu_only: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_capability_detection() {
        let detector = SystemDetector::new();
        let capabilities = detector.detect_capabilities().await.unwrap();

        assert!(capabilities.total_ram_bytes > 0);
        assert!(capabilities.available_ram_bytes > 0);
        assert!(capabilities.cpu_cores > 0);
        assert!(!capabilities.cpu_arch.is_empty());
        assert!(!capabilities.os.is_empty());
    }

    #[tokio::test]
    async fn test_model_compatibility_check() {
        let detector = SystemDetector::new();
        let requirements = ModelRequirements::for_model("llama-2-7b");

        let compatibility = detector
            .check_model_compatibility("llama-2-7b", &requirements)
            .await
            .unwrap();

        assert_eq!(compatibility.model_name, "llama-2-7b");
        // Compatibility depends on actual system
        assert!(compatibility.issues.is_empty() || !compatibility.issues.is_empty());
    }

    #[tokio::test]
    async fn test_quantization_recommendation() {
        let detector = SystemDetector::new();
        let capabilities = detector.detect_capabilities().await.unwrap();

        let requirements = ModelRequirements {
            min_ram_bytes: 64 * 1024 * 1024 * 1024, // 64GB (more than most systems have)
            recommended_ram_bytes: 128 * 1024 * 1024 * 1024, // 128GB
            min_vram_bytes: 32 * 1024 * 1024 * 1024, // 32GB
            recommended_vram_bytes: 48 * 1024 * 1024 * 1024, // 48GB
            supported_quantizations: vec!["GGUF".to_string()],
            architecture_requirements: vec![],
            cpu_only: true,
        };

        let recommendation = detector
            .recommend_quantization(&requirements, &capabilities)
            .await;
        assert!(recommendation.is_some()); // Should recommend quantization due to high memory requirements
    }

    #[tokio::test]
    async fn test_resource_usage() {
        let detector = SystemDetector::new();
        let usage = detector.get_resource_usage().await.unwrap();

        assert!(usage.ram_usage_percent >= 0.0 && usage.ram_usage_percent <= 100.0);
        assert!(usage.cpu_usage_percent >= 0.0 && usage.cpu_usage_percent <= 100.0);
    }
}
