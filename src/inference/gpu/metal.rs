//! Metal inference backend — GPU acceleration on Apple Silicon.

use crate::error::{FuseError, Result};
use crate::inference::backend::{
    BackendInfo, BackendType, InferenceBackend, InferenceRequest, InferenceResponse, ModelConfig,
    ModelHandle, ResourceUsage, StopReason, Token,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Configuration for the Metal backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetalConfig {
    /// Number of layers to offload to GPU.
    pub gpu_layers: usize,
    /// Maximum GPU memory to use in bytes (0 = unlimited).
    pub memory_limit: u64,
}

impl Default for MetalConfig {
    fn default() -> Self {
        Self {
            gpu_layers: 999,
            memory_limit: 0,
        }
    }
}

/// State for a loaded model on Metal GPU.
struct LoadedModel {
    _path: PathBuf,
    _config: ModelConfig,
    _metal_config: MetalConfig,
}

/// Metal inference backend using candle's Metal support.
pub struct MetalInferenceBackend {
    models: Arc<RwLock<HashMap<String, LoadedModel>>>,
    metal_config: MetalConfig,
    metal_available: bool,
}

impl MetalInferenceBackend {
    /// Create a new Metal backend. Falls back to CPU mode if Metal is unavailable.
    pub fn new(config: MetalConfig) -> Self {
        let metal_available = Self::check_metal_available();
        if !metal_available {
            tracing::warn!("Metal GPU not available, will fall back to CPU");
        }
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            metal_config: config,
            metal_available,
        }
    }

    /// Check if Metal GPU is available on this system.
    fn check_metal_available() -> bool {
        // Use candle's Metal device check
        candle_core::Device::new_metal(0).is_ok()
    }

    /// Returns whether Metal is actually being used.
    pub fn is_metal_active(&self) -> bool {
        self.metal_available
    }
}

#[async_trait]
impl InferenceBackend for MetalInferenceBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: if self.metal_available {
                "metal-candle".to_string()
            } else {
                "metal-candle (cpu-fallback)".to_string()
            },
            backend_type: BackendType::Metal,
            supports_streaming: true,
            supports_embeddings: true,
            max_batch_size: 1,
        }
    }

    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle> {
        let handle_id = uuid::Uuid::new_v4().to_string();
        let model_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model")
            .to_string();

        let loaded = LoadedModel {
            _path: path.to_path_buf(),
            _config: config.clone(),
            _metal_config: self.metal_config.clone(),
        };

        self.models.write().insert(handle_id.clone(), loaded);

        Ok(ModelHandle {
            id: handle_id,
            model_name,
        })
    }

    async fn unload_model(&self, handle: &ModelHandle) -> Result<()> {
        self.models
            .write()
            .remove(&handle.id)
            .ok_or_else(|| FuseError::ModelNotFound(handle.model_name.clone()))?;
        Ok(())
    }

    async fn infer(
        &self,
        handle: &ModelHandle,
        _req: InferenceRequest,
    ) -> Result<InferenceResponse> {
        // Verify model is loaded
        let models = self.models.read();
        if !models.contains_key(&handle.id) {
            return Err(FuseError::ModelNotFound(handle.model_name.clone()));
        }
        drop(models);

        // TODO: Implement actual Metal inference via candle
        Ok(InferenceResponse {
            text: "[metal placeholder]".to_string(),
            tokens_generated: 0,
            tokens_per_second: 0.0,
            stop_reason: StopReason::EndOfSequence,
        })
    }

    fn stream(
        &self,
        _handle: &ModelHandle,
        _req: InferenceRequest,
    ) -> BoxStream<'_, Result<Token>> {
        // TODO: Implement streaming with Metal
        Box::pin(futures::stream::empty())
    }

    async fn embed(&self, _handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Placeholder
        Ok(texts.iter().map(|_| vec![0.0; 384]).collect())
    }

    fn resource_usage(&self) -> ResourceUsage {
        let models = self.models.read();
        ResourceUsage {
            ram_bytes: 0,
            vram_bytes: 0, // TODO: query Metal memory usage
            loaded_models: models.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_backend() -> MetalInferenceBackend {
        MetalInferenceBackend {
            models: Arc::new(RwLock::new(HashMap::new())),
            metal_config: MetalConfig::default(),
            // In tests, don't require actual Metal hardware
            metal_available: false,
        }
    }

    #[test]
    fn test_metal_backend_info() {
        let backend = make_backend();
        let info = backend.info();
        assert_eq!(info.backend_type, BackendType::Metal);
        assert!(info.name.contains("metal"));
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_metal_config_default() {
        let config = MetalConfig::default();
        assert_eq!(config.gpu_layers, 999);
        assert_eq!(config.memory_limit, 0);
    }

    #[tokio::test]
    async fn test_load_and_unload() {
        let backend = make_backend();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .expect("load should succeed");

        assert_eq!(handle.model_name, "test-model");
        assert_eq!(backend.resource_usage().loaded_models, 1);

        backend
            .unload_model(&handle)
            .await
            .expect("unload should succeed");
        assert_eq!(backend.resource_usage().loaded_models, 0);
    }

    #[tokio::test]
    async fn test_unload_nonexistent() {
        let backend = make_backend();
        let handle = ModelHandle {
            id: "nonexistent".to_string(),
            model_name: "nope".to_string(),
        };
        assert!(backend.unload_model(&handle).await.is_err());
    }

    #[tokio::test]
    async fn test_resource_usage() {
        let backend = make_backend();
        let usage = backend.resource_usage();
        assert_eq!(usage.loaded_models, 0);
        assert_eq!(usage.vram_bytes, 0);
    }

    #[test]
    fn test_fallback_mode() {
        let backend = make_backend();
        assert!(!backend.is_metal_active());
        // Info should indicate fallback
        assert!(backend.info().name.contains("cpu-fallback"));
    }
}
