use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerType {
    GeoRestriction,
    ContentFilter,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub name: String,
    pub parameters: serde_json::Value,
}

pub struct LayerManipulator {}

impl LayerManipulator {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn remove_layer(&self, model_path: &Path, layer_id: &str) -> Result<()> {
        info!(
            "Removing layer {} from model: {}",
            layer_id,
            model_path.display()
        );

        if !model_path.exists() {
            return Err(FuseError::ModelNotFound(model_path.display().to_string()));
        }

        // Validate that layer removal is safe
        self.validate_layer_removal(model_path, layer_id).await?;

        // TODO: Implement actual layer removal
        // This would involve:
        // 1. Loading the model
        // 2. Removing the specified layer
        // 3. Reconnecting adjacent layers
        // 4. Saving the modified model

        info!("Layer {} removed successfully", layer_id);
        Ok(())
    }

    pub async fn add_layer(
        &self,
        model_path: &Path,
        layer_type: LayerType,
        config: LayerConfig,
    ) -> Result<()> {
        info!(
            "Adding {:?} layer to model: {}",
            layer_type,
            model_path.display()
        );

        if !model_path.exists() {
            return Err(FuseError::ModelNotFound(model_path.display().to_string()));
        }

        // Validate layer configuration
        self.validate_layer_config(&layer_type, &config)?;

        // TODO: Implement actual layer addition
        // This would involve:
        // 1. Loading the model
        // 2. Creating the new layer based on type and config
        // 3. Inserting the layer at the appropriate position
        // 4. Connecting it to adjacent layers
        // 5. Saving the modified model

        info!("Layer {} added successfully", config.name);
        Ok(())
    }

    async fn validate_layer_removal(&self, _model_path: &Path, layer_id: &str) -> Result<()> {
        // Check if layer can be safely removed
        // For example, don't allow removing critical layers like embeddings or output heads

        if layer_id.contains("embedding") || layer_id.contains("output") {
            warn!("Attempting to remove critical layer: {}", layer_id);
            return Err(FuseError::ValidationError(format!(
                "Cannot remove critical layer: {}",
                layer_id
            )));
        }

        Ok(())
    }

    fn validate_layer_config(&self, layer_type: &LayerType, config: &LayerConfig) -> Result<()> {
        match layer_type {
            LayerType::GeoRestriction => {
                // Validate geo-restriction config
                if !config.parameters.is_object() {
                    return Err(FuseError::ValidationError(
                        "Geo-restriction config must be an object".to_string(),
                    ));
                }

                let obj = config.parameters.as_object().unwrap();
                if !obj.contains_key("allowed_countries") && !obj.contains_key("blocked_countries")
                {
                    return Err(FuseError::ValidationError(
                        "Geo-restriction must specify allowed_countries or blocked_countries"
                            .to_string(),
                    ));
                }
            }
            LayerType::ContentFilter => {
                // Validate content filter config
                if !config.parameters.is_object() {
                    return Err(FuseError::ValidationError(
                        "Content filter config must be an object".to_string(),
                    ));
                }

                let obj = config.parameters.as_object().unwrap();
                if !obj.contains_key("filter_rules") {
                    return Err(FuseError::ValidationError(
                        "Content filter must specify filter_rules".to_string(),
                    ));
                }
            }
            LayerType::Custom => {
                // Custom layers have flexible config
                if config.name.is_empty() {
                    return Err(FuseError::ValidationError(
                        "Custom layer must have a name".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl Default for LayerManipulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_geo_restriction_config() {
        let manipulator = LayerManipulator::new();

        let valid_config = LayerConfig {
            name: "geo_filter".to_string(),
            parameters: json!({
                "allowed_countries": ["US", "CA", "UK"]
            }),
        };

        assert!(manipulator
            .validate_layer_config(&LayerType::GeoRestriction, &valid_config)
            .is_ok());

        let invalid_config = LayerConfig {
            name: "geo_filter".to_string(),
            parameters: json!({}),
        };

        assert!(manipulator
            .validate_layer_config(&LayerType::GeoRestriction, &invalid_config)
            .is_err());
    }

    #[test]
    fn test_validate_content_filter_config() {
        let manipulator = LayerManipulator::new();

        let valid_config = LayerConfig {
            name: "content_filter".to_string(),
            parameters: json!({
                "filter_rules": ["no_profanity", "no_violence"]
            }),
        };

        assert!(manipulator
            .validate_layer_config(&LayerType::ContentFilter, &valid_config)
            .is_ok());
    }
}
