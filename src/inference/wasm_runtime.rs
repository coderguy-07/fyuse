//! WASM inference runtime [8.2] — run AI models via WebAssembly.

use crate::error::{FuseError, Result};
use crate::inference::backend::{
    BackendInfo, BackendType, InferenceBackend, InferenceRequest, InferenceResponse, ModelConfig,
    ModelHandle, ResourceUsage, Token,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for the WASM inference backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Maximum memory in MB for the WASM module.
    pub memory_limit_mb: u64,
    /// Path to the WASM module file.
    pub module_path: String,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            memory_limit_mb: 256,
            module_path: String::new(),
        }
    }
}

/// WASM-based inference backend.
///
/// When the `wasm-runtime` feature is enabled, this uses wasmtime to execute
/// WASM inference modules. Without the feature, all operations return
/// `FeatureDisabled`.
pub struct WasmInferenceBackend {
    config: WasmConfig,
    #[cfg(feature = "wasm-runtime")]
    _engine: wasmtime::Engine,
}

impl WasmInferenceBackend {
    /// Create a new WASM inference backend.
    pub fn new(config: WasmConfig) -> Result<Self> {
        #[cfg(feature = "wasm-runtime")]
        {
            let mut wasm_config = wasmtime::Config::new();
            wasm_config.wasm_memory64(false);
            let engine = wasmtime::Engine::new(&wasm_config).map_err(|e| {
                FuseError::InferenceError(format!("Failed to create WASM engine: {e}"))
            })?;
            Ok(Self {
                config,
                _engine: engine,
            })
        }

        #[cfg(not(feature = "wasm-runtime"))]
        {
            Ok(Self { config })
        }
    }

    fn feature_disabled_error() -> FuseError {
        FuseError::FeatureDisabled(
            "wasm-runtime feature is not enabled. Rebuild with --features wasm-runtime".to_string(),
        )
    }
}

#[async_trait]
impl InferenceBackend for WasmInferenceBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "wasm".to_string(),
            backend_type: BackendType::CpuSimd,
            supports_streaming: false,
            supports_embeddings: false,
            max_batch_size: 1,
        }
    }

    async fn load_model(&self, _path: &Path, _config: &ModelConfig) -> Result<ModelHandle> {
        #[cfg(feature = "wasm-runtime")]
        {
            // TODO: Load WASM module and instantiate
            Ok(ModelHandle {
                id: uuid::Uuid::new_v4().to_string(),
                model_name: _path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            })
        }

        #[cfg(not(feature = "wasm-runtime"))]
        Err(Self::feature_disabled_error())
    }

    async fn unload_model(&self, _handle: &ModelHandle) -> Result<()> {
        #[cfg(feature = "wasm-runtime")]
        {
            Ok(())
        }

        #[cfg(not(feature = "wasm-runtime"))]
        Err(Self::feature_disabled_error())
    }

    async fn infer(
        &self,
        _handle: &ModelHandle,
        _req: InferenceRequest,
    ) -> Result<InferenceResponse> {
        #[cfg(feature = "wasm-runtime")]
        {
            // TODO: Execute inference via WASM module
            Err(FuseError::InferenceError(
                "WASM inference not yet implemented".to_string(),
            ))
        }

        #[cfg(not(feature = "wasm-runtime"))]
        Err(Self::feature_disabled_error())
    }

    fn stream(
        &self,
        _handle: &ModelHandle,
        _req: InferenceRequest,
    ) -> BoxStream<'_, Result<Token>> {
        Box::pin(futures::stream::once(async {
            Err(Self::feature_disabled_error())
        }))
    }

    async fn embed(&self, _handle: &ModelHandle, _texts: &[String]) -> Result<Vec<Vec<f32>>> {
        #[cfg(feature = "wasm-runtime")]
        {
            Err(FuseError::InferenceError(
                "WASM embeddings not yet implemented".to_string(),
            ))
        }

        #[cfg(not(feature = "wasm-runtime"))]
        Err(Self::feature_disabled_error())
    }

    fn resource_usage(&self) -> ResourceUsage {
        ResourceUsage {
            ram_bytes: self.config.memory_limit_mb * 1024 * 1024,
            vram_bytes: 0,
            loaded_models: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.memory_limit_mb, 256);
        assert!(config.module_path.is_empty());
    }

    #[test]
    fn test_wasm_config_custom() {
        let config = WasmConfig {
            memory_limit_mb: 512,
            module_path: "/path/to/model.wasm".to_string(),
        };
        assert_eq!(config.memory_limit_mb, 512);
        assert_eq!(config.module_path, "/path/to/model.wasm");
    }

    #[test]
    fn test_wasm_backend_creation() {
        let config = WasmConfig::default();
        let backend = WasmInferenceBackend::new(config);
        // On default features (no wasm-runtime), this should still succeed
        assert!(backend.is_ok());
    }

    #[test]
    fn test_wasm_backend_info() {
        let config = WasmConfig::default();
        let backend = WasmInferenceBackend::new(config).expect("backend creation failed");
        let info = backend.info();
        assert_eq!(info.name, "wasm");
        assert_eq!(info.max_batch_size, 1);
        assert!(!info.supports_streaming);
    }

    #[test]
    fn test_wasm_resource_usage() {
        let config = WasmConfig {
            memory_limit_mb: 128,
            module_path: String::new(),
        };
        let backend = WasmInferenceBackend::new(config).expect("backend creation failed");
        let usage = backend.resource_usage();
        assert_eq!(usage.ram_bytes, 128 * 1024 * 1024);
        assert_eq!(usage.vram_bytes, 0);
    }

    #[tokio::test]
    async fn test_wasm_backend_load_without_feature() {
        let config = WasmConfig::default();
        let backend = WasmInferenceBackend::new(config).expect("backend creation failed");
        let result = backend
            .load_model(Path::new("/tmp/model.wasm"), &ModelConfig::default())
            .await;

        #[cfg(not(feature = "wasm-runtime"))]
        assert!(result.is_err());

        #[cfg(feature = "wasm-runtime")]
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wasm_backend_infer_without_feature() {
        let config = WasmConfig::default();
        let backend = WasmInferenceBackend::new(config).expect("backend creation failed");
        let handle = ModelHandle {
            id: "test".to_string(),
            model_name: "test".to_string(),
        };
        let result = backend.infer(&handle, InferenceRequest::default()).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_wasm_config_serialization() {
        let config = WasmConfig {
            memory_limit_mb: 512,
            module_path: "/model.wasm".to_string(),
        };
        let json = serde_json::to_string(&config).expect("serialize failed");
        let deserialized: WasmConfig = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.memory_limit_mb, config.memory_limit_mb);
        assert_eq!(deserialized.module_path, config.module_path);
    }
}
