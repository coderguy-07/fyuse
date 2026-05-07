//! InferenceBackend trait — the core abstraction for running AI models.

use crate::error::Result;
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Unique handle to a loaded model instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelHandle {
    pub id: String,
    pub model_name: String,
}

/// Information about an inference backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendInfo {
    pub name: String,
    pub backend_type: BackendType,
    pub supports_streaming: bool,
    pub supports_embeddings: bool,
    pub max_batch_size: usize,
}

/// The type of backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    CpuSimd,
    Metal,
    Cuda,
    Vulkan,
    Remote,
}

/// Configuration for loading a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub context_length: usize,
    pub batch_size: usize,
    pub threads: Option<usize>,
    pub gpu_layers: Option<usize>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            context_length: 2048,
            batch_size: 1,
            threads: None,
            gpu_layers: None,
        }
    }
}

/// A request for inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: usize,
    pub stop_sequences: Vec<String>,
}

impl Default for InferenceRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            stop_sequences: vec![],
        }
    }
}

/// A completed inference response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub text: String,
    pub tokens_generated: usize,
    pub tokens_per_second: f64,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    MaxTokens,
    StopSequence,
    EndOfSequence,
}

/// A single generated token (for streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    pub id: u32,
    pub logprob: Option<f32>,
}

/// Resource usage snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    pub loaded_models: usize,
}

/// Core trait for all inference backends (CPU, Metal, CUDA, Remote).
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Backend metadata.
    fn info(&self) -> BackendInfo;

    /// Load a model from disk.
    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle>;

    /// Unload a model, freeing resources.
    async fn unload_model(&self, handle: &ModelHandle) -> Result<()>;

    /// Run inference (non-streaming).
    async fn infer(&self, handle: &ModelHandle, req: InferenceRequest)
        -> Result<InferenceResponse>;

    /// Stream tokens as they are generated.
    fn stream(&self, handle: &ModelHandle, req: InferenceRequest) -> BoxStream<'_, Result<Token>>;

    /// Generate embeddings for texts.
    async fn embed(&self, handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Current resource usage.
    fn resource_usage(&self) -> ResourceUsage;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// Mock backend for testing.
    struct MockBackend {
        info: BackendInfo,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                info: BackendInfo {
                    name: "mock".to_string(),
                    backend_type: BackendType::CpuSimd,
                    supports_streaming: true,
                    supports_embeddings: true,
                    max_batch_size: 32,
                },
            }
        }
    }

    #[async_trait]
    impl InferenceBackend for MockBackend {
        fn info(&self) -> BackendInfo {
            self.info.clone()
        }

        async fn load_model(&self, _path: &Path, _config: &ModelConfig) -> Result<ModelHandle> {
            Ok(ModelHandle {
                id: "mock-handle".to_string(),
                model_name: "test-model".to_string(),
            })
        }

        async fn unload_model(&self, _handle: &ModelHandle) -> Result<()> {
            Ok(())
        }

        async fn infer(
            &self,
            _handle: &ModelHandle,
            req: InferenceRequest,
        ) -> Result<InferenceResponse> {
            Ok(InferenceResponse {
                text: format!("Response to: {}", req.prompt),
                tokens_generated: 5,
                tokens_per_second: 100.0,
                stop_reason: StopReason::EndOfSequence,
            })
        }

        fn stream(
            &self,
            _handle: &ModelHandle,
            _req: InferenceRequest,
        ) -> BoxStream<'_, Result<Token>> {
            let tokens = vec![
                Ok(Token {
                    text: "hello".to_string(),
                    id: 1,
                    logprob: None,
                }),
                Ok(Token {
                    text: " world".to_string(),
                    id: 2,
                    logprob: None,
                }),
            ];
            Box::pin(futures::stream::iter(tokens))
        }

        async fn embed(&self, _handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }

        fn resource_usage(&self) -> ResourceUsage {
            ResourceUsage::default()
        }
    }

    #[test]
    fn test_backend_info() {
        let backend = MockBackend::new();
        let info = backend.info();
        assert_eq!(info.name, "mock");
        assert_eq!(info.backend_type, BackendType::CpuSimd);
        assert!(info.supports_streaming);
    }

    #[tokio::test]
    async fn test_load_and_infer() {
        let backend = MockBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/model"), &ModelConfig::default())
            .await
            .unwrap();
        assert_eq!(handle.model_name, "test-model");

        let response = backend
            .infer(
                &handle,
                InferenceRequest {
                    prompt: "test".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(response.text.contains("test"));
        assert_eq!(response.stop_reason, StopReason::EndOfSequence);
    }

    #[tokio::test]
    async fn test_streaming() {
        let backend = MockBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/model"), &ModelConfig::default())
            .await
            .unwrap();

        let mut stream = backend.stream(&handle, InferenceRequest::default());
        let mut tokens = vec![];
        while let Some(token) = stream.next().await {
            tokens.push(token.unwrap());
        }
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "hello");
        assert_eq!(tokens[1].text, " world");
    }

    #[tokio::test]
    async fn test_embeddings() {
        let backend = MockBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/model"), &ModelConfig::default())
            .await
            .unwrap();

        let embeddings = backend
            .embed(&handle, &["hello".to_string(), "world".to_string()])
            .await
            .unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 3);
    }

    #[tokio::test]
    async fn test_unload_model() {
        let backend = MockBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/model"), &ModelConfig::default())
            .await
            .unwrap();
        assert!(backend.unload_model(&handle).await.is_ok());
    }
}
