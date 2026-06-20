use crate::error::{FuseError, Result};
use crate::model::Auth;
use crate::storage::DownloadProgress;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

/// Hugging Face API client
pub struct HuggingFaceClient {
    client: Client,
    base_url: String,
}

/// Hugging Face model info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceModelInfo {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(rename = "sha")]
    pub sha: Option<String>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pipeline_tag: Option<String>,
    pub library_name: Option<String>,
}

/// Hugging Face file info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceFileInfo {
    pub path: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub file_type: String,
    pub oid: Option<String>,
}

impl HuggingFaceClient {
    /// Create a new Hugging Face client
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            base_url: "https://huggingface.co".to_string(),
        }
    }

    /// Get model information
    pub async fn get_model_info(
        &self,
        repository: &str,
        auth: Option<&Auth>,
    ) -> Result<HuggingFaceModelInfo> {
        let url = format!("{}/api/models/{}", self.base_url, repository);

        let mut request = self.client.get(&url).timeout(std::time::Duration::from_secs(30));

        // Add authentication if provided
        if let Some(auth) = auth {
            if let Some(header_value) = auth.to_header_value() {
                request = request.header("Authorization", header_value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("Failed to get model info: {}", e)))?;

        if !response.status().is_success() {
            return Err(FuseError::DownloadError(format!(
                "Failed to get model info: HTTP {}",
                response.status()
            )));
        }

        let model_info = response.json::<HuggingFaceModelInfo>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse model info: {}", e))
        })?;

        Ok(model_info)
    }

    /// List files in a model repository
    pub async fn list_files(
        &self,
        repository: &str,
        revision: Option<&str>,
        auth: Option<&Auth>,
    ) -> Result<Vec<HuggingFaceFileInfo>> {
        let revision = revision.unwrap_or("main");
        let url = format!(
            "{}/api/models/{}/tree/{}",
            self.base_url, repository, revision
        );

        let mut request = self.client.get(&url).timeout(std::time::Duration::from_secs(30));

        // Add authentication if provided
        if let Some(auth) = auth {
            if let Some(header_value) = auth.to_header_value() {
                request = request.header("Authorization", header_value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("Failed to list files: {}", e)))?;

        if !response.status().is_success() {
            return Err(FuseError::DownloadError(format!(
                "Failed to list files: HTTP {}",
                response.status()
            )));
        }

        let files = response
            .json::<Vec<HuggingFaceFileInfo>>()
            .await
            .map_err(|e| {
                FuseError::SerializationError(format!("Failed to parse file list: {}", e))
            })?;

        Ok(files)
    }

    /// Download a file from Hugging Face
    pub async fn download_file<F>(
        &self,
        url: &str,
        destination: &Path,
        auth: Option<&Auth>,
        resume: bool,
        expected_size: u64,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress) + Send,
    {
        info!("Downloading file from: {}", url);

        // Create parent directory if it doesn't exist
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut start_byte = 0u64;
        if resume && destination.exists() {
            start_byte = tokio::fs::metadata(destination).await.map(|m| m.len()).unwrap_or(0);
            if expected_size > 0 && start_byte >= expected_size {
                info!("File already fully downloaded: {}", destination.display());
                return Ok(());
            }
        }

        let mut request = self.client.get(url);

        if start_byte > 0 {
            request = request.header("Range", format!("bytes={}-", start_byte));
            info!("Resuming download from byte {}", start_byte);
        }

        // Add authentication if provided
        if let Some(auth) = auth {
            if let Some(header_value) = auth.to_header_value() {
                request = request.header("Authorization", header_value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("Failed to download file: {}", e)))?;

        let status = response.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(FuseError::DownloadError(format!(
                "Failed to download file: HTTP {}",
                status
            )));
        }

        // Check if server ignored the Range header and returned the full file (200 OK)
        let is_partial = status.as_u16() == 206;
        if start_byte > 0 && !is_partial {
            warn!("Server ignored Range request, downloading from scratch");
            start_byte = 0;
        }

        // Get total size
        let total_bytes = response.content_length().map(|len| start_byte + len);

        // Download with progress tracking
        let mut file = if start_byte > 0 && is_partial {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(destination)
                .await?
        } else {
            tokio::fs::File::create(destination).await?
        };
        let mut bytes_downloaded = start_byte;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| FuseError::NetworkError(format!("Failed to read chunk: {}", e)))?;

            file.write_all(&chunk).await?;
            bytes_downloaded += chunk.len() as u64;

            // Calculate speed and progress
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                bytes_downloaded as f64 / elapsed
            } else {
                0.0
            };

            let progress = DownloadProgress::new(bytes_downloaded, total_bytes, speed);
            progress_callback(progress);
        }

        file.flush().await?;

        info!("Downloaded {} bytes to {:?}", bytes_downloaded, destination);

        Ok(())
    }

    /// Download all model files
    pub async fn download_model<F>(
        &self,
        repository: &str,
        revision: Option<&str>,
        destination_dir: &Path,
        auth: Option<&Auth>,
        format: Option<&str>,
        resume: bool,
        mut progress_callback: F,
    ) -> Result<Vec<String>>
    where
        F: FnMut(&str, DownloadProgress) + Send,
    {
        info!("Downloading model {} to {:?}", repository, destination_dir);

        // Create destination directory
        tokio::fs::create_dir_all(destination_dir).await?;

        // List all files in the repository
        let files = self.list_files(repository, revision, auth).await?;

        // Filter for model files based on format
        let model_files: Vec<_> = files
            .into_iter()
            .filter(|f| {
                if f.file_type != "file" { return false; }

                let path_lower = f.path.to_lowercase();
                
                // Essential metadata files that are always needed (configs, tokenizers, vocabs)
                let is_metadata = path_lower.ends_with(".json") 
                    || path_lower.ends_with(".txt")
                    || path_lower.ends_with(".model");

                if let Some(fmt) = format {
                    let fmt_lower = fmt.to_lowercase();
                    // Map format names to extensions
                    let matches_format = match fmt_lower.as_str() {
                        "pytorch" | "pt" => path_lower.ends_with(".bin") || path_lower.ends_with(".pt") || path_lower.ends_with(".pth"),
                        "tensorrt" | "trt" => path_lower.ends_with(".engine") || path_lower.ends_with(".trt") || path_lower.ends_with(".plan"),
                        _ => path_lower.contains(&fmt_lower), // for gguf, safetensors, onnx
                    };
                    matches_format || is_metadata
                } else {
                    // Default behavior: download all recognized formats
                    path_lower.ends_with(".bin")
                        || path_lower.ends_with(".safetensors")
                        || path_lower.ends_with(".gguf")
                        || path_lower.ends_with(".onnx")
                        || path_lower.ends_with(".pt")
                        || path_lower.ends_with(".engine")
                        || is_metadata
                }
            })
            .collect();

        if model_files.is_empty() {
            warn!("No model files found in repository");
            return Err(FuseError::DownloadError(
                "No model files found in repository".to_string(),
            ));
        }

        // If a specific format was requested, ensure we found at least one non-metadata file matching it
        if format.is_some() {
            let has_format_file = model_files.iter().any(|f| {
                let path_lower = f.path.to_lowercase();
                !(path_lower.ends_with(".json") || path_lower.ends_with(".txt") || path_lower.ends_with(".model"))
            });
            if !has_format_file {
                warn!("No model weights found matching format: {:?}", format);
                return Err(FuseError::DownloadError(
                    format!("No model weights found matching requested format: {}", format.unwrap_or(""))
                ));
            }
        }

        info!("Found {} files to download", model_files.len());

        let mut downloaded_files = Vec::new();
        let revision = revision.unwrap_or("main");

        // Download each file
        for file_info in model_files {
            let dest_path = destination_dir.join(&file_info.path);
            let url = format!("{}/{}/resolve/{}/{}", self.base_url, repository, revision, file_info.path);

            info!("Downloading file: {}", file_info.path);

            self.download_file(&url, &dest_path, auth, resume, file_info.size, |progress| {
                progress_callback(&file_info.path, progress);
            })
            .await?;

            downloaded_files.push(file_info.path);
        }

        info!("Successfully downloaded {} files", downloaded_files.len());

        Ok(downloaded_files)
    }
}

impl Default for HuggingFaceClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_huggingface_client_creation() {
        let client = HuggingFaceClient::new();
        assert_eq!(client.base_url, "https://huggingface.co");
    }

    #[test]
    fn test_huggingface_model_info_serialization() {
        let info = HuggingFaceModelInfo {
            model_id: "gpt2".to_string(),
            sha: Some("abc123".to_string()),
            last_modified: Some("2024-01-01".to_string()),
            tags: Some(vec!["nlp".to_string()]),
            pipeline_tag: Some("text-generation".to_string()),
            library_name: Some("transformers".to_string()),
        };

        let serialized = serde_json::to_string(&info).unwrap();
        let deserialized: HuggingFaceModelInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.model_id, info.model_id);
        assert_eq!(deserialized.sha, info.sha);
    }

    #[test]
    fn test_huggingface_file_info_serialization() {
        let file_info = HuggingFaceFileInfo {
            path: "model.bin".to_string(),
            size: 1024,
            file_type: "file".to_string(),
            oid: Some("abc123".to_string()),
        };

        let serialized = serde_json::to_string(&file_info).unwrap();
        let deserialized: HuggingFaceFileInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.path, file_info.path);
        assert_eq!(deserialized.size, file_info.size);
        assert_eq!(deserialized.file_type, file_info.file_type);
    }
}
