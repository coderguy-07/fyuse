//! Quantizer trait — abstraction for model compression methods.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantConfig {
    pub bits: u8,
    pub group_size: usize,
    pub calibration_samples: usize,
}

impl Default for QuantConfig {
    fn default() -> Self {
        Self {
            bits: 4,
            group_size: 128,
            calibration_samples: 128,
        }
    }
}

/// Result of quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedModel {
    pub path: PathBuf,
    pub original_size_bytes: u64,
    pub quantized_size_bytes: u64,
    pub method: String,
    pub bits: u8,
}

/// Quality report comparing original vs quantized model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub perplexity_original: f64,
    pub perplexity_quantized: f64,
    pub perplexity_delta_pct: f64,
    pub passed: bool,
}

/// Core trait for quantization methods.
#[async_trait]
pub trait Quantizer: Send + Sync {
    /// Name of the quantization method.
    fn name(&self) -> &str;

    /// Supported bit widths.
    fn supported_bits(&self) -> &[u8];

    /// Quantize a model.
    async fn quantize(&self, model: &Path, config: QuantConfig) -> Result<QuantizedModel>;

    /// Validate quantized model quality.
    async fn validate(&self, original: &Path, quantized: &Path) -> Result<QualityReport>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockQuantizer;

    #[async_trait]
    impl Quantizer for MockQuantizer {
        fn name(&self) -> &str {
            "mock-q4"
        }
        fn supported_bits(&self) -> &[u8] {
            &[2, 4, 8]
        }
        async fn quantize(&self, _model: &Path, config: QuantConfig) -> Result<QuantizedModel> {
            Ok(QuantizedModel {
                path: PathBuf::from("/tmp/quantized"),
                original_size_bytes: 1_000_000,
                quantized_size_bytes: 250_000,
                method: "mock-q4".to_string(),
                bits: config.bits,
            })
        }
        async fn validate(&self, _original: &Path, _quantized: &Path) -> Result<QualityReport> {
            Ok(QualityReport {
                perplexity_original: 5.0,
                perplexity_quantized: 5.1,
                perplexity_delta_pct: 2.0,
                passed: true,
            })
        }
    }

    #[test]
    fn test_quantizer_info() {
        let q = MockQuantizer;
        assert_eq!(q.name(), "mock-q4");
        assert!(q.supported_bits().contains(&4));
    }

    #[tokio::test]
    async fn test_quantize() {
        let q = MockQuantizer;
        let result = q
            .quantize(Path::new("/tmp/model"), QuantConfig::default())
            .await
            .unwrap();
        assert!(result.quantized_size_bytes < result.original_size_bytes);
        assert_eq!(result.bits, 4);
    }

    #[tokio::test]
    async fn test_validate() {
        let q = MockQuantizer;
        let report = q
            .validate(Path::new("/tmp/orig"), Path::new("/tmp/quant"))
            .await
            .unwrap();
        assert!(report.passed);
        assert!(report.perplexity_delta_pct < 5.0);
    }
}
