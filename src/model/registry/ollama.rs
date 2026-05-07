//! Ollama model registry — resolve and download models using Ollama-style model names.
//!
//! Supports the `model:tag` format (e.g., `llama3.2:7b`, `mistral:latest`).
//! Resolves to HuggingFace downloads or Ollama's own registry.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const OLLAMA_REGISTRY_BASE: &str = "https://registry.ollama.ai/v2/library";

/// Parsed Ollama model reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelRef {
    pub name: String,
    pub tag: String,
}

impl OllamaModelRef {
    /// Parse a model reference like "llama3.2:7b" or "mistral".
    pub fn parse(model_ref: &str) -> Self {
        let parts: Vec<&str> = model_ref.splitn(2, ':').collect();
        Self {
            name: parts[0].to_string(),
            tag: parts.get(1).unwrap_or(&"latest").to_string(),
        }
    }

    /// Full reference string.
    pub fn full_ref(&self) -> String {
        format!("{}:{}", self.name, self.tag)
    }
}

/// Ollama manifest (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<u32>,
    pub layers: Vec<OllamaLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaLayer {
    pub digest: String,
    pub size: u64,
    #[serde(rename = "mediaType")]
    pub media_type: String,
}

impl OllamaManifest {
    /// Get the model weights layer (GGUF).
    pub fn model_layer(&self) -> Option<&OllamaLayer> {
        self.layers.iter().find(|l| l.media_type.contains("model"))
    }
}

/// Ollama model registry client.
pub struct OllamaRegistry {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl OllamaRegistry {
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            client: reqwest::Client::new(),
            cache_dir: cache_dir.to_path_buf(),
        }
    }

    /// Fetch the manifest for a model.
    pub async fn get_manifest(&self, model_ref: &OllamaModelRef) -> Result<OllamaManifest> {
        let url = format!(
            "{}/{}/manifests/{}",
            OLLAMA_REGISTRY_BASE, model_ref.name, model_ref.tag
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FuseError::InternalError(format!("Ollama registry error: {}", e)))?;

        if !resp.status().is_success() {
            return Err(FuseError::InternalError(format!(
                "Ollama registry returned status {} for {}",
                resp.status(),
                model_ref.full_ref()
            )));
        }

        resp.json::<OllamaManifest>()
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to parse manifest: {}", e)))
    }

    /// Download a model blob by digest.
    pub async fn download_blob(
        &self,
        model_ref: &OllamaModelRef,
        digest: &str,
        dest: &Path,
    ) -> Result<PathBuf> {
        let url = format!(
            "{}/{}/blobs/{}",
            OLLAMA_REGISTRY_BASE, model_ref.name, digest
        );

        let dest_path = dest.join(digest.replace(':', "-"));

        if dest_path.exists() {
            return Ok(dest_path);
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FuseError::InternalError(format!("Blob download failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(FuseError::InternalError(format!(
                "Blob download returned status {}",
                resp.status()
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to read blob: {}", e)))?;

        tokio::fs::write(&dest_path, &bytes)
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to write blob: {}", e)))?;

        Ok(dest_path)
    }

    /// Get the cache directory for a model.
    pub fn model_cache_dir(&self, model_ref: &OllamaModelRef) -> PathBuf {
        self.cache_dir
            .join(format!("ollama-{}-{}", model_ref.name, model_ref.tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_ref() {
        let m = OllamaModelRef::parse("llama3.2:7b");
        assert_eq!(m.name, "llama3.2");
        assert_eq!(m.tag, "7b");
        assert_eq!(m.full_ref(), "llama3.2:7b");
    }

    #[test]
    fn test_parse_model_ref_no_tag() {
        let m = OllamaModelRef::parse("mistral");
        assert_eq!(m.name, "mistral");
        assert_eq!(m.tag, "latest");
    }

    #[test]
    fn test_manifest_model_layer() {
        let manifest = OllamaManifest {
            schema_version: Some(2),
            layers: vec![
                OllamaLayer {
                    digest: "sha256:abc".to_string(),
                    size: 100,
                    media_type: "application/vnd.ollama.image.template".to_string(),
                },
                OllamaLayer {
                    digest: "sha256:def".to_string(),
                    size: 4_000_000_000,
                    media_type: "application/vnd.ollama.image.model".to_string(),
                },
            ],
        };

        let model = manifest.model_layer().unwrap();
        assert_eq!(model.digest, "sha256:def");
        assert_eq!(model.size, 4_000_000_000);
    }

    #[test]
    fn test_model_cache_dir() {
        let registry = OllamaRegistry::new(Path::new("/cache"));
        let model_ref = OllamaModelRef::parse("llama3.2:7b");
        let dir = registry.model_cache_dir(&model_ref);
        assert_eq!(dir, PathBuf::from("/cache/ollama-llama3.2-7b"));
    }

    #[test]
    fn test_manifest_deserialization() {
        let json = r#"{
            "schemaVersion": 2,
            "layers": [
                {
                    "digest": "sha256:abc123",
                    "size": 1000,
                    "mediaType": "application/vnd.ollama.image.model"
                }
            ]
        }"#;

        let manifest: OllamaManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.schema_version, Some(2));
        assert_eq!(manifest.layers.len(), 1);
        assert!(manifest.model_layer().is_some());
    }
}
