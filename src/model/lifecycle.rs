//! Model lifecycle management — list, inspect, remove, cache operations.

use crate::error::{FuseError, Result};
use crate::model::formats::gguf::GgufFile;
use crate::model::registry::huggingface::{HfDownloadOptions, HfModelRegistry};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Information about a locally cached model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModel {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub format: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub tensor_count: usize,
}

/// Model lifecycle manager — handles pull, list, inspect, remove.
pub struct ModelLifecycle {
    models_dir: PathBuf,
    hf_registry: HfModelRegistry,
}

impl ModelLifecycle {
    pub fn new(models_dir: &Path) -> Self {
        Self {
            models_dir: models_dir.to_path_buf(),
            hf_registry: HfModelRegistry::new(models_dir),
        }
    }

    /// List all locally cached models.
    pub async fn list(&self) -> Result<Vec<LocalModel>> {
        let mut models = Vec::new();

        if !self.models_dir.exists() {
            return Ok(models);
        }

        let mut entries = tokio::fs::read_dir(&self.models_dir)
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to read models dir: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| FuseError::InternalError(format!("Dir entry error: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                // Scan for GGUF files in subdirectories
                if let Ok(mut sub_entries) = tokio::fs::read_dir(&path).await {
                    while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().is_some_and(|e| e == "gguf") {
                            if let Ok(model) = self.inspect_file(&sub_path).await {
                                models.push(model);
                            }
                        }
                    }
                }
            } else if path.extension().is_some_and(|e| e == "gguf") {
                if let Ok(model) = self.inspect_file(&path).await {
                    models.push(model);
                }
            }
        }

        models.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(models)
    }

    /// Inspect a model file and return its metadata.
    pub async fn inspect_file(&self, path: &Path) -> Result<LocalModel> {
        let file = std::fs::File::open(path).map_err(|e| {
            FuseError::InternalError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let metadata = file
            .metadata()
            .map_err(|e| FuseError::InternalError(format!("Failed to get metadata: {}", e)))?;

        let mut reader = std::io::BufReader::new(file);
        let gguf = GgufFile::parse(&mut reader)?;

        let name = gguf
            .model_name()
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            })
            .to_string();

        Ok(LocalModel {
            name,
            path: path.to_path_buf(),
            size_bytes: metadata.len(),
            format: "gguf".to_string(),
            architecture: gguf.architecture().map(|s| s.to_string()),
            quantization: gguf.tensors.first().map(|t| format!("{:?}", t.ggml_type)),
            tensor_count: gguf.tensors.len(),
        })
    }

    /// Inspect a model by name (searches the models directory).
    pub async fn inspect(&self, name: &str) -> Result<LocalModel> {
        let models = self.list().await?;
        models
            .into_iter()
            .find(|m| m.name == name || m.path.to_string_lossy().contains(name))
            .ok_or_else(|| FuseError::ModelNotFound(name.to_string()))
    }

    /// Remove a model from the cache.
    pub async fn remove(&self, name: &str) -> Result<u64> {
        let model = self.inspect(name).await?;
        let size = model.size_bytes;

        tokio::fs::remove_file(&model.path).await.map_err(|e| {
            FuseError::InternalError(format!("Failed to remove {}: {}", model.path.display(), e))
        })?;

        // Try to remove parent dir if empty
        if let Some(parent) = model.path.parent() {
            let _ = tokio::fs::remove_dir(parent).await; // Ignore if not empty
        }

        Ok(size)
    }

    /// Pull a model from HuggingFace.
    pub async fn pull(
        &self,
        repo_id: &str,
        filename: Option<&str>,
        token: Option<&str>,
    ) -> Result<PathBuf> {
        let opts = HfDownloadOptions {
            repo_id: repo_id.to_string(),
            filename: filename.map(|s| s.to_string()),
            revision: None,
            auth_token: token.map(|s| s.to_string()),
            resume: true,
        };

        self.hf_registry.download(&opts, None).await
    }

    /// Total disk usage of all cached models.
    pub async fn disk_usage(&self) -> Result<u64> {
        let models = self.list().await?;
        Ok(models.iter().map(|m| m.size_bytes).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::formats::gguf::{GGUF_MAGIC, GGUF_VERSION_3};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_gguf(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();

        // Minimal GGUF
        file.write_all(&GGUF_MAGIC.to_le_bytes()).unwrap();
        file.write_all(&GGUF_VERSION_3.to_le_bytes()).unwrap();
        file.write_all(&0u64.to_le_bytes()).unwrap(); // 0 tensors
        file.write_all(&1u64.to_le_bytes()).unwrap(); // 1 metadata

        // general.name = test-model
        let key = "general.name";
        file.write_all(&(key.len() as u64).to_le_bytes()).unwrap();
        file.write_all(key.as_bytes()).unwrap();
        file.write_all(&8u32.to_le_bytes()).unwrap();
        let val = name.trim_end_matches(".gguf");
        file.write_all(&(val.len() as u64).to_le_bytes()).unwrap();
        file.write_all(val.as_bytes()).unwrap();

        path
    }

    #[tokio::test]
    async fn test_list_empty() {
        let dir = TempDir::new().unwrap();
        let lifecycle = ModelLifecycle::new(dir.path());
        let models = lifecycle.list().await.unwrap();
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_list_finds_gguf_files() {
        let dir = TempDir::new().unwrap();
        create_test_gguf(dir.path(), "model-a.gguf");
        create_test_gguf(dir.path(), "model-b.gguf");

        let lifecycle = ModelLifecycle::new(dir.path());
        let models = lifecycle.list().await.unwrap();
        assert_eq!(models.len(), 2);
    }

    #[tokio::test]
    async fn test_inspect_file() {
        let dir = TempDir::new().unwrap();
        let path = create_test_gguf(dir.path(), "test-model.gguf");

        let lifecycle = ModelLifecycle::new(dir.path());
        let model = lifecycle.inspect_file(&path).await.unwrap();
        assert_eq!(model.name, "test-model");
        assert_eq!(model.format, "gguf");
        assert!(model.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_remove_model() {
        let dir = TempDir::new().unwrap();
        create_test_gguf(dir.path(), "removeme.gguf");

        let lifecycle = ModelLifecycle::new(dir.path());
        let freed = lifecycle.remove("removeme").await.unwrap();
        assert!(freed > 0);

        let models = lifecycle.list().await.unwrap();
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_disk_usage() {
        let dir = TempDir::new().unwrap();
        create_test_gguf(dir.path(), "a.gguf");
        create_test_gguf(dir.path(), "b.gguf");

        let lifecycle = ModelLifecycle::new(dir.path());
        let usage = lifecycle.disk_usage().await.unwrap();
        assert!(usage > 0);
    }

    #[tokio::test]
    async fn test_inspect_not_found() {
        let dir = TempDir::new().unwrap();
        let lifecycle = ModelLifecycle::new(dir.path());
        assert!(lifecycle.inspect("nonexistent").await.is_err());
    }
}
