use crate::error::{FuseError, Result};
use crate::model::huggingface::HuggingFaceClient;
use crate::model::unsloth::UnslothClient;
use crate::model::{Auth, ModelMetadata, ModelSource, Provider};
use crate::storage::{DownloadManager, DownloadProgress, ModelRepository};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

/// Sort options for model listing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    /// Sort by model name
    #[default]
    Name,
    /// Sort by model size (largest first)
    Size,
    /// Sort by download date (newest first)
    Downloaded,
    /// Sort by last update date (newest first)
    Updated,
}

/// Model manager for handling model operations
pub struct ModelManager {
    repository: Arc<ModelRepository>,
    #[allow(dead_code)]
    download_manager: DownloadManager,
    models_dir: PathBuf,
    hf_client: HuggingFaceClient,
    unsloth_client: UnslothClient,
}

impl ModelManager {
    /// Create a new model manager
    pub fn new(repository: Arc<ModelRepository>, models_dir: PathBuf) -> Self {
        Self {
            repository,
            download_manager: DownloadManager::new(),
            models_dir,
            hf_client: HuggingFaceClient::new(),
            unsloth_client: UnslothClient::new(),
        }
    }

    /// Pull a model from a source
    pub async fn pull(
        &self,
        source: ModelSource,
        name: &str,
        auth: Option<Auth>,
        format: Option<String>,
        resume: bool,
    ) -> Result<ModelMetadata> {
        info!("Pulling model {} from {}", name, source);

        // Check if model already exists
        if self.repository.exists(name)? {
            warn!("Model {} already exists. Use update to refresh it.", name);
            return Err(FuseError::ValidationError(format!(
                "Model {} already exists. Use 'fuse update {}' to pull the latest version.",
                name, name
            )));
        }

        match source.provider {
            Provider::HuggingFace => self.pull_from_huggingface(source, name, auth, format, resume).await,
            Provider::Unsloth => self.pull_from_unsloth(source, name, auth, format, resume).await,
            Provider::Remote => Err(FuseError::FeatureDisabled(
                "Remote model pulling will be implemented in task 6".to_string(),
            )),
            Provider::Local => Err(FuseError::ValidationError(
                "Cannot pull from local source. Model is already local.".to_string(),
            )),
        }
    }

    /// Pull a model from Hugging Face
    async fn pull_from_huggingface(
        &self,
        source: ModelSource,
        name: &str,
        auth: Option<Auth>,
        format: Option<String>,
        resume: bool,
    ) -> Result<ModelMetadata> {
        info!("Pulling model from Hugging Face: {}", source.repository);

        // Get model info
        let model_info = self
            .hf_client
            .get_model_info(&source.repository, auth.as_ref())
            .await?;

        info!("Model info: {:?}", model_info);

        // Create model directory
        let model_dir = self.models_dir.join(name);
        tokio::fs::create_dir_all(&model_dir).await?;

        // Download all model files with progress tracking
        let downloaded_files = self
            .hf_client
            .download_model(
                &source.repository,
                source.version.as_deref(),
                &model_dir,
                auth.as_ref(),
                format.as_deref(),
                resume,
                |file_path, progress| {
                    self.print_progress(file_path, &progress);
                },
            )
            .await?;

        // Calculate total size
        let mut total_size = 0u64;
        for file_path in &downloaded_files {
            let full_path = model_dir.join(file_path);
            if let Ok(metadata) = tokio::fs::metadata(&full_path).await {
                total_size += metadata.len();
            }
        }

        // Create metadata
        let mut metadata = ModelMetadata::new(
            name,
            name,
            source.clone(),
            source.version.clone().unwrap_or_else(|| "main".to_string()),
            total_size,
        );

        // Add tags from model info
        if let Some(tags) = model_info.tags {
            metadata = metadata.with_tags(tags);
        }

        // Add library name
        if let Some(library) = model_info.library_name {
            metadata = metadata.with_architecture(library);
        }

        // Add file paths
        for file_path in downloaded_files {
            metadata = metadata.with_file_path(file_path.clone());

            // Identify special files
            if file_path.ends_with("config.json") {
                metadata = metadata.with_config_path(file_path.clone());
            } else if file_path.contains("tokenizer") && file_path.ends_with(".json") {
                metadata = metadata.with_tokenizer_path(file_path);
            }
        }

        // Save metadata to repository
        self.repository.save(&metadata)?;

        info!("Successfully pulled model {}", name);

        Ok(metadata)
    }

    /// Pull a model from Unsloth
    async fn pull_from_unsloth(
        &self,
        source: ModelSource,
        name: &str,
        auth: Option<Auth>,
        format: Option<String>,
        resume: bool,
    ) -> Result<ModelMetadata> {
        info!("Pulling model from Unsloth: {}", source.repository);

        // Get model info
        let model_info = self
            .unsloth_client
            .get_model_info(&source.repository, auth.as_ref())
            .await?;

        info!("Model info: {:?}", model_info);

        // Create model directory
        let model_dir = self.models_dir.join(name);
        tokio::fs::create_dir_all(&model_dir).await?;

        // Download all model files with progress tracking
        let downloaded_files = self
            .unsloth_client
            .download_model(
                &source.repository,
                &model_dir,
                auth.as_ref(),
                format.as_deref(),
                resume,
                |file_path, progress| {
                    self.print_progress(file_path, &progress);
                },
            )
            .await?;

        // Calculate total size
        let mut total_size = 0u64;
        for file_path in &downloaded_files {
            let full_path = model_dir.join(file_path);
            if let Ok(metadata) = tokio::fs::metadata(&full_path).await {
                total_size += metadata.len();
            }
        }

        // Create metadata
        let mut metadata = ModelMetadata::new(
            name,
            model_info.name.clone(),
            source.clone(),
            model_info
                .version
                .clone()
                .unwrap_or_else(|| "latest".to_string()),
            total_size,
        );

        // Add tags from model info
        if let Some(tags) = model_info.tags {
            metadata = metadata.with_tags(tags);
        }

        // Add architecture
        if let Some(architecture) = model_info.architecture {
            metadata = metadata.with_architecture(architecture);
        }

        // Add parameter count
        if let Some(parameters) = model_info.parameters {
            metadata = metadata.with_parameter_count(parameters);
        }

        // Add file paths
        for file_path in downloaded_files {
            metadata = metadata.with_file_path(file_path.clone());

            // Identify special files
            if file_path.ends_with("config.json") {
                metadata = metadata.with_config_path(file_path.clone());
            } else if file_path.contains("tokenizer") && file_path.ends_with(".json") {
                metadata = metadata.with_tokenizer_path(file_path);
            }
        }

        // Save metadata to repository
        self.repository.save(&metadata)?;

        info!("Successfully pulled model {}", name);

        Ok(metadata)
    }

    /// Print download progress
    fn print_progress(&self, file_path: &str, progress: &DownloadProgress) {
        if let Some(percentage) = progress.percentage {
            let speed_mb = progress.speed_bytes_per_sec / (1024.0 * 1024.0);
            let eta = progress
                .eta_seconds
                .map(|s| format!("{}s", s))
                .unwrap_or_else(|| "unknown".to_string());

            println!(
                "  {} - {:.1}% ({:.2} MB/s, ETA: {})",
                file_path, percentage, speed_mb, eta
            );
        } else {
            let downloaded_mb = progress.bytes_downloaded as f64 / (1024.0 * 1024.0);
            let speed_mb = progress.speed_bytes_per_sec / (1024.0 * 1024.0);

            println!(
                "  {} - {:.2} MB ({:.2} MB/s)",
                file_path, downloaded_mb, speed_mb
            );
        }
    }

    /// List all downloaded models
    pub async fn list(&self) -> Result<Vec<ModelMetadata>> {
        info!("Listing all downloaded models");

        let models = self.repository.list()?;

        info!("Found {} models", models.len());

        Ok(models)
    }

    /// List models with filtering and sorting
    pub async fn list_filtered(
        &self,
        source_filter: Option<Provider>,
        sort_by: SortBy,
    ) -> Result<Vec<ModelMetadata>> {
        info!("Listing models with filters");

        let mut models = self.repository.list()?;

        // Apply source filter
        if let Some(provider) = source_filter {
            models.retain(|m| m.source.provider == provider);
        }

        // Sort models
        match sort_by {
            SortBy::Name => {
                models.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortBy::Size => {
                models.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
            }
            SortBy::Downloaded => {
                models.sort_by(|a, b| b.downloaded_at.cmp(&a.downloaded_at));
            }
            SortBy::Updated => {
                models.sort_by(|a, b| match (b.updated_at, a.updated_at) {
                    (Some(b_time), Some(a_time)) => b_time.cmp(&a_time),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                });
            }
        }

        info!("Found {} models after filtering", models.len());

        Ok(models)
    }

    /// Get metadata for a specific model
    pub async fn get_metadata(&self, name: &str) -> Result<Option<ModelMetadata>> {
        info!("Getting metadata for model: {}", name);

        let metadata = self.repository.get(name)?;

        if metadata.is_some() {
            info!("Found metadata for model: {}", name);
        } else {
            info!("Model not found: {}", name);
        }

        Ok(metadata)
    }

    /// Remove a model
    pub async fn remove(&self, name: &str) -> Result<()> {
        info!("Removing model: {}", name);

        // Get model metadata
        let _metadata = self
            .repository
            .get(name)?
            .ok_or_else(|| FuseError::ModelNotFound(name.to_string()))?;

        // Delete model directory
        let model_dir = self.models_dir.join(name);
        if model_dir.exists() {
            info!("Deleting model directory: {:?}", model_dir);
            tokio::fs::remove_dir_all(&model_dir)
                .await
                .map_err(FuseError::IoError)?;
        } else {
            warn!("Model directory not found: {:?}", model_dir);
        }

        // Delete metadata from repository
        self.repository.delete(name)?;

        info!("Successfully removed model: {}", name);

        Ok(())
    }

    /// Update a model
    pub async fn update(&self, name: &str) -> Result<ModelMetadata> {
        info!("Updating model: {}", name);

        // Get existing model metadata
        let old_metadata = self
            .repository
            .get(name)?
            .ok_or_else(|| FuseError::ModelNotFound(name.to_string()))?;

        info!(
            "Found existing model: {} ({})",
            old_metadata.name, old_metadata.source
        );

        // Delete existing metadata first to allow re-pull
        self.repository.delete(name)?;

        // Remove old model files
        let model_dir = self.models_dir.join(name);
        if model_dir.exists() {
            info!("Removing old model files from: {:?}", model_dir);
            tokio::fs::remove_dir_all(&model_dir).await?;
        }

        // Re-download the model using the same source
        let source = old_metadata.source.clone();
        let auth = None; // TODO: Store auth in metadata for updates

        let mut new_metadata = match source.provider {
            Provider::HuggingFace => self.pull_from_huggingface(source, name, auth, None, false).await?,
            Provider::Unsloth => self.pull_from_unsloth(source, name, auth, None, false).await?,
            Provider::Remote => {
                return Err(FuseError::FeatureDisabled(
                    "Remote model updates will be implemented in task 6".to_string(),
                ));
            }
            Provider::Local => {
                return Err(FuseError::ValidationError(
                    "Cannot update local models".to_string(),
                ));
            }
        };

        // Mark as updated
        new_metadata.mark_updated();

        // Save updated metadata
        self.repository.save(&new_metadata)?;

        info!("Successfully updated model: {}", name);

        Ok(new_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    // Helper function to create a test database and model manager
    fn create_test_manager() -> (ModelManager, TempDir, TempDir) {
        let db_temp_dir = TempDir::new().unwrap();
        let models_temp_dir = TempDir::new().unwrap();

        let db_path = db_temp_dir.path().join("test.redb");
        let db = Arc::new(Database::new(db_path).unwrap());
        let repository = Arc::new(ModelRepository::new(db));

        let manager = ModelManager::new(repository, models_temp_dir.path().to_path_buf());

        (manager, db_temp_dir, models_temp_dir)
    }

    // Helper function to create test metadata
    fn create_test_metadata(name: &str) -> ModelMetadata {
        ModelMetadata::new(
            name,
            name,
            ModelSource::huggingface("test/model"),
            "1.0.0",
            1024 * 1024,
        )
        .with_architecture("transformer")
        .with_parameter_count(7_000_000_000)
        .with_tag("test")
    }

    #[tokio::test]
    async fn test_model_manager_creation() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Verify manager is created successfully
        assert!(manager.models_dir.exists());
    }

    #[tokio::test]
    async fn test_list_empty_models() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let models = manager.list().await.unwrap();
        assert_eq!(models.len(), 0);
    }

    #[tokio::test]
    async fn test_list_models_after_save() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save a test model metadata
        let metadata = create_test_metadata("test-model");
        manager.repository.save(&metadata).unwrap();

        let models = manager.list().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "test-model");
    }

    #[tokio::test]
    async fn test_get_metadata_existing_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save a test model metadata
        let metadata = create_test_metadata("test-model");
        manager.repository.save(&metadata).unwrap();

        let retrieved = manager.get_metadata("test-model").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-model");
    }

    #[tokio::test]
    async fn test_get_metadata_nonexistent_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let retrieved = manager.get_metadata("nonexistent").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_remove_existing_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save a test model metadata
        let metadata = create_test_metadata("test-model");
        manager.repository.save(&metadata).unwrap();

        // Create model directory
        let model_dir = manager.models_dir.join("test-model");
        tokio::fs::create_dir_all(&model_dir).await.unwrap();
        tokio::fs::write(model_dir.join("test.txt"), "test")
            .await
            .unwrap();

        // Remove the model
        let result = manager.remove("test-model").await;
        assert!(result.is_ok());

        // Verify model is removed from repository
        let retrieved = manager.repository.get("test-model").unwrap();
        assert!(retrieved.is_none());

        // Verify model directory is removed
        assert!(!model_dir.exists());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let result = manager.remove("nonexistent").await;
        assert!(result.is_err());

        match result {
            Err(FuseError::ModelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_pull_duplicate_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save a test model metadata to simulate existing model
        let metadata = create_test_metadata("test-model");
        manager.repository.save(&metadata).unwrap();

        // Try to pull the same model again
        let source = ModelSource::huggingface("test/model");
        let result = manager.pull(source, "test-model", None, None, false).await;

        assert!(result.is_err());
        match result {
            Err(FuseError::ValidationError(msg)) => {
                assert!(msg.contains("already exists"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_pull_from_local_source() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let source = ModelSource::local("/path/to/model");
        let result = manager.pull(source, "test-model", None, None, false).await;

        assert!(result.is_err());
        match result {
            Err(FuseError::ValidationError(msg)) => {
                assert!(msg.contains("Cannot pull from local source"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_pull_from_remote_source() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let source = ModelSource::remote("https://example.com/model");
        let result = manager.pull(source, "test-model", None, None, false).await;

        assert!(result.is_err());
        match result {
            Err(FuseError::FeatureDisabled(msg)) => {
                assert!(msg.contains("Remote model pulling"));
            }
            _ => panic!("Expected FeatureDisabled error"),
        }
    }

    #[tokio::test]
    async fn test_update_nonexistent_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        let result = manager.update("nonexistent").await;
        assert!(result.is_err());

        match result {
            Err(FuseError::ModelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_update_local_model() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Create a local model
        let metadata = ModelMetadata::new(
            "local-model",
            "local-model",
            ModelSource::local("/path/to/model"),
            "1.0.0",
            1024,
        );
        manager.repository.save(&metadata).unwrap();

        let result = manager.update("local-model").await;
        assert!(result.is_err());

        match result {
            Err(FuseError::ValidationError(msg)) => {
                assert!(msg.contains("Cannot update local models"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_list_filtered_by_provider() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save models from different providers
        let hf_model = ModelMetadata::new(
            "hf-model",
            "hf-model",
            ModelSource::huggingface("test/hf"),
            "1.0.0",
            1024,
        );
        manager.repository.save(&hf_model).unwrap();

        let unsloth_model = ModelMetadata::new(
            "unsloth-model",
            "unsloth-model",
            ModelSource::unsloth("test/unsloth"),
            "1.0.0",
            1024,
        );
        manager.repository.save(&unsloth_model).unwrap();

        // Filter by HuggingFace
        let hf_models = manager
            .list_filtered(Some(Provider::HuggingFace), SortBy::Name)
            .await
            .unwrap();
        assert_eq!(hf_models.len(), 1);
        assert_eq!(hf_models[0].name, "hf-model");

        // Filter by Unsloth
        let unsloth_models = manager
            .list_filtered(Some(Provider::Unsloth), SortBy::Name)
            .await
            .unwrap();
        assert_eq!(unsloth_models.len(), 1);
        assert_eq!(unsloth_models[0].name, "unsloth-model");
    }

    #[tokio::test]
    async fn test_list_sorted_by_name() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save models with different names
        let model_c = create_test_metadata("c-model");
        let model_a = create_test_metadata("a-model");
        let model_b = create_test_metadata("b-model");

        manager.repository.save(&model_c).unwrap();
        manager.repository.save(&model_a).unwrap();
        manager.repository.save(&model_b).unwrap();

        let models = manager.list_filtered(None, SortBy::Name).await.unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].name, "a-model");
        assert_eq!(models[1].name, "b-model");
        assert_eq!(models[2].name, "c-model");
    }

    #[tokio::test]
    async fn test_list_sorted_by_size() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save models with different sizes
        let small_model = ModelMetadata::new(
            "small",
            "small",
            ModelSource::huggingface("test/small"),
            "1.0.0",
            1024,
        );

        let large_model = ModelMetadata::new(
            "large",
            "large",
            ModelSource::huggingface("test/large"),
            "1.0.0",
            1024 * 1024 * 1024,
        );

        let medium_model = ModelMetadata::new(
            "medium",
            "medium",
            ModelSource::huggingface("test/medium"),
            "1.0.0",
            1024 * 1024,
        );

        manager.repository.save(&small_model).unwrap();
        manager.repository.save(&large_model).unwrap();
        manager.repository.save(&medium_model).unwrap();

        let models = manager.list_filtered(None, SortBy::Size).await.unwrap();
        assert_eq!(models.len(), 3);
        // Should be sorted largest first
        assert_eq!(models[0].name, "large");
        assert_eq!(models[1].name, "medium");
        assert_eq!(models[2].name, "small");
    }

    #[tokio::test]
    async fn test_list_sorted_by_downloaded() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save models with different download times
        let mut old_model = create_test_metadata("old-model");
        old_model.downloaded_at = chrono::Utc::now() - chrono::Duration::days(10);

        let mut new_model = create_test_metadata("new-model");
        new_model.downloaded_at = chrono::Utc::now();

        manager.repository.save(&old_model).unwrap();
        manager.repository.save(&new_model).unwrap();

        let models = manager
            .list_filtered(None, SortBy::Downloaded)
            .await
            .unwrap();
        assert_eq!(models.len(), 2);
        // Should be sorted newest first
        assert_eq!(models[0].name, "new-model");
        assert_eq!(models[1].name, "old-model");
    }

    #[tokio::test]
    async fn test_list_sorted_by_updated() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save models with different update times
        let mut never_updated = create_test_metadata("never-updated");
        never_updated.updated_at = None;

        let mut recently_updated = create_test_metadata("recently-updated");
        recently_updated.updated_at = Some(chrono::Utc::now());

        let mut old_updated = create_test_metadata("old-updated");
        old_updated.updated_at = Some(chrono::Utc::now() - chrono::Duration::days(5));

        manager.repository.save(&never_updated).unwrap();
        manager.repository.save(&recently_updated).unwrap();
        manager.repository.save(&old_updated).unwrap();

        let models = manager.list_filtered(None, SortBy::Updated).await.unwrap();
        assert_eq!(models.len(), 3);
        // Should be sorted newest first, with never-updated last
        // Models with updated_at come first, sorted by most recent
        let has_update: Vec<_> = models.iter().filter(|m| m.updated_at.is_some()).collect();
        let no_update: Vec<_> = models.iter().filter(|m| m.updated_at.is_none()).collect();

        assert_eq!(has_update.len(), 2);
        assert_eq!(no_update.len(), 1);
        assert_eq!(has_update[0].name, "recently-updated");
        assert_eq!(has_update[1].name, "old-updated");
        assert_eq!(no_update[0].name, "never-updated");
    }

    #[tokio::test]
    async fn test_sortby_default() {
        let sort_by = SortBy::default();
        assert_eq!(sort_by, SortBy::Name);
    }

    #[tokio::test]
    async fn test_remove_model_without_directory() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save metadata but don't create directory
        let metadata = create_test_metadata("test-model");
        manager.repository.save(&metadata).unwrap();

        // Remove should still succeed even if directory doesn't exist
        let result = manager.remove("test-model").await;
        assert!(result.is_ok());

        // Verify model is removed from repository
        let retrieved = manager.repository.get("test-model").unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_metadata_storage_and_retrieval() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Create comprehensive metadata
        let metadata = ModelMetadata::new(
            "comprehensive-model",
            "comprehensive-model",
            ModelSource::huggingface("test/comprehensive"),
            "2.0.0",
            5 * 1024 * 1024 * 1024, // 5GB
        )
        .with_architecture("llama")
        .with_parameter_count(13_000_000_000)
        .with_tag("nlp")
        .with_tag("text-generation")
        .with_file_path("model.safetensors")
        .with_file_path("config.json")
        .with_config_path("config.json")
        .with_tokenizer_path("tokenizer.json");

        // Save metadata
        manager.repository.save(&metadata).unwrap();

        // Retrieve and verify all fields
        let retrieved = manager.get_metadata("comprehensive-model").await.unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "comprehensive-model");
        assert_eq!(retrieved.version, "2.0.0");
        assert_eq!(retrieved.size_bytes, 5 * 1024 * 1024 * 1024);
        assert_eq!(retrieved.architecture, Some("llama".to_string()));
        assert_eq!(retrieved.parameter_count, Some(13_000_000_000));
        assert_eq!(retrieved.tags.len(), 2);
        assert!(retrieved.tags.contains(&"nlp".to_string()));
        assert!(retrieved.tags.contains(&"text-generation".to_string()));
        assert_eq!(retrieved.file_paths.len(), 2);
        assert_eq!(retrieved.config_path, Some("config.json".to_string()));
        assert_eq!(retrieved.tokenizer_path, Some("tokenizer.json".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_models_storage() {
        let (manager, _db_temp, _models_temp) = create_test_manager();

        // Save multiple models
        for i in 0..10 {
            let metadata = ModelMetadata::new(
                &format!("model-{}", i),
                &format!("model-{}", i),
                ModelSource::huggingface(&format!("test/model-{}", i)),
                "1.0.0",
                (i as u64 + 1) * 1024 * 1024,
            );
            manager.repository.save(&metadata).unwrap();
        }

        // Verify all models are stored
        let models = manager.list().await.unwrap();
        assert_eq!(models.len(), 10);

        // Verify each model can be retrieved individually
        for i in 0..10 {
            let retrieved = manager.get_metadata(&format!("model-{}", i)).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().name, format!("model-{}", i));
        }
    }
}
