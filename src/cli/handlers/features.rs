use crate::cli::commands::FeatureArgs;
use crate::config::{
    feature_flags::{Feature, FeatureFlagManager},
    FuseConfig,
};
use crate::error::Result;

pub fn handle(
    args: FeatureArgs,
    feature_manager: FeatureFlagManager,
    mut config: FuseConfig,
) -> Result<()> {
    match args {
        FeatureArgs::List => {
            list_features(&feature_manager);
        }
        FeatureArgs::Enable { feature } => {
            enable_feature(&feature, &feature_manager, &mut config)?;
        }
        FeatureArgs::Disable { feature } => {
            disable_feature(&feature, &feature_manager, &mut config)?;
        }
    }

    Ok(())
}

fn list_features(feature_manager: &FeatureFlagManager) {
    println!("\nAvailable Features:");
    println!("{:<30} {:<10} Description", "Feature", "Status");
    println!("{}", "-".repeat(80));

    for feature in Feature::all() {
        let status = if feature_manager.is_enabled(feature) {
            "✓ enabled"
        } else {
            "✗ disabled"
        };
        println!(
            "{:<30} {:<10} {}",
            feature.name(),
            status,
            feature.description()
        );
    }
}

fn enable_feature(
    feature_name: &str,
    feature_manager: &FeatureFlagManager,
    config: &mut FuseConfig,
) -> Result<()> {
    if let Some(feature) = Feature::parse_feature(feature_name) {
        feature_manager.enable(feature);
        config.feature_flags.enable(feature);
        config.to_file(&FuseConfig::default_config_path())?;
        println!("✓ Feature '{}' enabled", feature.name());
    } else {
        eprintln!("Unknown feature: {}", feature_name);
        eprintln!(
            "Available features: {}",
            Feature::all()
                .iter()
                .map(|f| f.name())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    }

    Ok(())
}

fn disable_feature(
    feature_name: &str,
    feature_manager: &FeatureFlagManager,
    config: &mut FuseConfig,
) -> Result<()> {
    if let Some(feature) = Feature::parse_feature(feature_name) {
        feature_manager.disable(feature);
        config.feature_flags.disable(feature);
        config.to_file(&FuseConfig::default_config_path())?;
        println!("✓ Feature '{}' disabled", feature.name());
    } else {
        eprintln!("Unknown feature: {}", feature_name);
        eprintln!(
            "Available features: {}",
            Feature::all()
                .iter()
                .map(|f| f.name())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    }

    Ok(())
}
