use crate::error::{FuseError, Result};
use crate::model::Auth;
use crate::storage::DownloadProgress;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

/// Unsloth API client
pub struct UnslothClient {
    client: Client,
    base_url: String,
}

/// Unsloth model info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnslothModelInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub size: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub architecture: Option<String>,
    pub parameters: Option<usize>,
}

/// Unsloth file info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnslothFileInfo {
    pub name: String,
    pub size: u64,
    pub url: String,
}

impl UnslothClient {
    /// Create a new Unsloth client
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            base_url: "https://unsloth.ai".to_string(),
        }
    }

    /// Get model information
    pub async fn get_model_info(
        &self,
        model_id: &str,
        auth: Option<&Auth>,
    ) -> Result<UnslothModelInfo> {
        let url = format!("{}/api/models/{}", self.base_url, model_id);

        let mut request = self.client.get(&url);

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

        let model_info = response.json::<UnslothModelInfo>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse model info: {}", e))
        })?;

        Ok(model_info)
    }

    /// List files for a model
    pub async fn list_files(
        &self,
        model_id: &str,
        auth: Option<&Auth>,
    ) -> Result<Vec<UnslothFileInfo>> {
        let url = format!("{}/api/models/{}/files", self.base_url, model_id);

        let mut request = self.client.get(&url);

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

        let files = response.json::<Vec<UnslothFileInfo>>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse file list: {}", e))
        })?;

        Ok(files)
    }

    /// Download a file from Unsloth
    pub async fn download_file<F>(
        &self,
        url: &str,
        destination: &Path,
        auth: Option<&Auth>,
        resume: bool,
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

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(FuseError::DownloadError(format!(
                "Failed to download file: HTTP {}",
                response.status()
            )));
        }

        // Get total size
        let total_bytes = response.content_length().map(|len| start_byte + len);

        // Download with progress tracking
        let mut file = if start_byte > 0 {
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
        model_id: &str,
        destination_dir: &Path,
        auth: Option<&Auth>,
        format: Option<&str>,
        resume: bool,
        mut progress_callback: F,
    ) -> Result<Vec<String>>
    where
        F: FnMut(&str, DownloadProgress) + Send,
    {
        info!("Downloading model {} to {:?}", model_id, destination_dir);

        // Create destination directory
        tokio::fs::create_dir_all(destination_dir).await?;

        // List all files for the model
        let all_files = self.list_files(model_id, auth).await?;

        // Filter files by format
        let files: Vec<_> = all_files
            .into_iter()
            .filter(|f| {
                let path_lower = f.name.to_lowercase();
                
                let is_metadata = path_lower.ends_with(".json") 
                    || path_lower.ends_with(".txt")
                    || path_lower.ends_with(".model");

                if let Some(fmt) = format {
                    let fmt_lower = fmt.to_lowercase();
                    let matches_format = match fmt_lower.as_str() {
                        "pytorch" | "pt" => path_lower.ends_with(".bin") || path_lower.ends_with(".pt") || path_lower.ends_with(".pth"),
                        "tensorrt" | "trt" => path_lower.ends_with(".engine") || path_lower.ends_with(".trt") || path_lower.ends_with(".plan"),
                        _ => path_lower.contains(&fmt_lower),
                    };
                    matches_format || is_metadata
                } else {
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

        if files.is_empty() {
            warn!("No model files found matching criteria");
            return Err(FuseError::DownloadError("No model files found".to_string()));
        }

        info!("Found {} files to download", files.len());

        let mut downloaded_files = Vec::new();

        // Download each file
        for file_info in files {
            let file_destination = destination_dir.join(&file_info.name);

            info!("Downloading file: {}", file_info.name);

            self.download_file(&file_info.url, &file_destination, auth, resume, |progress| {
                progress_callback(&file_info.name, progress)
            })
            .await?;

            downloaded_files.push(file_info.name);
        }

        info!("Successfully downloaded {} files", downloaded_files.len());

        Ok(downloaded_files)
    }
}

impl Default for UnslothClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsloth_client_creation() {
        let client = UnslothClient::new();
        assert_eq!(client.base_url, "https://unsloth.ai");
    }

    #[test]
    fn test_unsloth_model_info_serialization() {
        let info = UnslothModelInfo {
            id: "llama-3".to_string(),
            name: "Llama 3".to_string(),
            version: Some("1.0".to_string()),
            size: Some(7_000_000_000),
            tags: Some(vec!["nlp".to_string()]),
            architecture: Some("llama".to_string()),
            parameters: Some(7_000_000_000),
        };

        let serialized = serde_json::to_string(&info).unwrap();
        let deserialized: UnslothModelInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, info.id);
        assert_eq!(deserialized.name, info.name);
        assert_eq!(deserialized.version, info.version);
    }

    #[test]
    fn test_unsloth_file_info_serialization() {
        let file_info = UnslothFileInfo {
            name: "model.bin".to_string(),
            size: 1024,
            url: "https://example.com/model.bin".to_string(),
        };

        let serialized = serde_json::to_string(&file_info).unwrap();
        let deserialized: UnslothFileInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.name, file_info.name);
        assert_eq!(deserialized.size, file_info.size);
        assert_eq!(deserialized.url, file_info.url);
    }
}
