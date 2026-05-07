use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum QuantizationMethod {
    // GGUF formats
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,

    // GPTQ
    GPTQ,

    // AWQ
    AWQ,

    // K-quant variants
    Q4_K_M,
    Q5_K_M,
    Q6_K,

    // GGML
    GGML,
}

impl QuantizationMethod {
    pub fn parse_method(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "q4_0" | "q4-0" => Ok(Self::Q4_0),
            "q4_1" | "q4-1" => Ok(Self::Q4_1),
            "q5_0" | "q5-0" => Ok(Self::Q5_0),
            "q5_1" | "q5-1" => Ok(Self::Q5_1),
            "q8_0" | "q8-0" => Ok(Self::Q8_0),
            "q4_k_m" | "q4-k-m" | "q4_k" => Ok(Self::Q4_K_M),
            "q5_k_m" | "q5-k-m" | "q5_k" => Ok(Self::Q5_K_M),
            "q6_k" | "q6-k" => Ok(Self::Q6_K),
            "gptq" => Ok(Self::GPTQ),
            "awq" => Ok(Self::AWQ),
            "ggml" => Ok(Self::GGML),
            _ => Err(FuseError::ValidationError(format!(
                "Unknown quantization method: {}",
                s
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Q4_0 => "q4_0",
            Self::Q4_1 => "q4_1",
            Self::Q5_0 => "q5_0",
            Self::Q5_1 => "q5_1",
            Self::Q8_0 => "q8_0",
            Self::Q4_K_M => "q4_k_m",
            Self::Q5_K_M => "q5_k_m",
            Self::Q6_K => "q6_k",
            Self::GPTQ => "gptq",
            Self::AWQ => "awq",
            Self::GGML => "ggml",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Q4_0 => "4-bit quantization (method 0) - smallest size, lower quality",
            Self::Q4_1 => "4-bit quantization (method 1) - small size, better quality than Q4_0",
            Self::Q5_0 => "5-bit quantization (method 0) - balanced size and quality",
            Self::Q5_1 => "5-bit quantization (method 1) - balanced size, better quality than Q5_0",
            Self::Q8_0 => "8-bit quantization - larger size, high quality",
            Self::Q4_K_M => "4-bit K-quant (medium) - best quality/size ratio for 4-bit",
            Self::Q5_K_M => "5-bit K-quant (medium) - excellent quality, moderate size",
            Self::Q6_K => "6-bit K-quant - near-lossless quality",
            Self::GPTQ => "GPTQ quantization - GPU-optimized, requires calibration data",
            Self::AWQ => "AWQ quantization - activation-aware, high quality",
            Self::GGML => "GGML format quantization - CPU-optimized",
        }
    }

    pub fn expected_size_reduction(&self) -> f32 {
        match self {
            Self::Q4_0 | Self::Q4_1 => 0.25, // ~75% reduction
            Self::Q4_K_M => 0.27,            // ~73% reduction (slightly better than Q4_0)
            Self::Q5_0 | Self::Q5_1 => 0.31, // ~69% reduction
            Self::Q5_K_M => 0.33,            // ~67% reduction
            Self::Q6_K => 0.38,              // ~62% reduction
            Self::Q8_0 => 0.50,              // ~50% reduction
            Self::GPTQ => 0.25,              // ~75% reduction
            Self::AWQ => 0.25,               // ~75% reduction
            Self::GGML => 0.50,              // ~50% reduction
        }
    }

    pub fn is_gpu_optimized(&self) -> bool {
        matches!(self, Self::GPTQ | Self::AWQ)
    }

    pub fn is_cpu_optimized(&self) -> bool {
        matches!(
            self,
            Self::Q4_0
                | Self::Q4_1
                | Self::Q5_0
                | Self::Q5_1
                | Self::Q8_0
                | Self::Q4_K_M
                | Self::Q5_K_M
                | Self::Q6_K
                | Self::GGML
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    pub method: QuantizationMethod,
    pub calibration_dataset: Option<String>,
    pub num_samples: Option<usize>,
    pub block_size: Option<usize>,
    pub preserve_embeddings: bool,
}

impl Default for QuantizationConfig {
    fn default() -> Self {
        Self {
            method: QuantizationMethod::Q4_0,
            calibration_dataset: None,
            num_samples: Some(128),
            block_size: Some(128),
            preserve_embeddings: true,
        }
    }
}

impl QuantizationConfig {
    pub fn new(method: QuantizationMethod) -> Self {
        Self {
            method,
            ..Default::default()
        }
    }

    pub fn with_calibration_dataset(mut self, dataset: String) -> Self {
        self.calibration_dataset = Some(dataset);
        self
    }

    pub fn with_num_samples(mut self, num_samples: usize) -> Self {
        self.num_samples = Some(num_samples);
        self
    }

    pub fn validate(&self) -> Result<()> {
        // GPTQ requires calibration dataset
        if self.method == QuantizationMethod::GPTQ && self.calibration_dataset.is_none() {
            return Err(FuseError::ValidationError(
                "GPTQ quantization requires a calibration dataset".to_string(),
            ));
        }

        // Validate num_samples
        if let Some(samples) = self.num_samples {
            if samples == 0 {
                return Err(FuseError::ValidationError(
                    "Number of samples must be greater than 0".to_string(),
                ));
            }
        }

        // Validate block_size
        if let Some(block_size) = self.block_size {
            if block_size == 0 || block_size > 1024 {
                return Err(FuseError::ValidationError(
                    "Block size must be between 1 and 1024".to_string(),
                ));
            }
        }

        Ok(())
    }
}

pub struct QuantizationProgress {
    pub current_layer: usize,
    pub total_layers: usize,
    pub current_step: String,
    pub percentage: f32,
}

impl QuantizationProgress {
    pub fn new(total_layers: usize) -> Self {
        Self {
            current_layer: 0,
            total_layers,
            current_step: "Initializing".to_string(),
            percentage: 0.0,
        }
    }

    pub fn update(&mut self, layer: usize, step: String) {
        self.current_layer = layer;
        self.current_step = step;
        self.percentage = if self.total_layers > 0 {
            (layer as f32 / self.total_layers as f32) * 100.0
        } else {
            0.0
        };
    }

    pub fn format(&self) -> String {
        format!(
            "[{}/{}] {} ({:.1}%)",
            self.current_layer, self.total_layers, self.current_step, self.percentage
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_method_from_str() {
        assert_eq!(
            QuantizationMethod::parse_method("q4_0").unwrap(),
            QuantizationMethod::Q4_0
        );
        assert_eq!(
            QuantizationMethod::parse_method("Q4-0").unwrap(),
            QuantizationMethod::Q4_0
        );
        assert_eq!(
            QuantizationMethod::parse_method("gptq").unwrap(),
            QuantizationMethod::GPTQ
        );
        assert!(QuantizationMethod::parse_method("invalid").is_err());
    }

    #[test]
    fn test_k_quant_variants_parse() {
        assert_eq!(
            QuantizationMethod::parse_method("q4_k_m").expect("parse q4_k_m"),
            QuantizationMethod::Q4_K_M
        );
        assert_eq!(
            QuantizationMethod::parse_method("Q4-K-M").expect("parse Q4-K-M"),
            QuantizationMethod::Q4_K_M
        );
        assert_eq!(
            QuantizationMethod::parse_method("q5_k_m").expect("parse q5_k_m"),
            QuantizationMethod::Q5_K_M
        );
        assert_eq!(
            QuantizationMethod::parse_method("q6_k").expect("parse q6_k"),
            QuantizationMethod::Q6_K
        );
    }

    #[test]
    fn test_k_quant_variants_as_str() {
        assert_eq!(QuantizationMethod::Q4_K_M.as_str(), "q4_k_m");
        assert_eq!(QuantizationMethod::Q5_K_M.as_str(), "q5_k_m");
        assert_eq!(QuantizationMethod::Q6_K.as_str(), "q6_k");
    }

    #[test]
    fn test_k_quant_variants_properties() {
        assert!(QuantizationMethod::Q4_K_M.is_cpu_optimized());
        assert!(!QuantizationMethod::Q4_K_M.is_gpu_optimized());
        assert!(QuantizationMethod::Q5_K_M.is_cpu_optimized());
        assert!(QuantizationMethod::Q6_K.is_cpu_optimized());

        // Size reductions ordered correctly
        let q4_km = QuantizationMethod::Q4_K_M.expected_size_reduction();
        let q5_km = QuantizationMethod::Q5_K_M.expected_size_reduction();
        let q6_k = QuantizationMethod::Q6_K.expected_size_reduction();
        assert!(q4_km < q5_km);
        assert!(q5_km < q6_k);
    }

    #[test]
    fn test_k_quant_descriptions_not_empty() {
        assert!(!QuantizationMethod::Q4_K_M.description().is_empty());
        assert!(!QuantizationMethod::Q5_K_M.description().is_empty());
        assert!(!QuantizationMethod::Q6_K.description().is_empty());
    }

    #[test]
    fn test_quantization_config_validation() {
        let config = QuantizationConfig::new(QuantizationMethod::Q4_0);
        assert!(config.validate().is_ok());

        let gptq_config = QuantizationConfig::new(QuantizationMethod::GPTQ);
        assert!(gptq_config.validate().is_err()); // Missing calibration dataset

        let gptq_with_dataset = gptq_config.with_calibration_dataset("dataset.txt".to_string());
        assert!(gptq_with_dataset.validate().is_ok());
    }

    #[test]
    fn test_quantization_progress() {
        let mut progress = QuantizationProgress::new(10);
        assert_eq!(progress.percentage, 0.0);

        progress.update(5, "Processing".to_string());
        assert_eq!(progress.percentage, 50.0);
        assert!(progress.format().contains("50.0%"));
    }
}
