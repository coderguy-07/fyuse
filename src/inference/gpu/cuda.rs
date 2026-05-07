//! CUDA inference backend — GPU acceleration on NVIDIA hardware.

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

/// Configuration for the CUDA backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaConfig {
    /// CUDA device ordinal (0, 1, 2, ...).
    pub device_id: usize,
    /// Number of layers to offload to GPU.
    pub gpu_layers: usize,
    /// Fraction of GPU memory to use (0.0..=1.0).
    pub memory_fraction: f64,
}

impl Default for CudaConfig {
    fn default() -> Self {
        Self {
            device_id: 0,
            gpu_layers: 999,
            memory_fraction: 0.9,
        }
    }
}

/// State for a loaded model on CUDA GPU.
struct LoadedModel {
    _path: PathBuf,
    _config: ModelConfig,
    _cuda_config: CudaConfig,
}

/// CUDA inference backend using candle's CUDA support.
pub struct CudaInferenceBackend {
    models: Arc<RwLock<HashMap<String, LoadedModel>>>,
    cuda_config: CudaConfig,
    cuda_available: bool,
}

impl CudaInferenceBackend {
    /// Create a new CUDA backend.
    pub fn new(config: CudaConfig) -> Self {
        let cuda_available = Self::check_cuda_available(config.device_id);
        if !cuda_available {
            tracing::warn!("CUDA device {} not available", config.device_id);
        }
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            cuda_config: config,
            cuda_available,
        }
    }

    /// Check if CUDA is available for the given device.
    fn check_cuda_available(device_id: usize) -> bool {
        candle_core::Device::new_cuda(device_id).is_ok()
    }

    /// Returns whether CUDA is actually being used.
    pub fn is_cuda_active(&self) -> bool {
        self.cuda_available
    }
}

#[async_trait]
impl InferenceBackend for CudaInferenceBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: format!(
                "cuda-candle:{}{}",
                self.cuda_config.device_id,
                if self.cuda_available {
                    ""
                } else {
                    " (unavailable)"
                }
            ),
            backend_type: BackendType::Cuda,
            supports_streaming: true,
            supports_embeddings: true,
            max_batch_size: 8,
        }
    }

    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle> {
        if !self.cuda_available {
            return Err(FuseError::ResourceUnavailable(
                "CUDA device not available".to_string(),
            ));
        }

        let handle_id = uuid::Uuid::new_v4().to_string();
        let model_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model")
            .to_string();

        let loaded = LoadedModel {
            _path: path.to_path_buf(),
            _config: config.clone(),
            _cuda_config: self.cuda_config.clone(),
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
        let models = self.models.read();
        if !models.contains_key(&handle.id) {
            return Err(FuseError::ModelNotFound(handle.model_name.clone()));
        }
        drop(models);

        // TODO: Implement actual CUDA inference via candle
        Ok(InferenceResponse {
            text: "[cuda placeholder]".to_string(),
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
        Box::pin(futures::stream::empty())
    }

    async fn embed(&self, _handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; 384]).collect())
    }

    fn resource_usage(&self) -> ResourceUsage {
        let models = self.models.read();
        ResourceUsage {
            ram_bytes: 0,
            vram_bytes: 0, // TODO: query CUDA memory via cudarc
            loaded_models: models.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_backend() -> CudaInferenceBackend {
        CudaInferenceBackend {
            models: Arc::new(RwLock::new(HashMap::new())),
            cuda_config: CudaConfig::default(),
            cuda_available: false,
        }
    }

    #[test]
    fn test_cuda_backend_info() {
        let backend = make_backend();
        let info = backend.info();
        assert_eq!(info.backend_type, BackendType::Cuda);
        assert!(info.name.contains("cuda"));
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_cuda_config_default() {
        let config = CudaConfig::default();
        assert_eq!(config.device_id, 0);
        assert_eq!(config.gpu_layers, 999);
        assert!((config.memory_fraction - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_load_fails_without_cuda() {
        let backend = make_backend();
        let result = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await;
        assert!(result.is_err());
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
    fn test_cuda_not_active_in_test() {
        let backend = make_backend();
        assert!(!backend.is_cuda_active());
    }
}
