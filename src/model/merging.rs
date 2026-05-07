use crate::error::{FuseError, Result};
use crate::model::ModelMetadata;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    Average,
    Weighted(Vec<f32>), // Weights for each model
    SLERP(SlerpConfig),
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlerpConfig {
    pub t: f32, // Interpolation parameter (0.0 to 1.0)
    pub base_model_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    pub strategy: MergeStrategy,
    pub output_name: String,
    pub output_path: Option<PathBuf>,
    pub preserve_metadata: bool,
    pub validation_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub success: bool,
    pub output_path: PathBuf,
    pub strategy_used: MergeStrategy,
    pub models_merged: Vec<String>,
    pub total_parameters: u64,
    pub output_size_bytes: u64,
    pub merge_duration_ms: u64,
    pub validation_passed: Option<bool>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub timestamp: chrono::DateTime<Utc>,
}

pub struct ModelMerger {
    workspace_dir: PathBuf,
}

impl ModelMerger {
    pub fn new(workspace_dir: impl AsRef<Path>) -> Self {
        Self {
            workspace_dir: workspace_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn merge_models(
        &self,
        models: &[ModelMetadata],
        config: &MergeConfig,
    ) -> Result<MergeResult> {
        if models.len() < 2 {
            return Err(FuseError::ValidationError(
                "At least 2 models are required for merging".to_string(),
            ));
        }

        let start_time = std::time::Instant::now();
        info!("Starting model merge with strategy: {:?}", config.strategy);

        // Validate merge compatibility
        self.validate_merge_compatibility(models, &config.strategy)
            .await?;

        // Determine output path
        let output_path = config
            .output_path
            .clone()
            .unwrap_or_else(|| self.workspace_dir.join("models").join(&config.output_name));

        // Create output directory
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Perform the merge based on strategy
        let result = match &config.strategy {
            MergeStrategy::Average => self.merge_average(models, &output_path).await?,
            MergeStrategy::Weighted(weights) => {
                self.merge_weighted(models, weights, &output_path).await?
            }
            MergeStrategy::SLERP(slerp_config) => {
                self.merge_slerp(models, slerp_config, &output_path).await?
            }
            MergeStrategy::Custom(strategy_name) => {
                self.merge_custom(models, strategy_name, &output_path)
                    .await?
            }
        };

        let merge_duration = start_time.elapsed().as_millis() as u64;

        // Validate result if enabled
        let validation_passed = if config.validation_enabled {
            Some(self.validate_merge_result(&output_path).await?)
        } else {
            None
        };

        // Create metadata for merged model
        if config.preserve_metadata {
            self.create_merged_metadata(
                models,
                &MergeResult {
                    success: true,
                    output_path: result.output_path.clone(),
                    strategy_used: config.strategy.clone(),
                    models_merged: models.iter().map(|m| m.name.clone()).collect(),
                    total_parameters: result.total_parameters,
                    output_size_bytes: result.output_size_bytes,
                    merge_duration_ms: merge_duration,
                    validation_passed: None,
                    warnings: result.warnings.clone(),
                    errors: result.errors.clone(),
                    timestamp: Utc::now(),
                },
                config,
            )
            .await?;
        }

        let final_result = MergeResult {
            success: true,
            output_path: result.output_path,
            strategy_used: config.strategy.clone(),
            models_merged: models.iter().map(|m| m.name.clone()).collect(),
            total_parameters: result.total_parameters,
            output_size_bytes: result.output_size_bytes,
            merge_duration_ms: merge_duration,
            validation_passed,
            warnings: result.warnings,
            errors: result.errors,
            timestamp: Utc::now(),
        };

        info!(
            "Model merge completed successfully in {} ms",
            merge_duration
        );
        Ok(final_result)
    }

    async fn validate_merge_compatibility(
        &self,
        models: &[ModelMetadata],
        strategy: &MergeStrategy,
    ) -> Result<()> {
        // Check basic requirements
        match strategy {
            MergeStrategy::SLERP(_) => {
                if models.len() != 2 {
                    return Err(FuseError::ValidationError(
                        "SLERP merging requires exactly 2 models".to_string(),
                    ));
                }
            }
            MergeStrategy::Weighted(weights) => {
                if weights.len() != models.len() {
                    return Err(FuseError::ValidationError(format!(
                        "Weighted merging requires {} weights but {} provided",
                        models.len(),
                        weights.len()
                    )));
                }

                let sum: f32 = weights.iter().sum();
                if (sum - 1.0).abs() > 0.01 {
                    return Err(FuseError::ValidationError(format!(
                        "Weights must sum to 1.0, got {}",
                        sum
                    )));
                }
            }
            _ => {}
        }

        // Check architecture compatibility
        let architectures: Vec<String> = models
            .iter()
            .map(|m| {
                m.architecture
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .collect();

        let all_same = architectures.windows(2).all(|w| w[0] == w[1]);
        if !all_same {
            warn!(
                "Merging models with different architectures: {:?}",
                architectures
            );
        }

        Ok(())
    }

    async fn merge_average(
        &self,
        models: &[ModelMetadata],
        output_path: &Path,
    ) -> Result<MergeIntermediateResult> {
        info!("Performing average merge of {} models", models.len());

        // Placeholder implementation - in real implementation, this would:
        // 1. Load model weights from each model
        // 2. Average corresponding parameters
        // 3. Save the merged model

        let warnings = Vec::new();
        let errors = Vec::new();

        // Simulate parameter averaging
        let total_params: u64 = models
            .iter()
            .map(|m| m.parameter_count.unwrap_or(0) as u64)
            .sum::<u64>();

        let avg_params = total_params / models.len() as u64;

        // Create a placeholder merged model file
        let merged_content = format!(
            "# Merged Model (Average)\n\
             # Merged from: {}\n\
             # Total parameters: {}\n\
             # Average parameters: {}\n\
             # Generated: {}\n",
            models
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            total_params,
            { avg_params },
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        fs::write(output_path, merged_content).await?;

        let metadata = fs::metadata(output_path).await?;
        let output_size = metadata.len();

        Ok(MergeIntermediateResult {
            output_path: output_path.to_path_buf(),
            total_parameters: avg_params,
            output_size_bytes: output_size,
            warnings,
            errors,
        })
    }

    async fn merge_weighted(
        &self,
        models: &[ModelMetadata],
        weights: &[f32],
        output_path: &Path,
    ) -> Result<MergeIntermediateResult> {
        info!("Performing weighted merge of {} models", models.len());

        let warnings = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // Validate weights
        for (i, &weight) in weights.iter().enumerate() {
            if !(0.0..=1.0).contains(&weight) {
                errors.push(format!(
                    "Invalid weight {} for model {}: must be between 0.0 and 1.0",
                    weight, models[i].name
                ));
            }
        }

        if !errors.is_empty() {
            return Err(FuseError::ValidationError(errors.join("; ")));
        }

        // Calculate weighted parameter count
        let weighted_params: u64 = models
            .iter()
            .zip(weights.iter())
            .map(|(model, &weight)| (model.parameter_count.unwrap_or(0) as f32 * weight) as u64)
            .sum();

        // Create merged model content
        let weight_info: Vec<String> = weights
            .iter()
            .zip(models.iter())
            .map(|(&w, m)| format!("{}: {:.2}", m.name, w))
            .collect();

        let merged_content = format!(
            "# Merged Model (Weighted)\n\
             # Weights: {}\n\
             # Weighted parameters: {}\n\
             # Generated: {}\n",
            weight_info.join(", "),
            weighted_params,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        fs::write(output_path, merged_content).await?;

        let metadata = fs::metadata(output_path).await?;
        let output_size = metadata.len();

        Ok(MergeIntermediateResult {
            output_path: output_path.to_path_buf(),
            total_parameters: weighted_params,
            output_size_bytes: output_size,
            warnings,
            errors,
        })
    }

    async fn merge_slerp(
        &self,
        models: &[ModelMetadata],
        config: &SlerpConfig,
        output_path: &Path,
    ) -> Result<MergeIntermediateResult> {
        if models.len() != 2 {
            return Err(FuseError::ValidationError(
                "SLERP merging requires exactly 2 models".to_string(),
            ));
        }

        info!("Performing SLERP merge with t={:.2}", config.t);

        let warnings: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // Validate SLERP parameters
        if config.t < 0.0 || config.t > 1.0 {
            errors.push(format!(
                "Invalid SLERP parameter t={}: must be between 0.0 and 1.0",
                config.t
            ));
        }

        if config.base_model_index >= models.len() {
            errors.push(format!(
                "Invalid base model index {}: must be < {}",
                config.base_model_index,
                models.len()
            ));
        }

        if !errors.is_empty() {
            return Err(FuseError::ValidationError(errors.join("; ")));
        }

        // Calculate interpolated parameter count
        let params1 = models[0].parameter_count.unwrap_or(0) as f32;
        let params2 = models[1].parameter_count.unwrap_or(0) as f32;
        let interpolated_params = ((1.0 - config.t) * params1 + config.t * params2) as u64;

        let merged_content = format!(
            "# Merged Model (SLERP)\n\
             # Model 1: {} (weight: {:.2})\n\
             # Model 2: {} (weight: {:.2})\n\
             # Interpolation parameter: {:.2}\n\
             # Base model: {}\n\
             # Interpolated parameters: {}\n\
             # Generated: {}\n",
            models[0].name,
            1.0 - config.t,
            models[1].name,
            config.t,
            config.t,
            models[config.base_model_index].name,
            interpolated_params,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        fs::write(output_path, merged_content).await?;

        let metadata = fs::metadata(output_path).await?;
        let output_size = metadata.len();

        Ok(MergeIntermediateResult {
            output_path: output_path.to_path_buf(),
            total_parameters: interpolated_params,
            output_size_bytes: output_size,
            warnings,
            errors,
        })
    }

    async fn merge_custom(
        &self,
        models: &[ModelMetadata],
        strategy_name: &str,
        output_path: &Path,
    ) -> Result<MergeIntermediateResult> {
        info!("Performing custom merge with strategy: {}", strategy_name);

        let mut warnings = vec![format!("Using custom merge strategy: {}", strategy_name)];
        let _errors: Vec<String> = Vec::new();

        // For custom strategies, we'd typically load a strategy configuration
        // and execute it. For now, fall back to average.
        warn!(
            "Custom merge strategy '{}' not implemented, falling back to average",
            strategy_name
        );
        warnings.push(format!(
            "Custom strategy '{}' not implemented, used average instead",
            strategy_name
        ));

        self.merge_average(models, output_path).await
    }

    async fn validate_merge_result(&self, output_path: &Path) -> Result<bool> {
        // Basic validation - check if file exists and has content
        if !output_path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(output_path).await?;
        if metadata.len() == 0 {
            return Ok(false);
        }

        // In a real implementation, this would:
        // 1. Load the merged model
        // 2. Check model architecture
        // 3. Validate parameter shapes
        // 4. Run basic inference test

        Ok(true)
    }

    async fn create_merged_metadata(
        &self,
        models: &[ModelMetadata],
        result: &MergeResult,
        config: &MergeConfig,
    ) -> Result<()> {
        let merged_metadata = ModelMetadata {
            id: format!("merged_{}", Utc::now().timestamp()),
            name: config.output_name.clone(),
            source: models[0].source.clone(), // Use first model's source
            version: "merged".to_string(),
            downloaded_at: Utc::now(),
            updated_at: Some(Utc::now()),
            size_bytes: result.output_size_bytes,
            architecture: models[0].architecture.clone(), // Assume same architecture
            parameter_count: Some(result.total_parameters as usize),
            quantization: None, // Merged models typically not quantized
            format: None,
            tags: {
                let mut tags = vec!["merged".to_string()];
                tags.extend(models.iter().flat_map(|m| m.tags.clone()));
                tags
            },
            custom_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "merge_strategy".to_string(),
                    serde_json::to_value(&config.strategy).unwrap_or(serde_json::Value::Null),
                );
                metadata.insert(
                    "source_models".to_string(),
                    serde_json::to_value(models.iter().map(|m| m.name.clone()).collect::<Vec<_>>())
                        .unwrap_or(serde_json::Value::Null),
                );
                metadata.insert(
                    "merge_timestamp".to_string(),
                    serde_json::Value::String(Utc::now().to_rfc3339()),
                );
                metadata
            },
            file_paths: vec![format!("{}.bin", config.output_name)], // Placeholder file path
            config_path: Some(format!("{}_config.json", config.output_name)),
            tokenizer_path: models[0].tokenizer_path.clone(), // Use first model's tokenizer
        };

        let metadata_path = result.output_path.with_extension("metadata.json");
        let json = serde_json::to_string_pretty(&merged_metadata)?;
        fs::write(&metadata_path, json).await?;

        debug!(
            "Created merged model metadata at: {}",
            metadata_path.display()
        );
        Ok(())
    }
}

#[derive(Debug)]
struct MergeIntermediateResult {
    output_path: PathBuf,
    total_parameters: u64,
    output_size_bytes: u64,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl MergeStrategy {
    pub fn display_name(&self) -> String {
        match self {
            MergeStrategy::Average => "Average - Simple parameter averaging".to_string(),
            MergeStrategy::Weighted(weights) => format!("Weighted - Custom weights: {:?}", weights),
            MergeStrategy::SLERP(config) => {
                format!("SLERP - Spherical interpolation (t={:.2})", config.t)
            }
            MergeStrategy::Custom(name) => format!("Custom - {}", name),
        }
    }

    pub fn validate(&self, model_count: usize) -> Result<()> {
        match self {
            MergeStrategy::SLERP(_) => {
                if model_count != 2 {
                    return Err(FuseError::ValidationError(
                        "SLERP merging requires exactly 2 models".to_string(),
                    ));
                }
            }
            MergeStrategy::Weighted(weights) => {
                if weights.len() != model_count {
                    return Err(FuseError::ValidationError(format!(
                        "Weighted merging requires {} weights but {} provided",
                        model_count,
                        weights.len()
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            strategy: MergeStrategy::Average,
            output_name: "merged_model".to_string(),
            output_path: None,
            preserve_metadata: true,
            validation_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_merge_strategy_validation() {
        let strategy = MergeStrategy::SLERP(SlerpConfig {
            t: 0.5,
            base_model_index: 0,
        });
        assert!(strategy.validate(2).is_ok());
        assert!(strategy.validate(3).is_err());

        let strategy = MergeStrategy::Weighted(vec![0.5, 0.5]);
        assert!(strategy.validate(2).is_ok());
        assert!(strategy.validate(3).is_err());
    }

    #[tokio::test]
    async fn test_average_merge() {
        let temp_dir = TempDir::new().unwrap();
        let merger = ModelMerger::new(temp_dir.path());

        let models = vec![
            ModelMetadata {
                id: "1".to_string(),
                name: "model1".to_string(),
                source: crate::model::ModelSource::huggingface("test1"),
                version: "1.0".to_string(),
                downloaded_at: Utc::now(),
                updated_at: None,
                size_bytes: 1000,
                architecture: Some("transformer".to_string()),
                parameter_count: Some(100_000_000),
                quantization: None,
                format: None,
                tags: vec![],
                custom_metadata: HashMap::new(),
                file_paths: vec![],
                config_path: None,
                tokenizer_path: None,
            },
            ModelMetadata {
                id: "2".to_string(),
                name: "model2".to_string(),
                source: crate::model::ModelSource::huggingface("test2"),
                version: "1.0".to_string(),
                downloaded_at: Utc::now(),
                updated_at: None,
                size_bytes: 1000,
                architecture: Some("transformer".to_string()),
                parameter_count: Some(200_000_000),
                quantization: None,
                format: None,
                tags: vec![],
                custom_metadata: HashMap::new(),
                file_paths: vec![],
                config_path: None,
                tokenizer_path: None,
            },
        ];

        let config = MergeConfig {
            strategy: MergeStrategy::Average,
            output_name: "merged_test".to_string(),
            output_path: Some(temp_dir.path().join("merged_test")),
            preserve_metadata: false,
            validation_enabled: false,
        };

        let result = merger.merge_models(&models, &config).await.unwrap();
        assert!(result.success);
        assert_eq!(result.models_merged, vec!["model1", "model2"]);
        assert_eq!(result.total_parameters, 150_000_000); // Average of 100M and 200M
    }
}
