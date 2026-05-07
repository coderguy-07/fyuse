use crate::error::{FuseError, Result};
use crate::quantization::QuantizationMethod;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

pub struct QuantizationCompatibility {
    // Cache for compatibility checks
    compatibility_cache: std::collections::HashMap<String, bool>,
}

impl Default for QuantizationCompatibility {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantizationCompatibility {
    pub fn new() -> Self {
        Self {
            compatibility_cache: std::collections::HashMap::new(),
        }
    }

    pub async fn check_compatibility(
        &self,
        model_path: &Path,
        method: QuantizationMethod,
    ) -> Result<bool> {
        let cache_key = format!("{}:{}", model_path.display(), method.as_str());

        if let Some(&result) = self.compatibility_cache.get(&cache_key) {
            debug!("Using cached compatibility result for {}", cache_key);
            return Ok(result);
        }

        let result = self
            .check_compatibility_uncached(model_path, method)
            .await?;
        // Note: In a real implementation, we'd need mutable access to cache
        // For now, we'll skip caching to avoid borrow checker issues

        Ok(result)
    }

    async fn check_compatibility_uncached(
        &self,
        model_path: &Path,
        method: QuantizationMethod,
    ) -> Result<bool> {
        info!(
            "Checking compatibility of {} with method {:?}",
            model_path.display(),
            method
        );

        // Basic file existence check
        if !model_path.exists() {
            return Ok(false);
        }

        match method {
            QuantizationMethod::Q4_0
            | QuantizationMethod::Q4_1
            | QuantizationMethod::Q5_0
            | QuantizationMethod::Q5_1
            | QuantizationMethod::Q8_0
            | QuantizationMethod::Q4_K_M
            | QuantizationMethod::Q5_K_M
            | QuantizationMethod::Q6_K => self.check_gguf_compatibility(model_path).await,
            QuantizationMethod::GPTQ => self.check_gptq_compatibility(model_path).await,
            QuantizationMethod::AWQ => self.check_awq_compatibility(model_path).await,
            QuantizationMethod::GGML => self.check_ggml_compatibility(model_path).await,
        }
    }

    async fn check_gguf_compatibility(&self, model_path: &Path) -> Result<bool> {
        // GGUF quantization works with most transformer models
        // Check for common model files
        let model_files = vec!["pytorch_model.bin", "model.safetensors", "config.json"];

        for file in model_files {
            if model_path.join(file).exists() {
                debug!("Found model file: {}", file);
                return Ok(true);
            }
        }

        // Check if it's already a GGUF file
        if let Some(extension) = model_path.extension() {
            if extension == "gguf" {
                debug!("Model is already in GGUF format");
                return Ok(false); // Can't quantize already quantized GGUF
            }
        }

        // Check for model directory
        if model_path.is_dir() {
            let mut entries = fs::read_dir(model_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.contains("pytorch") || filename.contains("safetensors") {
                    debug!("Found model file in directory: {}", filename);
                    return Ok(true);
                }
            }
        }

        warn!("No compatible model files found for GGUF quantization");
        Ok(false)
    }

    async fn check_gptq_compatibility(&self, model_path: &Path) -> Result<bool> {
        // GPTQ requires specific model architectures and calibration data
        if let Ok(config) = self.read_model_config(model_path).await {
            // Check for supported architectures
            let supported_architectures = ["llama", "gpt2", "gpt_neo", "gpt_neox", "opt", "bloom"];

            if let Some(arch) = config.get("model_type").and_then(|v| v.as_str()) {
                let is_supported = supported_architectures.iter().any(|&a| arch.contains(a));
                if is_supported {
                    debug!("Model architecture '{}' is compatible with GPTQ", arch);
                    return Ok(true);
                } else {
                    warn!("Model architecture '{}' is not supported by GPTQ", arch);
                    return Ok(false);
                }
            }
        }

        // Fallback: assume compatible if config exists
        let config_path = model_path.join("config.json");
        if config_path.exists() {
            debug!("Model has config.json, assuming GPTQ compatibility");
            Ok(true)
        } else {
            warn!("No config.json found for GPTQ compatibility check");
            Ok(false)
        }
    }

    async fn check_awq_compatibility(&self, model_path: &Path) -> Result<bool> {
        // AWQ works with similar architectures as GPTQ
        if let Ok(config) = self.read_model_config(model_path).await {
            let supported_architectures = ["llama", "gpt2", "opt", "bloom"];

            if let Some(arch) = config.get("model_type").and_then(|v| v.as_str()) {
                let is_supported = supported_architectures.iter().any(|&a| arch.contains(a));
                if is_supported {
                    debug!("Model architecture '{}' is compatible with AWQ", arch);
                    return Ok(true);
                } else {
                    warn!("Model architecture '{}' is not supported by AWQ", arch);
                    return Ok(false);
                }
            }
        }

        // Fallback check
        let config_path = model_path.join("config.json");
        if config_path.exists() {
            debug!("Model has config.json, assuming AWQ compatibility");
            Ok(true)
        } else {
            warn!("No config.json found for AWQ compatibility check");
            Ok(false)
        }
    }

    async fn check_ggml_compatibility(&self, model_path: &Path) -> Result<bool> {
        // GGML is more restrictive, mainly for LLaMA models
        if let Ok(config) = self.read_model_config(model_path).await {
            if let Some(arch) = config.get("model_type").and_then(|v| v.as_str()) {
                if arch.contains("llama") {
                    debug!("Model architecture '{}' is compatible with GGML", arch);
                    return Ok(true);
                } else {
                    warn!("Model architecture '{}' is not supported by GGML", arch);
                    return Ok(false);
                }
            }
        }

        // Check for specific GGML model files
        let ggml_files = vec!["ggml-model.bin", "ggml-model-f16.bin"];

        for file in ggml_files {
            if model_path.join(file).exists() {
                debug!("Found GGML model file: {}", file);
                return Ok(false); // Already GGML format
            }
        }

        // Default to compatible for LLaMA models
        let config_path = model_path.join("config.json");
        if config_path.exists() {
            debug!("Model has config.json, checking for LLaMA architecture");
            Ok(true) // Assume compatible, detailed check would be done during quantization
        } else {
            warn!("No config.json found for GGML compatibility check");
            Ok(false)
        }
    }

    async fn read_model_config(&self, model_path: &Path) -> Result<serde_json::Value> {
        let config_path = model_path.join("config.json");
        let content = fs::read_to_string(&config_path)
            .await
            .map_err(FuseError::IoError)?;

        serde_json::from_str(&content).map_err(|e| FuseError::SerializationError(e.to_string()))
    }

    pub async fn get_compatibility_report(&self, model_path: &Path) -> Result<CompatibilityReport> {
        let mut compatible_methods = Vec::new();
        let mut incompatible_methods = Vec::new();

        let methods = vec![
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
        ];

        for method in methods {
            match self.check_compatibility(model_path, method).await {
                Ok(true) => compatible_methods.push(method),
                Ok(false) => incompatible_methods.push(method),
                Err(e) => {
                    warn!("Error checking compatibility for {:?}: {}", method, e);
                    incompatible_methods.push(method);
                }
            }
        }

        let recommended_method = self.recommend_best_method(&compatible_methods);

        Ok(CompatibilityReport {
            model_path: model_path.to_path_buf(),
            compatible_methods,
            incompatible_methods,
            recommended_method,
        })
    }

    fn recommend_best_method(
        &self,
        compatible_methods: &[QuantizationMethod],
    ) -> Option<QuantizationMethod> {
        if compatible_methods.is_empty() {
            return None;
        }

        // Prefer methods in this order: Q4_0, GPTQ, AWQ, Q5_0, Q8_0, GGML
        let preference_order = vec![
            QuantizationMethod::Q4_0,
            QuantizationMethod::GPTQ,
            QuantizationMethod::AWQ,
            QuantizationMethod::Q5_0,
            QuantizationMethod::Q8_0,
            QuantizationMethod::Q4_1,
            QuantizationMethod::Q5_1,
            QuantizationMethod::GGML,
        ];

        for preferred in preference_order {
            if compatible_methods.contains(&preferred) {
                return Some(preferred);
            }
        }

        // Fallback to first compatible method
        compatible_methods.first().cloned()
    }
}

#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    pub model_path: std::path::PathBuf,
    pub compatible_methods: Vec<QuantizationMethod>,
    pub incompatible_methods: Vec<QuantizationMethod>,
    pub recommended_method: Option<QuantizationMethod>,
}

impl CompatibilityReport {
    pub fn format(&self) -> String {
        let mut output = format!(
            "Quantization Compatibility Report for {}\n",
            self.model_path.display()
        );
        output.push_str("=".repeat(50).as_str());
        output.push('\n');

        output.push_str("\nCompatible Methods:\n");
        for method in &self.compatible_methods {
            output.push_str(&format!(
                "  ✓ {} - {}\n",
                method.as_str(),
                method.description()
            ));
        }

        if !self.incompatible_methods.is_empty() {
            output.push_str("\nIncompatible Methods:\n");
            for method in &self.incompatible_methods {
                output.push_str(&format!(
                    "  ✗ {} - {}\n",
                    method.as_str(),
                    method.description()
                ));
            }
        }

        if let Some(recommended) = &self.recommended_method {
            output.push_str(&format!(
                "\nRecommended Method: {} - {}\n",
                recommended.as_str(),
                recommended.description()
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_compatibility_checker_creation() {
        let checker = QuantizationCompatibility::new();
        assert!(checker.compatibility_cache.is_empty());
    }

    #[tokio::test]
    async fn test_gguf_compatibility() {
        let temp_dir = TempDir::new().unwrap();
        let checker = QuantizationCompatibility::new();

        // Test with non-existent path
        let result = checker
            .check_gguf_compatibility(temp_dir.path())
            .await
            .unwrap();
        assert!(!result);

        // Test with pytorch model file
        let model_file = temp_dir.path().join("pytorch_model.bin");
        fs::write(&model_file, "dummy model data").unwrap();

        let result = checker
            .check_gguf_compatibility(temp_dir.path())
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_compatibility_report() {
        let temp_dir = TempDir::new().unwrap();
        let checker = QuantizationCompatibility::new();

        // Create a basic model directory
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, r#"{"model_type": "llama"}"#).unwrap();

        let report = checker
            .get_compatibility_report(temp_dir.path())
            .await
            .unwrap();

        assert!(!report.compatible_methods.is_empty());
        assert!(report.recommended_method.is_some());
    }
}
