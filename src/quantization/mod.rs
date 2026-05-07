pub mod compatibility;
pub mod gguf_codec;
pub mod methods;
pub mod optimizer;
pub mod quantizer;
pub mod traits;
pub mod validator;

pub use compatibility::QuantizationCompatibility;
pub use methods::{QuantizationConfig, QuantizationMethod, QuantizationProgress};
pub use optimizer::{LayerSensitivity, QuantizationOptimizer};
pub use quantizer::Quantizer;
pub use validator::{QualityReport as QuantizationQualityReport, QualityValidator};

use crate::error::{FuseError, Result};
use crate::system::SystemDetector;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationRecommendation {
    pub method: QuantizationMethod,
    pub confidence: f32,
    pub reason: String,
    pub estimated_size_mb: Option<u64>,
    pub performance_impact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationResult {
    pub method: QuantizationMethod,
    pub input_size_mb: u64,
    pub output_size_mb: u64,
    pub compression_ratio: f32,
    pub duration_secs: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

pub struct QuantizationService {
    quantizer: Quantizer,
    compatibility_checker: QuantizationCompatibility,
    optimizer: QuantizationOptimizer,
    system_detector: SystemDetector,
}

impl QuantizationService {
    pub fn new(cache_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            quantizer: Quantizer::new(&cache_dir),
            compatibility_checker: QuantizationCompatibility::new(),
            optimizer: QuantizationOptimizer::new(),
            system_detector: SystemDetector::new(),
        })
    }

    pub async fn quantize(
        &self,
        model_path: &Path,
        config: &QuantizationConfig,
        output_path: &Path,
    ) -> Result<QuantizationResult> {
        config.validate()?;

        let start_time = std::time::Instant::now();
        let input_size = self.get_model_size(model_path)?;

        info!("Starting quantization with method: {:?}", config.method);
        info!("Input model size: {} MB", input_size / 1024 / 1024);

        let result = match self
            .quantizer
            .quantize(model_path, output_path, config)
            .await
        {
            Ok(()) => {
                let output_size = self.get_model_size(output_path)?;
                let compression_ratio = input_size as f32 / output_size as f32;

                info!("Quantization completed successfully");
                info!(
                    "Output size: {} MB, Compression ratio: {:.2}x",
                    output_size / 1024 / 1024,
                    compression_ratio
                );

                QuantizationResult {
                    method: config.method,
                    input_size_mb: input_size / 1024 / 1024,
                    output_size_mb: output_size / 1024 / 1024,
                    compression_ratio,
                    duration_secs: start_time.elapsed().as_secs(),
                    success: true,
                    error_message: None,
                }
            }
            Err(e) => {
                error!("Quantization failed: {}", e);
                QuantizationResult {
                    method: config.method,
                    input_size_mb: input_size / 1024 / 1024,
                    output_size_mb: 0,
                    compression_ratio: 1.0,
                    duration_secs: start_time.elapsed().as_secs(),
                    success: false,
                    error_message: Some(e.to_string()),
                }
            }
        };

        Ok(result)
    }

    pub async fn recommend_quantization(
        &self,
        model_path: &Path,
    ) -> Result<Vec<QuantizationRecommendation>> {
        info!("Analyzing system and model for quantization recommendations");

        let system_info = self.system_detector.detect_capabilities().await?;
        let model_info = self.analyze_model(model_path).await?;

        let mut recommendations = Vec::new();

        // CPU-optimized recommendations
        let ram_gb = system_info.total_ram_bytes / 1024 / 1024 / 1024;
        if ram_gb >= 8 {
            recommendations.push(QuantizationRecommendation {
                method: QuantizationMethod::Q4_0,
                confidence: 0.9,
                reason: "High RAM availability allows efficient CPU inference".to_string(),
                estimated_size_mb: Some((model_info.size_mb as f32 * 0.25) as u64),
                performance_impact: "Minimal impact on CPU inference".to_string(),
            });
        } else if ram_gb >= 4 {
            recommendations.push(QuantizationRecommendation {
                method: QuantizationMethod::Q8_0,
                confidence: 0.8,
                reason: "Balanced RAM usage for CPU inference".to_string(),
                estimated_size_mb: Some((model_info.size_mb as f32 * 0.5) as u64),
                performance_impact: "Good balance of size and performance".to_string(),
            });
        }

        // GPU-optimized recommendations
        let has_gpu = system_info.gpu_info.is_some();
        let gpu_vram_gb = system_info
            .gpu_info
            .as_ref()
            .map(|gpu| gpu.total_vram_bytes / 1024 / 1024 / 1024)
            .unwrap_or(0);

        if has_gpu && gpu_vram_gb >= 4 {
            recommendations.push(QuantizationRecommendation {
                method: QuantizationMethod::GPTQ,
                confidence: 0.85,
                reason: "GPU available with sufficient VRAM".to_string(),
                estimated_size_mb: Some((model_info.size_mb as f32 * 0.25) as u64),
                performance_impact: "Optimized for GPU inference".to_string(),
            });

            recommendations.push(QuantizationRecommendation {
                method: QuantizationMethod::AWQ,
                confidence: 0.8,
                reason: "High-quality GPU quantization".to_string(),
                estimated_size_mb: Some((model_info.size_mb as f32 * 0.25) as u64),
                performance_impact: "Excellent quality retention".to_string(),
            });
        }

        // Fallback recommendations
        if recommendations.is_empty() {
            recommendations.push(QuantizationRecommendation {
                method: QuantizationMethod::GGML,
                confidence: 0.6,
                reason: "Fallback CPU quantization for limited resources".to_string(),
                estimated_size_mb: Some((model_info.size_mb as f32 * 0.5) as u64),
                performance_impact: "Basic CPU optimization".to_string(),
            });
        }

        // Sort by confidence
        recommendations.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        Ok(recommendations)
    }

    pub async fn check_compatibility(
        &self,
        model_path: &Path,
        method: QuantizationMethod,
    ) -> Result<bool> {
        self.compatibility_checker
            .check_compatibility(model_path, method)
            .await
    }

    pub async fn optimize_quantized_model(&self, model_path: &Path) -> Result<()> {
        self.optimizer.optimize_model(model_path).await
    }

    pub async fn validate_quantized_model(&self, model_path: &Path) -> Result<bool> {
        self.quantizer.validate_quantized_model(model_path).await
    }

    pub fn supported_methods(&self) -> Vec<QuantizationMethod> {
        vec![
            QuantizationMethod::Q4_0,
            QuantizationMethod::Q4_1,
            QuantizationMethod::Q5_0,
            QuantizationMethod::Q5_1,
            QuantizationMethod::Q8_0,
            QuantizationMethod::Q4_K_M,
            QuantizationMethod::Q5_K_M,
            QuantizationMethod::Q6_K,
            QuantizationMethod::GPTQ,
            QuantizationMethod::AWQ,
            QuantizationMethod::GGML,
        ]
    }

    pub async fn get_method_info(&self, method: QuantizationMethod) -> QuantizationMethodInfo {
        QuantizationMethodInfo {
            method,
            description: method.description().to_string(),
            size_reduction: method.expected_size_reduction(),
            gpu_optimized: method.is_gpu_optimized(),
            cpu_optimized: method.is_cpu_optimized(),
            requires_calibration: matches!(method, QuantizationMethod::GPTQ),
            supported_architectures: self.get_supported_architectures(method),
        }
    }

    fn get_supported_architectures(&self, method: QuantizationMethod) -> Vec<String> {
        match method {
            QuantizationMethod::Q4_0
            | QuantizationMethod::Q4_1
            | QuantizationMethod::Q5_0
            | QuantizationMethod::Q5_1
            | QuantizationMethod::Q8_0
            | QuantizationMethod::Q4_K_M
            | QuantizationMethod::Q5_K_M
            | QuantizationMethod::Q6_K => {
                vec!["llama".to_string(), "gpt".to_string(), "bert".to_string()]
            }
            QuantizationMethod::GPTQ => vec!["llama".to_string(), "gpt".to_string()],
            QuantizationMethod::AWQ => vec!["llama".to_string(), "gpt".to_string()],
            QuantizationMethod::GGML => vec!["llama".to_string()],
        }
    }

    async fn analyze_model(&self, model_path: &Path) -> Result<ModelAnalysis> {
        let size_bytes = self.get_model_size(model_path)?;
        let size_mb = size_bytes / 1024 / 1024;

        // Basic model analysis - could be extended
        Ok(ModelAnalysis {
            size_mb,
            architecture: "unknown".to_string(), // Would need model inspection
            parameter_count: None,
        })
    }

    fn get_model_size(&self, path: &Path) -> Result<u64> {
        if path.is_file() {
            let metadata = std::fs::metadata(path)?;
            Ok(metadata.len())
        } else if path.is_dir() {
            // Calculate directory size recursively
            self.calculate_directory_size(path)
        } else {
            Err(FuseError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Model path not found",
            )))
        }
    }

    fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total_size = 0u64;
        Self::calculate_directory_size_recursive(path, &mut total_size)?;
        Ok(total_size)
    }

    fn calculate_directory_size_recursive(path: &Path, total_size: &mut u64) -> Result<()> {
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_file() {
                let metadata = entry.metadata()?;
                *total_size += metadata.len();
            } else if entry_path.is_dir() {
                Self::calculate_directory_size_recursive(&entry_path, total_size)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct QuantizationMethodInfo {
    pub method: QuantizationMethod,
    pub description: String,
    pub size_reduction: f32,
    pub gpu_optimized: bool,
    pub cpu_optimized: bool,
    pub requires_calibration: bool,
    pub supported_architectures: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ModelAnalysis {
    size_mb: u64,
    architecture: String,
    parameter_count: Option<u64>,
}
