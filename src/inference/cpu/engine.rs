//! CPU inference engine — runs transformer models using candle on CPU.

use crate::error::{FuseError, Result};
use crate::inference::backend::{
    BackendInfo, BackendType, InferenceBackend, InferenceRequest, InferenceResponse, ModelConfig,
    ModelHandle, ResourceUsage, StopReason, Token,
};
use crate::inference::cpu::kv_cache::KvCache;
use async_trait::async_trait;
use candle_core::{Device, Tensor};
use futures::stream::BoxStream;
use parking_lot::{Mutex as PLMutex, RwLock};
use rand::Rng as _;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// State for a loaded model on CPU.
struct LoadedModel {
    _path: PathBuf,
    _config: ModelConfig,
    kv_cache: KvCache,
    vocab_size: usize,
    _num_layers: usize,
    // In a full implementation, this would hold the actual model weights
    // via candle_transformers. For now, we store the device and basic info.
    device: Device,
}

/// CPU inference backend using candle.
pub struct CpuInferenceBackend {
    models: Arc<RwLock<HashMap<String, Arc<PLMutex<LoadedModel>>>>>,
    device: Device,
}

impl CpuInferenceBackend {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            device: Device::Cpu,
        }
    }

    /// Generate the next token logits given vocab size and device.
    /// In a full implementation, this runs the transformer forward pass.
    /// For now, returns random logits to enable testing the pipeline.
    fn forward_with_vocab(
        &self,
        vocab_size: usize,
        device: &Device,
        _kv_cache: &mut KvCache,
    ) -> Result<Tensor> {
        // Placeholder: return random logits of vocab_size
        // Real implementation would run candle transformer layers
        Tensor::randn(0f32, 1.0, (1, vocab_size), device)
            .map_err(|e| FuseError::InferenceError(format!("Forward pass failed: {}", e)))
    }

    /// Sample a token from logits using temperature.
    fn sample_token(&self, logits: &Tensor, temperature: f32) -> Result<u32> {
        let logits = logits
            .squeeze(0)
            .map_err(|e| FuseError::InferenceError(format!("Squeeze failed: {}", e)))?;

        if temperature == 0.0 {
            // Greedy: argmax
            let token = logits
                .argmax(0)
                .map_err(|e| FuseError::InferenceError(format!("Argmax failed: {}", e)))?;
            let token_id = token
                .to_scalar::<u32>()
                .map_err(|e| FuseError::InferenceError(format!("Scalar failed: {}", e)))?;
            Ok(token_id)
        } else {
            // Temperature sampling
            let scaled = (&logits / temperature as f64)
                .map_err(|e| FuseError::InferenceError(format!("Scale failed: {}", e)))?;
            let probs = candle_nn::ops::softmax(&scaled, 0)
                .map_err(|e| FuseError::InferenceError(format!("Softmax failed: {}", e)))?;
            let probs_vec: Vec<f32> = probs
                .to_vec1()
                .map_err(|e| FuseError::InferenceError(format!("To vec failed: {}", e)))?;

            // Weighted random sampling
            let r: f32 = rand::rng().random();
            let mut cumsum = 0.0;
            for (i, &p) in probs_vec.iter().enumerate() {
                cumsum += p;
                if cumsum >= r {
                    return Ok(i as u32);
                }
            }
            Ok((probs_vec.len() - 1) as u32)
        }
    }
}

impl Default for CpuInferenceBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for CpuInferenceBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "cpu-candle".to_string(),
            backend_type: BackendType::CpuSimd,
            supports_streaming: true,
            supports_embeddings: true,
            max_batch_size: 1, // Start with batch_size=1
        }
    }

    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle> {
        let handle_id = uuid::Uuid::new_v4().to_string();
        let model_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model")
            .to_string();

        // TODO: Actually load model weights via candle_transformers
        // For now, create a placeholder with reasonable defaults
        let num_layers = 12; // Will be read from model config
        let vocab_size = 32000; // Will be read from model config

        let loaded = LoadedModel {
            _path: path.to_path_buf(),
            _config: config.clone(),
            kv_cache: KvCache::new(num_layers),
            vocab_size,
            _num_layers: num_layers,
            device: self.device.clone(),
        };

        self.models
            .write()
            .insert(handle_id.clone(), Arc::new(PLMutex::new(loaded)));

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
        req: InferenceRequest,
    ) -> Result<InferenceResponse> {
        let start = Instant::now();
        // Run generation loop on a blocking thread
        let models = Arc::clone(&self.models);
        let handle_id = handle.id.clone();
        let max_tokens = req.max_tokens;
        let temperature = req.temperature;

        // Acquire map read lock briefly to get the per-model handle, then release.
        let model_arc = {
            let models_read = models.read();
            models_read
                .get(&handle_id)
                .cloned()
                .ok_or_else(|| FuseError::ModelNotFound(handle_id.clone()))?
        };

        let tokens = tokio::task::spawn_blocking(move || -> Result<Vec<u32>> {
            let mut model = model_arc.lock();
            let backend = CpuInferenceBackend::new();
            let mut tokens = Vec::new();
            let vocab_size = model.vocab_size;
            let device = model.device.clone();

            for _ in 0..max_tokens {
                let logits =
                    backend.forward_with_vocab(vocab_size, &device, &mut model.kv_cache)?;
                let token_id = backend.sample_token(&logits, temperature)?;
                tokens.push(token_id);

                // Check for EOS (token 2 is common EOS)
                if token_id == 2 {
                    break;
                }
            }

            Ok(tokens)
        })
        .await
        .map_err(|e| FuseError::InferenceError(format!("Task join error: {}", e)))??;

        let elapsed = start.elapsed().as_secs_f64();
        let tokens_per_second = if elapsed > 0.0 {
            tokens.len() as f64 / elapsed
        } else {
            0.0
        };

        let stop_reason = if tokens.last() == Some(&2) {
            StopReason::EndOfSequence
        } else {
            StopReason::MaxTokens
        };

        Ok(InferenceResponse {
            text: format!("[generated {} tokens]", tokens.len()),
            tokens_generated: tokens.len(),
            tokens_per_second,
            stop_reason,
        })
    }

    fn stream(&self, handle: &ModelHandle, req: InferenceRequest) -> BoxStream<'_, Result<Token>> {
        let models = Arc::clone(&self.models);
        let handle_id = handle.id.clone();
        let max_tokens = req.max_tokens;
        let temperature = req.temperature;

        Box::pin(async_stream::stream! {
            // Acquire map read lock briefly to get the per-model handle, then release.
            // The guard must be dropped before any yield/await, so separate the lookup
            // from the match that contains the yield.
            let model_arc_opt = {
                let models_read = models.read();
                models_read.get(&handle_id).cloned()
                // models_read dropped here
            };

            let model_arc = match model_arc_opt {
                Some(m) => m,
                None => {
                    yield Err(FuseError::ModelNotFound(handle_id.clone()));
                    return;
                }
            };

            for _i in 0..max_tokens {
                let model_ref = Arc::clone(&model_arc);

                let result = tokio::task::spawn_blocking(move || -> Result<u32> {
                    let mut model = model_ref.lock();
                    let backend = CpuInferenceBackend::new();
                    let vocab_size = model.vocab_size;
                    let device = model.device.clone();
                    let logits = backend.forward_with_vocab(vocab_size, &device, &mut model.kv_cache)?;
                    backend.sample_token(&logits, temperature)
                })
                .await
                .map_err(|e| FuseError::InferenceError(format!("Task error: {}", e)))?;

                match result {
                    Ok(token_id) => {
                        if token_id == 2 {
                            break;
                        }
                        yield Ok(Token {
                            text: format!("[tok{}]", token_id),
                            id: token_id,
                            logprob: None,
                        });
                    }
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                }
            }
        })
    }

    async fn embed(&self, _handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Placeholder embedding: generate random vectors
        // Real implementation would run the model's embedding layer
        let dim = 384; // Common embedding dimension
        Ok(texts
            .iter()
            .map(|_| {
                (0..dim)
                    .map(|_| rand::rng().random::<f32>() * 2.0 - 1.0)
                    .collect()
            })
            .collect())
    }

    fn resource_usage(&self) -> ResourceUsage {
        let models = self.models.read();
        ResourceUsage {
            ram_bytes: 0, // TODO: track actual memory usage
            vram_bytes: 0,
            loaded_models: models.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_cpu_backend_info() {
        let backend = CpuInferenceBackend::new();
        let info = backend.info();
        assert_eq!(info.name, "cpu-candle");
        assert_eq!(info.backend_type, BackendType::CpuSimd);
        assert!(info.supports_streaming);
    }

    #[tokio::test]
    async fn test_load_and_unload() {
        let backend = CpuInferenceBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .unwrap();

        assert_eq!(handle.model_name, "test-model");
        assert_eq!(backend.resource_usage().loaded_models, 1);

        backend.unload_model(&handle).await.unwrap();
        assert_eq!(backend.resource_usage().loaded_models, 0);
    }

    #[tokio::test]
    async fn test_generate_tokens() {
        let backend = CpuInferenceBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .unwrap();

        let response = backend
            .infer(
                &handle,
                InferenceRequest {
                    prompt: "Hello".to_string(),
                    max_tokens: 10,
                    temperature: 0.7,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(response.tokens_generated > 0);
        assert!(response.tokens_generated <= 10);
        assert!(response.tokens_per_second > 0.0);
    }

    #[tokio::test]
    async fn test_greedy_sampling() {
        let backend = CpuInferenceBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .unwrap();

        // Temperature 0 = greedy/deterministic
        let response = backend
            .infer(
                &handle,
                InferenceRequest {
                    prompt: "test".to_string(),
                    max_tokens: 5,
                    temperature: 0.0,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(response.tokens_generated > 0);
    }

    #[tokio::test]
    async fn test_streaming() {
        let backend = CpuInferenceBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .unwrap();

        let mut stream = backend.stream(
            &handle,
            InferenceRequest {
                max_tokens: 5,
                temperature: 0.7,
                ..Default::default()
            },
        );

        let mut count = 0;
        while let Some(token_result) = stream.next().await {
            let _token = token_result.unwrap();
            count += 1;
        }
        assert!(count > 0);
        assert!(count <= 5);
    }

    #[tokio::test]
    async fn test_embeddings() {
        let backend = CpuInferenceBackend::new();
        let handle = backend
            .load_model(Path::new("/tmp/test-model"), &ModelConfig::default())
            .await
            .unwrap();

        let embeddings = backend
            .embed(&handle, &["hello".to_string(), "world".to_string()])
            .await
            .unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[tokio::test]
    async fn test_unload_nonexistent() {
        let backend = CpuInferenceBackend::new();
        let handle = ModelHandle {
            id: "nonexistent".to_string(),
            model_name: "nope".to_string(),
        };
        assert!(backend.unload_model(&handle).await.is_err());
    }
}
