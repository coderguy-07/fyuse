use crate::error::{FuseError, Result};
use crate::model::inference::{
    FinishReason, InferenceEngine, InferenceInput, InferenceMetadata, InferenceOutput, ModelConfig,
    ModelHandle, ModelInfo, ModelState, Token,
};
use crate::model::resource_manager::{ResourceManager, ResourcePolicy};
use crate::storage::ModelRepository;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Local inference engine implementation with intelligent resource management
pub struct LocalInferenceEngine {
    /// Repository for model metadata
    #[allow(dead_code)]
    model_repo: Arc<ModelRepository>,
    /// Cache of loaded models
    loaded_models: Arc<RwLock<HashMap<String, ModelHandle>>>,
    /// Models directory
    models_dir: PathBuf,
    /// Maximum number of models to keep loaded
    max_loaded_models: usize,
    /// Resource manager for intelligent lifecycle management
    resource_manager: Arc<ResourceManager>,
}

/// Standalone function for streaming inference (to avoid Send issues)
async fn perform_streaming_inference_task(
    model_name: &str,
    input: &InferenceInput,
    tx: tokio::sync::mpsc::Sender<Result<Token>>,
) -> Result<()> {
    // This is a placeholder implementation
    // In a real implementation, this would stream tokens as they're generated

    tracing::debug!(
        model = %model_name,
        prompt_length = input.prompt.len(),
        "Performing streaming inference"
    );

    // Simulate streaming tokens
    let response = format!(
        "Streaming response from model '{}' for prompt: '{}'",
        model_name,
        input.prompt.chars().take(30).collect::<String>()
    );

    let words: Vec<&str> = response.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        // Simulate token generation delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let is_final = i == words.len() - 1;
        let token_text = if is_final {
            word.to_string()
        } else {
            format!("{} ", word)
        };

        let token = Token {
            text: token_text,
            id: Some(i as u32),
            logprob: None,
            is_final,
        };

        if tx.send(Ok(token)).await.is_err() {
            // Receiver dropped, stop streaming
            break;
        }
    }

    Ok(())
}

impl LocalInferenceEngine {
    /// Create a new local inference engine with default resource policy
    pub fn new(model_repo: Arc<ModelRepository>, models_dir: PathBuf) -> Self {
        Self::with_resource_policy(model_repo, models_dir, ResourcePolicy::default())
    }

    /// Create with custom cache size and resource policy
    pub fn with_cache_size(
        model_repo: Arc<ModelRepository>,
        models_dir: PathBuf,
        max_loaded_models: usize,
    ) -> Self {
        let policy = ResourcePolicy {
            max_loaded_models,
            ..ResourcePolicy::default()
        };
        Self::with_resource_policy(model_repo, models_dir, policy)
    }

    /// Create with custom resource policy (config-driven)
    pub fn with_resource_policy(
        model_repo: Arc<ModelRepository>,
        models_dir: PathBuf,
        policy: ResourcePolicy,
    ) -> Self {
        let max_loaded_models = policy.max_loaded_models;
        let resource_manager = Arc::new(ResourceManager::new(policy));

        Self {
            model_repo,
            loaded_models: Arc::new(RwLock::new(HashMap::new())),
            models_dir,
            max_loaded_models,
            resource_manager,
        }
    }

    /// Get resource manager for external monitoring
    pub fn resource_manager(&self) -> Arc<ResourceManager> {
        self.resource_manager.clone()
    }

    /// Get the path to a model's directory
    fn get_model_path(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name)
    }

    /// Check if model exists on disk
    async fn model_exists(&self, model_name: &str) -> Result<bool> {
        let model_path = self.get_model_path(model_name);
        Ok(model_path.exists())
    }

    /// Load model configuration from disk
    async fn load_model_config(&self, model_name: &str) -> Result<ModelConfig> {
        let model_path = self.get_model_path(model_name);
        let config_path = model_path.join("config.json");

        if !config_path.exists() {
            // Return default config if no config file exists
            return Ok(ModelConfig {
                max_context_length: 4096,
                architecture: "unknown".to_string(),
                extra: serde_json::json!({}),
            });
        }

        let config_data = tokio::fs::read_to_string(&config_path).await?;
        let config: ModelConfig = serde_json::from_str(&config_data)?;
        Ok(config)
    }

    /// Evict least recently used model if cache is full
    async fn evict_if_needed(&self) -> Result<()> {
        let mut loaded = self.loaded_models.write();

        if loaded.len() >= self.max_loaded_models {
            // Find the oldest loaded model
            if let Some((oldest_name, _)) = loaded
                .iter()
                .min_by_key(|(_, handle)| handle.loaded_at)
                .map(|(name, handle)| (name.clone(), handle.clone()))
            {
                tracing::info!(
                    model = %oldest_name,
                    "Evicting model from cache to make room"
                );
                loaded.remove(&oldest_name);
            }
        }

        Ok(())
    }

    /// Get a loaded model from cache
    fn get_cached_model(&self, model_name: &str) -> Option<ModelHandle> {
        let loaded = self.loaded_models.read();
        loaded.get(model_name).cloned()
    }

    /// Add model to cache
    fn cache_model(&self, model_name: String, handle: ModelHandle) {
        let mut loaded = self.loaded_models.write();
        loaded.insert(model_name, handle);
    }

    /// Remove model from cache
    fn remove_from_cache(&self, model_name: &str) -> Option<ModelHandle> {
        let mut loaded = self.loaded_models.write();
        loaded.remove(model_name)
    }

    /// Estimate memory usage for a model (placeholder implementation)
    fn estimate_memory_usage(&self, model_path: &PathBuf) -> u64 {
        // In a real implementation, this would calculate based on model size
        // For now, return a placeholder value
        let mut total_size = 0u64;

        if let Ok(entries) = std::fs::read_dir(model_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                }
            }
        }

        total_size
    }
}

#[async_trait::async_trait]
impl InferenceEngine for LocalInferenceEngine {
    async fn load_model(&self, model_name: &str) -> Result<ModelHandle> {
        tracing::info!(model = %model_name, "Loading model");

        // Check if model is already loaded
        if let Some(handle) = self.get_cached_model(model_name) {
            tracing::debug!(model = %model_name, "Model already loaded, returning cached handle");
            // Mark as active in resource manager
            self.resource_manager.mark_active(model_name);
            return Ok(handle);
        }

        // Check if model exists
        if !self.model_exists(model_name).await? {
            return Err(FuseError::ModelNotFound(model_name.to_string()));
        }

        // Check resource limits and optimize if needed
        if self.resource_manager.is_over_limit() {
            tracing::info!("Resource limits exceeded, optimizing idle models");
            self.resource_manager.optimize_idle_models().await?;
            self.resource_manager.enforce_limits().await?;
        }

        // Evict old models if cache is full
        self.evict_if_needed().await?;

        // Load model configuration
        let config = self.load_model_config(model_name).await?;
        let model_path = self.get_model_path(model_name);

        // Estimate memory usage
        let memory_bytes = self.estimate_memory_usage(&model_path);

        // Create model state
        let state = ModelState {
            model_path: model_path.clone(),
            config: config.clone(),
            is_busy: false,
        };

        // Create model handle
        let handle_id = Uuid::new_v4().to_string();
        let handle = ModelHandle::new(handle_id, model_name.to_string(), state);

        // Cache the model
        self.cache_model(model_name.to_string(), handle.clone());

        // Register with resource manager
        self.resource_manager
            .register_model(model_name.to_string(), handle.clone(), memory_bytes);

        tracing::info!(
            model = %model_name,
            handle_id = %handle.id,
            memory_mb = memory_bytes / (1024 * 1024),
            "Model loaded successfully"
        );

        Ok(handle)
    }

    async fn unload_model(&self, handle: ModelHandle) -> Result<()> {
        tracing::info!(
            model = %handle.model_name,
            handle_id = %handle.id,
            "Unloading model"
        );

        // Unregister from resource manager
        self.resource_manager.unregister_model(&handle.model_name);

        // Remove from cache
        if let Some(_removed) = self.remove_from_cache(&handle.model_name) {
            tracing::info!(
                model = %handle.model_name,
                "Model unloaded successfully, resources freed"
            );
            Ok(())
        } else {
            tracing::warn!(
                model = %handle.model_name,
                "Model was not in cache"
            );
            Ok(())
        }
    }

    async fn infer(&self, handle: &ModelHandle, input: InferenceInput) -> Result<InferenceOutput> {
        // Validate parameters
        input.parameters.validate()?;

        // Mark as active in resource manager
        self.resource_manager.mark_active(&handle.model_name);

        // Check if model is busy
        {
            let state = handle.state().await;
            if state.is_busy {
                self.resource_manager
                    .mark_request_complete(&handle.model_name);
                return Err(FuseError::InferenceError(
                    "Model is currently busy processing another request".to_string(),
                ));
            }
        }

        // Mark model as busy
        {
            let mut state = handle.state_mut().await;
            state.is_busy = true;
        }

        let start_time = std::time::Instant::now();

        // Perform inference (placeholder implementation)
        // In a real implementation, this would call the actual model
        let result = self.perform_inference_internal(handle, &input).await;

        // Mark model as not busy
        {
            let mut state = handle.state_mut().await;
            state.is_busy = false;
        }

        // Mark request complete in resource manager
        self.resource_manager
            .mark_request_complete(&handle.model_name);

        let inference_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(text) => {
                // Format as markdown
                let formatted_text = self.format_as_markdown(&text);

                // Calculate token counts (placeholder)
                let prompt_tokens = self.estimate_token_count(&input.prompt);
                let completion_tokens = self.estimate_token_count(&text);

                Ok(InferenceOutput {
                    text: text.clone(),
                    formatted_text,
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                    model: handle.model_name.clone(),
                    timestamp: Utc::now(),
                    metadata: Some(InferenceMetadata {
                        inference_time_ms,
                        finish_reason: FinishReason::Stop,
                        extra: serde_json::json!({}),
                    }),
                })
            }
            Err(e) => Err(e),
        }
    }

    async fn infer_stream(
        &self,
        handle: &ModelHandle,
        input: InferenceInput,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<Token>>> {
        // Validate parameters
        input.parameters.validate()?;

        // Check if model is busy
        {
            let state = handle.state().await;
            if state.is_busy {
                return Err(FuseError::InferenceError(
                    "Model is currently busy processing another request".to_string(),
                ));
            }
        }

        // Mark model as busy
        {
            let mut state = handle.state_mut().await;
            state.is_busy = true;
        }

        // Create channel for streaming tokens
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Clone necessary data for the spawned task
        let handle_clone = handle.clone();
        let model_name = handle.model_name.clone();

        // Spawn task to perform streaming inference
        tokio::spawn(async move {
            let result = perform_streaming_inference_task(&model_name, &input, tx.clone()).await;

            // Mark model as not busy
            {
                let mut state = handle_clone.state_mut().await;
                state.is_busy = false;
            }

            if let Err(e) = result {
                let _ = tx.send(Err(e)).await;
            }
        });

        Ok(rx)
    }

    async fn is_loaded(&self, model_name: &str) -> bool {
        let loaded = self.loaded_models.read();
        loaded.contains_key(model_name)
    }

    async fn get_model_info(&self, model_name: &str) -> Result<ModelInfo> {
        // Clone the handle to avoid holding the lock across await
        let handle = {
            let loaded = self.loaded_models.read();
            loaded.get(model_name).cloned()
        };

        if let Some(handle) = handle {
            let state = handle.state().await;
            let model_path = state.model_path.clone();
            let memory_usage = self.estimate_memory_usage(&model_path);

            Ok(ModelInfo {
                name: handle.model_name.clone(),
                handle_id: handle.id.clone(),
                loaded_at: handle.loaded_at,
                memory_usage_bytes: memory_usage,
                config: state.config.clone(),
                is_busy: state.is_busy,
            })
        } else {
            Err(FuseError::ModelNotFound(format!(
                "Model '{}' is not loaded",
                model_name
            )))
        }
    }
}

impl LocalInferenceEngine {
    /// Internal inference implementation (placeholder)
    async fn perform_inference_internal(
        &self,
        handle: &ModelHandle,
        input: &InferenceInput,
    ) -> Result<String> {
        // This is a placeholder implementation
        // In a real implementation, this would:
        // 1. Load the model weights
        // 2. Tokenize the input
        // 3. Run the model forward pass
        // 4. Decode the output tokens
        // 5. Return the generated text

        tracing::debug!(
            model = %handle.model_name,
            prompt_length = input.prompt.len(),
            "Performing inference"
        );

        // Simulate inference delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Return placeholder response
        Ok(format!(
            "This is a placeholder response from model '{}'. In a real implementation, \
             this would be the actual model output based on the prompt: '{}'",
            handle.model_name,
            input.prompt.chars().take(50).collect::<String>()
        ))
    }

    /// Format text as markdown
    fn format_as_markdown(&self, text: &str) -> String {
        // Simple markdown formatting
        // In a real implementation, this could be more sophisticated
        text.to_string()
    }

    /// Estimate token count (placeholder)
    fn estimate_token_count(&self, text: &str) -> usize {
        // Rough estimate: ~4 characters per token
        text.len().div_ceil(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::inference::{InferenceInput, InferenceParameters};
    use crate::storage::Database;
    use tempfile::TempDir;

    async fn setup_test_engine() -> (LocalInferenceEngine, TempDir, TempDir) {
        let temp_db = TempDir::new().unwrap();
        let temp_models = TempDir::new().unwrap();

        let db_path = temp_db.path().join("test.redb");
        let db = Arc::new(Database::new(db_path).unwrap());
        let model_repo = Arc::new(ModelRepository::new(db));

        let engine = LocalInferenceEngine::new(model_repo, temp_models.path().to_path_buf());

        (engine, temp_db, temp_models)
    }

    async fn create_test_model(models_dir: &std::path::Path, model_name: &str) {
        let model_path = models_dir.join(model_name);
        tokio::fs::create_dir_all(&model_path).await.unwrap();

        // Create a dummy model file
        let model_file = model_path.join("model.bin");
        tokio::fs::write(&model_file, b"dummy model data")
            .await
            .unwrap();

        // Create config file
        let config = ModelConfig {
            max_context_length: 2048,
            architecture: "test".to_string(),
            extra: serde_json::json!({}),
        };
        let config_json = serde_json::to_string_pretty(&config).unwrap();
        let config_file = model_path.join("config.json");
        tokio::fs::write(&config_file, config_json).await.unwrap();
    }

    #[tokio::test]
    async fn test_load_model_success() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let result = engine.load_model("test-model").await;
        assert!(result.is_ok());

        let handle = result.unwrap();
        assert_eq!(handle.model_name, "test-model");
        assert!(!handle.id.is_empty());
    }

    #[tokio::test]
    async fn test_load_nonexistent_model() {
        let (engine, _temp_db, _temp_models) = setup_test_engine().await;

        let result = engine.load_model("nonexistent-model").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ModelNotFound(_)));
    }

    #[tokio::test]
    async fn test_model_caching() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        // Load model first time
        let handle1 = engine.load_model("test-model").await.unwrap();

        // Load same model again - should return cached handle
        let handle2 = engine.load_model("test-model").await.unwrap();

        assert_eq!(handle1.id, handle2.id);
    }

    #[tokio::test]
    async fn test_unload_model() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();
        assert!(engine.is_loaded("test-model").await);

        let result = engine.unload_model(handle).await;
        assert!(result.is_ok());
        assert!(!engine.is_loaded("test-model").await);
    }

    #[tokio::test]
    async fn test_is_loaded() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        assert!(!engine.is_loaded("test-model").await);

        let _handle = engine.load_model("test-model").await.unwrap();
        assert!(engine.is_loaded("test-model").await);
    }

    #[tokio::test]
    async fn test_get_model_info() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let info = engine.get_model_info("test-model").await.unwrap();
        assert_eq!(info.name, "test-model");
        assert_eq!(info.handle_id, handle.id);
        assert!(!info.is_busy);
        assert!(info.memory_usage_bytes > 0);
    }

    #[tokio::test]
    async fn test_infer_success() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let input = InferenceInput {
            prompt: "Hello, world!".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = engine.infer(&handle, input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.text.is_empty());
        assert!(!output.formatted_text.is_empty());
        assert_eq!(output.model, "test-model");
        assert!(output.prompt_tokens > 0);
        assert!(output.completion_tokens > 0);
        assert_eq!(
            output.total_tokens,
            output.prompt_tokens + output.completion_tokens
        );
        assert!(output.metadata.is_some());
    }

    #[tokio::test]
    async fn test_infer_with_invalid_parameters() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let mut params = InferenceParameters::default();
        params.temperature = 3.0; // Invalid: should be 0.0-2.0

        let input = InferenceInput {
            prompt: "Test".to_string(),
            images: vec![],
            context: None,
            parameters: params,
        };

        let result = engine.infer(&handle, input).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[tokio::test]
    async fn test_infer_formats_as_markdown() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let input = InferenceInput {
            prompt: "Generate markdown".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = engine.infer(&handle, input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        // In the placeholder implementation, formatted_text equals text
        assert_eq!(output.formatted_text, output.text);
    }

    #[tokio::test]
    async fn test_infer_respects_context_window() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        // Create a very long prompt
        let long_prompt = "word ".repeat(10000);

        let input = InferenceInput {
            prompt: long_prompt,
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        // Should still work (in real implementation, would truncate or error)
        let result = engine.infer(&handle, input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_infer_stream_success() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let input = InferenceInput {
            prompt: "Stream this".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = engine.infer_stream(&handle, input).await;
        assert!(result.is_ok());

        let mut rx = result.unwrap();
        let mut tokens = Vec::new();

        while let Some(token_result) = rx.recv().await {
            assert!(token_result.is_ok());
            tokens.push(token_result.unwrap());
        }

        assert!(!tokens.is_empty());
        assert!(tokens.last().unwrap().is_final);
    }

    #[tokio::test]
    async fn test_infer_stream_token_by_token() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let input = InferenceInput {
            prompt: "Test streaming".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let mut rx = engine.infer_stream(&handle, input).await.unwrap();
        let mut token_count = 0;

        while let Some(token_result) = rx.recv().await {
            assert!(token_result.is_ok());
            let token = token_result.unwrap();
            assert!(!token.text.is_empty());
            token_count += 1;
        }

        assert!(token_count > 0);
    }

    #[tokio::test]
    async fn test_infer_stream_cancellation() {
        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        let input = InferenceInput {
            prompt: "Cancel this".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let mut rx = engine.infer_stream(&handle, input).await.unwrap();

        // Receive first token
        let first_token = rx.recv().await;
        assert!(first_token.is_some());

        // Drop receiver to cancel streaming
        drop(rx);

        // Model should eventually become not busy
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let info = engine.get_model_info("test-model").await.unwrap();
        assert!(!info.is_busy);
    }

    #[tokio::test]
    async fn test_infer_with_images() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        // Create a test image
        let image_data = STANDARD.encode(b"fake image data");
        let image = crate::model::inference::Image::new(
            image_data,
            crate::model::inference::ImageFormat::Png,
        );

        let input = InferenceInput {
            prompt: "Describe this image".to_string(),
            images: vec![image],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = engine.infer(&handle, input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.text.is_empty());
    }

    #[tokio::test]
    async fn test_infer_with_multiple_images() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let (engine, _temp_db, temp_models) = setup_test_engine().await;
        create_test_model(temp_models.path(), "test-model").await;

        let handle = engine.load_model("test-model").await.unwrap();

        // Create multiple test images
        let image1 = crate::model::inference::Image::new(
            STANDARD.encode(b"image 1"),
            crate::model::inference::ImageFormat::Png,
        );
        let image2 = crate::model::inference::Image::new(
            STANDARD.encode(b"image 2"),
            crate::model::inference::ImageFormat::Jpg,
        );

        let input = InferenceInput {
            prompt: "Compare these images".to_string(),
            images: vec![image1, image2],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = engine.infer(&handle, input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let (temp_db, temp_models) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let db_path = temp_db.path().join("test.redb");
        let db = Arc::new(Database::new(db_path).unwrap());
        let model_repo = Arc::new(ModelRepository::new(db));

        // Create engine with cache size of 2
        let engine =
            LocalInferenceEngine::with_cache_size(model_repo, temp_models.path().to_path_buf(), 2);

        // Create 3 test models
        create_test_model(temp_models.path(), "model1").await;
        create_test_model(temp_models.path(), "model2").await;
        create_test_model(temp_models.path(), "model3").await;

        // Load 3 models - should evict the first one
        let _h1 = engine.load_model("model1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let _h2 = engine.load_model("model2").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let _h3 = engine.load_model("model3").await.unwrap();

        // model1 should have been evicted
        assert!(!engine.is_loaded("model1").await);
        assert!(engine.is_loaded("model2").await);
        assert!(engine.is_loaded("model3").await);
    }
}
