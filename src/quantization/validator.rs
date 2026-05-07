//! Quality validation for quantized weights.

use crate::error::{FuseError, Result};
use crate::quantization::methods::QuantizationMethod;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::gguf_codec;

/// Quality report from comparing original and quantized weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub method: QuantizationMethod,
    pub max_error: f32,
    pub mean_error: f32,
    pub rmse: f32,
    pub passed: bool,
}

/// Configurable quality thresholds per quantization method.
#[derive(Debug, Clone)]
pub struct QualityThresholds {
    pub max_error: f32,
    pub mean_error: f32,
    pub rmse: f32,
}

/// Validator for quantization quality.
pub struct QualityValidator {
    thresholds: std::collections::HashMap<String, QualityThresholds>,
}

impl Default for QualityValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityValidator {
    pub fn new() -> Self {
        let mut thresholds = std::collections::HashMap::new();

        // 4-bit methods: higher tolerance
        for key in &["q4_0", "q4_1", "q4_k_m"] {
            thresholds.insert(
                (*key).to_string(),
                QualityThresholds {
                    max_error: 0.5,
                    mean_error: 0.15,
                    rmse: 0.2,
                },
            );
        }

        // 5-bit methods
        for key in &["q5_0", "q5_1", "q5_k_m"] {
            thresholds.insert(
                (*key).to_string(),
                QualityThresholds {
                    max_error: 0.3,
                    mean_error: 0.1,
                    rmse: 0.15,
                },
            );
        }

        // 6-bit
        thresholds.insert(
            "q6_k".to_string(),
            QualityThresholds {
                max_error: 0.2,
                mean_error: 0.05,
                rmse: 0.08,
            },
        );

        // 8-bit methods: tighter tolerance
        thresholds.insert(
            "q8_0".to_string(),
            QualityThresholds {
                max_error: 0.1,
                mean_error: 0.02,
                rmse: 0.03,
            },
        );

        Self { thresholds }
    }

    /// Set custom thresholds for a method.
    pub fn set_thresholds(&mut self, method: QuantizationMethod, thresholds: QualityThresholds) {
        self.thresholds
            .insert(method.as_str().to_string(), thresholds);
    }

    /// Validate quantization quality by dequantizing and comparing to original.
    pub fn validate_quantization(
        &self,
        original: &[f32],
        quantized: &[u8],
        method: QuantizationMethod,
    ) -> Result<QualityReport> {
        info!(
            method = method.as_str(),
            original_len = original.len(),
            "Validating quantization quality"
        );

        let dequantized = match method {
            QuantizationMethod::Q4_0 => gguf_codec::dequantize_q4_0(quantized),
            QuantizationMethod::Q8_0 => gguf_codec::dequantize_q8_0(quantized),
            _ => {
                return Err(FuseError::QuantizationError(format!(
                    "Validation not yet supported for method: {}",
                    method.as_str()
                )));
            }
        };

        if dequantized.len() < original.len() {
            return Err(FuseError::QuantizationError(
                "Dequantized output shorter than original input".to_string(),
            ));
        }

        let len = original.len();
        let mut max_error: f32 = 0.0;
        let mut sum_error: f32 = 0.0;
        let mut sum_sq_error: f32 = 0.0;

        for i in 0..len {
            let err = (original[i] - dequantized[i]).abs();
            if err > max_error {
                max_error = err;
            }
            sum_error += err;
            sum_sq_error += err * err;
        }

        let mean_error = if len > 0 { sum_error / len as f32 } else { 0.0 };
        let rmse = if len > 0 {
            (sum_sq_error / len as f32).sqrt()
        } else {
            0.0
        };

        let passed = self.check_thresholds(method, max_error, mean_error, rmse);

        info!(
            method = method.as_str(),
            max_error, mean_error, rmse, passed, "Quantization quality validation complete"
        );

        Ok(QualityReport {
            method,
            max_error,
            mean_error,
            rmse,
            passed,
        })
    }

    fn check_thresholds(
        &self,
        method: QuantizationMethod,
        max_err: f32,
        mean_err: f32,
        rmse: f32,
    ) -> bool {
        if let Some(t) = self.thresholds.get(method.as_str()) {
            max_err <= t.max_error && mean_err <= t.mean_error && rmse <= t.rmse
        } else {
            // Default: generous thresholds
            max_err <= 1.0 && mean_err <= 0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_q4_0_good_quantization() {
        let validator = QualityValidator::new();
        let weights: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) / 32.0).collect();
        let quantized = gguf_codec::quantize_q4_0(&weights);

        let report = validator
            .validate_quantization(&weights, &quantized, QuantizationMethod::Q4_0)
            .expect("validation should succeed");

        assert!(
            report.passed,
            "Good Q4_0 quantization should pass: {:?}",
            report
        );
        assert!(report.max_error < 0.5);
        assert!(report.rmse < 0.2);
    }

    #[test]
    fn test_validate_q8_0_good_quantization() {
        let validator = QualityValidator::new();
        let weights: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) / 32.0).collect();
        let quantized = gguf_codec::quantize_q8_0(&weights);

        let report = validator
            .validate_quantization(&weights, &quantized, QuantizationMethod::Q8_0)
            .expect("validation should succeed");

        assert!(
            report.passed,
            "Good Q8_0 quantization should pass: {:?}",
            report
        );
        assert!(report.max_error < 0.1);
    }

    #[test]
    fn test_validate_bad_quantization_fails() {
        let validator = QualityValidator::new();
        let weights: Vec<f32> = (0..32).map(|i| i as f32).collect();

        // Use Q4_0 quantized data but claim it's Q8_0 -- this will produce garbage
        let quantized = gguf_codec::quantize_q4_0(&weights);

        // Dequantizing Q4_0 data as Q8_0 should produce wrong results
        // But the data size won't match Q8_0 blocks. Let's craft bad data instead.
        // Create all-zero quantized data (scale=0 means all output is 0)
        let fake_quantized = vec![0u8; 34]; // One Q8_0 block, all zeros
        let report = validator
            .validate_quantization(&weights, &fake_quantized, QuantizationMethod::Q8_0)
            .expect("validation should succeed");

        // Weights are 0..31, so dequantized all-zeros will have large error
        assert!(!report.passed, "Bad quantization should fail: {:?}", report);
    }

    #[test]
    fn test_validate_zeros() {
        let validator = QualityValidator::new();
        let weights = vec![0.0_f32; 32];
        let quantized = gguf_codec::quantize_q4_0(&weights);

        let report = validator
            .validate_quantization(&weights, &quantized, QuantizationMethod::Q4_0)
            .expect("validation should succeed");

        assert!(report.passed);
        assert!(report.max_error < f32::EPSILON);
    }

    #[test]
    fn test_custom_thresholds() {
        let mut validator = QualityValidator::new();
        validator.set_thresholds(
            QuantizationMethod::Q4_0,
            QualityThresholds {
                max_error: 0.001,
                mean_error: 0.001,
                rmse: 0.001,
            },
        );

        let weights: Vec<f32> = (0..32).map(|i| (i as f32 - 16.0) / 16.0).collect();
        let quantized = gguf_codec::quantize_q4_0(&weights);

        let report = validator
            .validate_quantization(&weights, &quantized, QuantizationMethod::Q4_0)
            .expect("validation should succeed");

        // With very tight thresholds, Q4 should fail
        assert!(
            !report.passed,
            "Tight thresholds should cause Q4_0 to fail: {:?}",
            report
        );
    }

    #[test]
    fn test_unsupported_method_returns_error() {
        let validator = QualityValidator::new();
        let weights = vec![0.0_f32; 32];
        let data = vec![0u8; 32];

        let result = validator.validate_quantization(&weights, &data, QuantizationMethod::GPTQ);
        assert!(result.is_err());
    }
}
