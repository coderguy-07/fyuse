use crate::cli::commands::MergeModelsArgs;
use crate::config::FuseConfig;
use crate::error::{FuseError, Result};
use crate::model::{MergeConfig, MergeStrategy, ModelManager, ModelMerger, SlerpConfig};
use std::sync::Arc;
use tracing::info;

pub async fn handle_merge_models(args: MergeModelsArgs, config: &FuseConfig) -> Result<()> {
    info!("Starting model merge operation");

    // Validate arguments
    if args.models.len() < 2 {
        return Err(FuseError::ValidationError(
            "At least 2 models are required for merging".to_string(),
        ));
    }

    // Initialize model manager
    let db_path = config.data_dir.join("fuse.redb");
    let db = Arc::new(crate::storage::database::Database::new(db_path)?);
    let repository = Arc::new(crate::storage::ModelRepository::new(db));
    let model_manager = ModelManager::new(repository, config.models_dir.clone());

    // Load model metadata for all models
    let mut models_metadata = Vec::new();
    for model_name in &args.models {
        match model_manager.get_metadata(model_name).await? {
            Some(metadata) => models_metadata.push(metadata),
            None => {
                return Err(FuseError::ModelNotFound(format!(
                    "Model '{}' not found",
                    model_name
                )));
            }
        }
    }

    // Parse merge strategy
    let strategy = parse_merge_strategy(&args)?;

    // Create merge configuration
    let merge_config = MergeConfig {
        strategy,
        output_name: args.output.clone(),
        output_path: Some(config.models_dir.join(&args.output)),
        preserve_metadata: args.preserve_metadata,
        validation_enabled: args.validate,
    };

    // Initialize model merger
    let merger = ModelMerger::new(&config.models_dir);

    // Perform the merge
    info!(
        "Merging {} models using {} strategy",
        models_metadata.len(),
        args.strategy
    );
    let result = merger.merge_models(&models_metadata, &merge_config).await?;

    // Display results
    println!("✅ Model merge completed successfully!");
    println!("📁 Output: {}", result.output_path.display());
    println!("🎯 Strategy: {}", result.strategy_used.display_name());
    println!(
        "📊 Parameters: {}",
        format_parameter_count(result.total_parameters)
    );
    println!("💾 Size: {}", format_size(result.output_size_bytes));
    println!("⏱️  Duration: {} ms", result.merge_duration_ms);

    if let Some(validation_passed) = result.validation_passed {
        if validation_passed {
            println!("✅ Validation: Passed");
        } else {
            println!("❌ Validation: Failed");
        }
    }

    if !result.warnings.is_empty() {
        println!("\n⚠️  Warnings:");
        for warning in &result.warnings {
            println!("   • {}", warning);
        }
    }

    if !result.errors.is_empty() {
        println!("\n❌ Errors:");
        for error in &result.errors {
            println!("   • {}", error);
        }
    }

    println!("\n📋 Models merged:");
    for model_name in &result.models_merged {
        println!("   • {}", model_name);
    }

    Ok(())
}

fn parse_merge_strategy(args: &MergeModelsArgs) -> Result<MergeStrategy> {
    match args.strategy.to_lowercase().as_str() {
        "average" => Ok(MergeStrategy::Average),
        "weighted" => {
            if let Some(weights_str) = &args.weights {
                let weights: Vec<f32> = weights_str
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect::<std::result::Result<Vec<f32>, _>>()
                    .map_err(|_| {
                        FuseError::ValidationError(
                            "Invalid weights format. Expected comma-separated numbers.".to_string(),
                        )
                    })?;

                Ok(MergeStrategy::Weighted(weights))
            } else {
                Err(FuseError::ValidationError(
                    "Weighted merge requires --weights parameter".to_string(),
                ))
            }
        }
        "slerp" => {
            let t = args.slerp_t.unwrap_or(0.5);
            let base_model = args.base_model.unwrap_or(0);

            if !(0.0..=1.0).contains(&t) {
                return Err(FuseError::ValidationError(
                    "SLERP parameter t must be between 0.0 and 1.0".to_string(),
                ));
            }

            Ok(MergeStrategy::SLERP(SlerpConfig {
                t,
                base_model_index: base_model,
            }))
        }
        "custom" => Ok(MergeStrategy::Custom(args.strategy.clone())),
        _ => Err(FuseError::ValidationError(format!(
            "Unknown merge strategy: {}. Supported: average, weighted, slerp, custom",
            args.strategy
        ))),
    }
}

fn format_parameter_count(count: u64) -> String {
    if count >= 1_000_000_000 {
        format!("{:.1}B", count as f64 / 1_000_000_000.0)
    } else if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_average_strategy() {
        let args = MergeModelsArgs {
            models: vec!["model1".to_string(), "model2".to_string()],
            output: "merged".to_string(),
            strategy: "average".to_string(),
            weights: None,
            slerp_t: None,
            base_model: None,
            validate: true,
            preserve_metadata: true,
        };

        let strategy = parse_merge_strategy(&args).unwrap();
        assert!(matches!(strategy, MergeStrategy::Average));
    }

    #[test]
    fn test_parse_weighted_strategy() {
        let args = MergeModelsArgs {
            models: vec!["model1".to_string(), "model2".to_string()],
            output: "merged".to_string(),
            strategy: "weighted".to_string(),
            weights: Some("0.6,0.4".to_string()),
            slerp_t: None,
            base_model: None,
            validate: true,
            preserve_metadata: true,
        };

        let strategy = parse_merge_strategy(&args).unwrap();
        match strategy {
            MergeStrategy::Weighted(weights) => {
                assert_eq!(weights, vec![0.6, 0.4]);
            }
            _ => panic!("Expected Weighted strategy"),
        }
    }

    #[test]
    fn test_parse_slerp_strategy() {
        let args = MergeModelsArgs {
            models: vec!["model1".to_string(), "model2".to_string()],
            output: "merged".to_string(),
            strategy: "slerp".to_string(),
            weights: None,
            slerp_t: Some(0.7),
            base_model: Some(1),
            validate: true,
            preserve_metadata: true,
        };

        let strategy = parse_merge_strategy(&args).unwrap();
        match strategy {
            MergeStrategy::SLERP(config) => {
                assert_eq!(config.t, 0.7);
                assert_eq!(config.base_model_index, 1);
            }
            _ => panic!("Expected SLERP strategy"),
        }
    }

    #[test]
    fn test_format_parameter_count() {
        assert_eq!(format_parameter_count(500), "500");
        assert_eq!(format_parameter_count(1500), "1.5K");
        assert_eq!(format_parameter_count(2_500_000), "2.5M");
        assert_eq!(format_parameter_count(3_000_000_000), "3.0B");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512.0 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(2_500_000), "2.4 MB");
        assert_eq!(format_size(3_000_000_000), "2.8 GB");
    }
}
