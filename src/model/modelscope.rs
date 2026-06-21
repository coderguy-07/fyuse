use crate::error::{FuseError, Result};
use crate::model::Auth;
use crate::storage::DownloadProgress;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

const MODELSCOPE_BASE: &str = "https://www.modelscope.cn";

/// ModelScope API client
pub struct ModelScopeClient {
    client: Client,
    base_url: String,
}

/// ModelScope model info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeModelInfo {
    #[serde(rename = "ModelId")]
    pub model_id: String,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "Tags")]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "TaskName")]
    pub task_name: Option<String>,
}

/// ModelScope file entry from tree API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeFileInfo {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Size")]
    pub size: u64,
    #[serde(rename = "Type")]
    pub file_type: String, // "blob" = file, "tree" = directory
}

/// ModelScope tree API response wrapper
#[derive(Debug, Deserialize)]
struct TreeResponse {
    #[serde(rename = "Data")]
    data: TreeData,
}

#[derive(Debug, Deserialize)]
struct TreeData {
    #[serde(rename = "Files")]
    files: Vec<ModelScopeFileInfo>,
}

impl ModelScopeClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
            base_url: MODELSCOPE_BASE.to_string(),
        }
    }

    fn auth_header(auth: Option<&Auth>) -> Option<String> {
        auth?.to_header_value()
    }

    /// Get model information from ModelScope
    pub async fn get_model_info(
        &self,
        repository: &str,
        auth: Option<&Auth>,
    ) -> Result<ModelScopeModelInfo> {
        let url = format!("{}/api/v1/models/{}", self.base_url, repository);
        let mut req = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30));
        if let Some(hv) = Self::auth_header(auth) {
            req = req.header("Authorization", hv);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("ModelScope model info failed: {}", e)))?;
        if !resp.status().is_success() {
            return Err(FuseError::DownloadError(format!(
                "ModelScope model info HTTP {}",
                resp.status()
            )));
        }
        resp.json::<ModelScopeModelInfo>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse ModelScope model info: {}", e))
        })
    }

    /// List files in a ModelScope repository
    pub async fn list_files(
        &self,
        repository: &str,
        revision: Option<&str>,
        auth: Option<&Auth>,
    ) -> Result<Vec<ModelScopeFileInfo>> {
        let revision = revision.unwrap_or("master");
        let url = format!(
            "{}/api/v1/models/{}/repo/tree?revision={}",
            self.base_url, repository, revision
        );
        let mut req = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30));
        if let Some(hv) = Self::auth_header(auth) {
            req = req.header("Authorization", hv);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("ModelScope list files failed: {}", e)))?;
        if !resp.status().is_success() {
            return Err(FuseError::DownloadError(format!(
                "ModelScope list files HTTP {}",
                resp.status()
            )));
        }
        let tree = resp.json::<TreeResponse>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse ModelScope file tree: {}", e))
        })?;
        Ok(tree.data.files)
    }

    /// Download a single file from ModelScope
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
        info!("Downloading ModelScope file from: {}", url);

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut start_byte = 0u64;
        if resume && destination.exists() {
            start_byte = tokio::fs::metadata(destination)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            if expected_size > 0 && start_byte >= expected_size {
                info!("File already fully downloaded: {}", destination.display());
                return Ok(());
            }
        }

        let mut req = self.client.get(url);
        if start_byte > 0 {
            req = req.header("Range", format!("bytes={}-", start_byte));
        }
        if let Some(hv) = Self::auth_header(auth) {
            req = req.header("Authorization", hv);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("ModelScope download failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(FuseError::DownloadError(format!(
                "ModelScope download HTTP {}",
                status
            )));
        }

        let is_partial = status.as_u16() == 206;
        if start_byte > 0 && !is_partial {
            warn!("Server ignored Range request, downloading from scratch");
            start_byte = 0;
        }

        let total_bytes = resp.content_length().map(|len| start_byte + len);
        let mut file = if start_byte > 0 && is_partial {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(destination)
                .await?
        } else {
            tokio::fs::File::create(destination).await?
        };

        let mut bytes_downloaded = start_byte;
        let mut stream = resp.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| FuseError::NetworkError(format!("Chunk read failed: {}", e)))?;
            file.write_all(&chunk).await?;
            bytes_downloaded += chunk.len() as u64;
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                bytes_downloaded as f64 / elapsed
            } else {
                0.0
            };
            progress_callback(DownloadProgress::new(bytes_downloaded, total_bytes, speed));
        }
        file.flush().await?;
        info!("Downloaded {} bytes to {:?}", bytes_downloaded, destination);
        Ok(())
    }

    /// Download all model files from ModelScope with smart GGUF selection
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
        info!("Downloading ModelScope model {} to {:?}", repository, destination_dir);

        tokio::fs::create_dir_all(destination_dir).await?;

        let all_files = self.list_files(repository, revision, auth).await?;

        // Smart GGUF selection when no explicit format is requested
        let smart_winner: Option<String> = if format.is_none() {
            let hw = crate::platform::hardware::HardwareProfiler::new().detect();
            let vram = hw.gpu.as_ref().and_then(|g| g.vram_bytes);
            let ram_budget = hw.available_ram_bytes.saturating_sub(2 * 1024 * 1024 * 1024);
            let candidates: Vec<_> = all_files
                .iter()
                .filter(|f| f.file_type == "blob")
                .map(|f| crate::model::format_selector::FileCandidate {
                    name: f.name.clone(),
                    size: f.size,
                })
                .collect();
            let selected =
                crate::model::format_selector::select_best_gguf(&candidates, ram_budget, vram);
            if let Some(ref winner) = selected {
                info!(
                    "Smart GGUF selection: {} (RAM budget {}MB)",
                    winner,
                    ram_budget / 1_048_576
                );
            }
            selected
        } else {
            None
        };

        let model_files: Vec<_> = all_files
            .into_iter()
            .filter(|f| {
                if f.file_type != "blob" {
                    return false;
                }
                let name_lower = f.name.to_lowercase();
                let is_metadata = name_lower.ends_with(".json")
                    || name_lower.ends_with(".txt")
                    || name_lower.ends_with(".model");

                if let Some(ref winner) = smart_winner {
                    f.name == *winner || is_metadata
                } else if let Some(fmt) = format {
                    let fmt_lower = fmt.to_lowercase();
                    let matches_format = match fmt_lower.as_str() {
                        "pytorch" | "pt" => {
                            name_lower.ends_with(".bin")
                                || name_lower.ends_with(".pt")
                                || name_lower.ends_with(".pth")
                        }
                        "tensorrt" | "trt" => {
                            name_lower.ends_with(".engine")
                                || name_lower.ends_with(".trt")
                                || name_lower.ends_with(".plan")
                        }
                        _ => name_lower.contains(&fmt_lower),
                    };
                    matches_format || is_metadata
                } else {
                    name_lower.ends_with(".bin")
                        || name_lower.ends_with(".safetensors")
                        || name_lower.ends_with(".gguf")
                        || name_lower.ends_with(".onnx")
                        || name_lower.ends_with(".pt")
                        || name_lower.ends_with(".engine")
                        || is_metadata
                }
            })
            .collect();

        if model_files.is_empty() {
            warn!("No model files found in ModelScope repository");
            return Err(FuseError::DownloadError(
                "No model files found in repository".to_string(),
            ));
        }

        if format.is_some() {
            let has_weights = model_files.iter().any(|f| {
                let n = f.name.to_lowercase();
                !(n.ends_with(".json") || n.ends_with(".txt") || n.ends_with(".model"))
            });
            if !has_weights {
                return Err(FuseError::DownloadError(format!(
                    "No model weights found matching requested format: {}",
                    format.unwrap_or("")
                )));
            }
        }

        // Disk space check
        let total_required: u64 = model_files.iter().map(|f| f.size).sum();
        let available_disk =
            crate::platform::hardware::HardwareProfiler::available_disk_bytes(destination_dir);
        if total_required > available_disk {
            return Err(crate::error::FuseError::InsufficientDiskSpace {
                required_gb: total_required as f64 / 1e9,
                available_gb: available_disk as f64 / 1e9,
            });
        }

        info!(
            "Found {} files to download ({:.2}GB total)",
            model_files.len(),
            total_required as f64 / 1e9
        );

        let revision = revision.unwrap_or("master");
        let mut downloaded_files = Vec::new();

        for file_info in model_files {
            let dest_path = destination_dir.join(&file_info.name);
            let url = format!(
                "{}/models/{}/resolve/{}/{}",
                self.base_url, repository, revision, file_info.name
            );
            info!("Downloading file: {}", file_info.name);
            self.download_file(&url, &dest_path, auth, resume, file_info.size, |progress| {
                progress_callback(&file_info.name, progress);
            })
            .await?;
            downloaded_files.push(file_info.name);
        }

        info!("Successfully downloaded {} files", downloaded_files.len());
        Ok(downloaded_files)
    }
}

impl Default for ModelScopeClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modelscope_client_creation() {
        let client = ModelScopeClient::new();
        assert_eq!(client.base_url, MODELSCOPE_BASE);
    }

    #[test]
    fn test_modelscope_file_info_serialization() {
        let info = ModelScopeFileInfo {
            name: "model.gguf".to_string(),
            size: 4_000_000_000,
            file_type: "blob".to_string(),
        };
        let serialized = serde_json::to_string(&info).unwrap();
        let deserialized: ModelScopeFileInfo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, info.name);
        assert_eq!(deserialized.size, info.size);
        assert_eq!(deserialized.file_type, info.file_type);
    }
}
