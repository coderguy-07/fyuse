use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub id: String,
    pub name: String,
    pub layer_type: String,
    pub size_bytes: u64,
    pub parameters: u64,
    pub input_shape: Vec<usize>,
    pub output_shape: Vec<usize>,
    pub activation: Option<String>,
    pub trainable: bool,
    pub position: usize,
}

impl LayerInfo {
    pub fn format_size(&self) -> String {
        let size = self.size_bytes as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn format_parameters(&self) -> String {
        let params = self.parameters as f64;
        if params < 1000.0 {
            format!("{}", params)
        } else if params < 1_000_000.0 {
            format!("{:.2}K", params / 1000.0)
        } else if params < 1_000_000_000.0 {
            format!("{:.2}M", params / 1_000_000.0)
        } else {
            format!("{:.2}B", params / 1_000_000_000.0)
        }
    }
}

pub struct LayerInspector {}

impl LayerInspector {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn inspect(&self, model_path: &Path, wide: bool) -> Result<Vec<LayerInfo>> {
        info!("Inspecting layers for model: {}", model_path.display());

        if !model_path.exists() {
            return Err(FuseError::ModelNotFound(model_path.display().to_string()));
        }

        // TODO: Implement actual layer inspection using model loading libraries
        // For now, return mock data
        let layers = self.generate_mock_layers(wide);

        info!("Found {} layers", layers.len());
        Ok(layers)
    }

    fn generate_mock_layers(&self, _wide: bool) -> Vec<LayerInfo> {
        vec![
            LayerInfo {
                id: "layer_0".to_string(),
                name: "embedding".to_string(),
                layer_type: "Embedding".to_string(),
                size_bytes: 512_000_000,
                parameters: 128_000_000,
                input_shape: vec![1, 512],
                output_shape: vec![1, 512, 768],
                activation: None,
                trainable: true,
                position: 0,
            },
            LayerInfo {
                id: "layer_1".to_string(),
                name: "transformer_0".to_string(),
                layer_type: "TransformerBlock".to_string(),
                size_bytes: 1_024_000_000,
                parameters: 256_000_000,
                input_shape: vec![1, 512, 768],
                output_shape: vec![1, 512, 768],
                activation: Some("GELU".to_string()),
                trainable: true,
                position: 1,
            },
            LayerInfo {
                id: "layer_2".to_string(),
                name: "transformer_1".to_string(),
                layer_type: "TransformerBlock".to_string(),
                size_bytes: 1_024_000_000,
                parameters: 256_000_000,
                input_shape: vec![1, 512, 768],
                output_shape: vec![1, 512, 768],
                activation: Some("GELU".to_string()),
                trainable: true,
                position: 2,
            },
            LayerInfo {
                id: "layer_3".to_string(),
                name: "output_head".to_string(),
                layer_type: "Linear".to_string(),
                size_bytes: 256_000_000,
                parameters: 64_000_000,
                input_shape: vec![1, 512, 768],
                output_shape: vec![1, 512, 50257],
                activation: None,
                trainable: true,
                position: 3,
            },
        ]
    }
}

impl Default for LayerInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        let layer = LayerInfo {
            id: "test".to_string(),
            name: "test".to_string(),
            layer_type: "Linear".to_string(),
            size_bytes: 1_024_000_000,
            parameters: 1_000_000,
            input_shape: vec![1, 512],
            output_shape: vec![1, 512],
            activation: None,
            trainable: true,
            position: 0,
        };

        assert!(layer.format_size().contains("MB") || layer.format_size().contains("GB"));
    }

    #[test]
    fn test_format_parameters() {
        let layer = LayerInfo {
            id: "test".to_string(),
            name: "test".to_string(),
            layer_type: "Linear".to_string(),
            size_bytes: 1_024_000,
            parameters: 1_000_000,
            input_shape: vec![1, 512],
            output_shape: vec![1, 512],
            activation: None,
            trainable: true,
            position: 0,
        };

        assert!(layer.format_parameters().contains("M"));
    }
}
