use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Download state for resumable downloads
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadState {
    /// Download is pending
    Pending,
    /// Download is in progress
    InProgress {
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
        started_at: DateTime<Utc>,
    },
    /// Download is paused
    Paused {
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
        paused_at: DateTime<Utc>,
    },
    /// Download completed successfully
    Completed {
        bytes_downloaded: u64,
        completed_at: DateTime<Utc>,
    },
    /// Download failed
    Failed {
        error: String,
        bytes_downloaded: u64,
        failed_at: DateTime<Utc>,
        retry_count: u32,
    },
}

/// Download progress information
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percentage: Option<f64>,
    pub speed_bytes_per_sec: f64,
    pub eta_seconds: Option<u64>,
}

impl DownloadProgress {
    pub fn new(bytes_downloaded: u64, total_bytes: Option<u64>, speed_bytes_per_sec: f64) -> Self {
        let percentage = total_bytes.map(|total| {
            if total > 0 {
                (bytes_downloaded as f64 / total as f64) * 100.0
            } else {
                0.0
            }
        });

        let eta_seconds = total_bytes.and_then(|total| {
            if speed_bytes_per_sec > 0.0 && bytes_downloaded < total {
                let remaining = total - bytes_downloaded;
                Some((remaining as f64 / speed_bytes_per_sec) as u64)
            } else {
                None
            }
        });

        Self {
            bytes_downloaded,
            total_bytes,
            percentage,
            speed_bytes_per_sec,
            eta_seconds,
        }
    }
}

/// Download manager with pause/resume capability
pub struct DownloadManager {
    client: Client,
    state: Arc<RwLock<DownloadState>>,
    max_retries: u32,
}

impl DownloadManager {
    /// Create a new download manager
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            state: Arc::new(RwLock::new(DownloadState::Pending)),
            max_retries: 2,
        }
    }

    /// Download a file with resume capability
    pub async fn download_with_resume<F>(
        &self,
        url: &str,
        destination: &Path,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress) + Send,
    {
        let mut retry_count = 0;
        let mut bytes_downloaded = 0u64;

        // Check if partial download exists
        if destination.exists() {
            bytes_downloaded = tokio::fs::metadata(destination)
                .await
                .map(|m| m.len())
                .unwrap_or(0);

            if bytes_downloaded > 0 {
                info!("Resuming download from {} bytes", bytes_downloaded);
            }
        }

        loop {
            match self
                .download_chunk(url, destination, bytes_downloaded, &mut progress_callback)
                .await
            {
                Ok(()) => {
                    // Download completed successfully
                    let mut state = self.state.write().await;
                    *state = DownloadState::Completed {
                        bytes_downloaded,
                        completed_at: Utc::now(),
                    };
                    info!("Download completed successfully");
                    return Ok(());
                }
                Err(e) if e.is_retryable() && retry_count < self.max_retries => {
                    retry_count += 1;
                    warn!(
                        "Download failed (attempt {}/{}): {}",
                        retry_count, self.max_retries, e
                    );

                    // Update state to paused
                    let mut state = self.state.write().await;
                    *state = DownloadState::Paused {
                        bytes_downloaded,
                        total_bytes: None,
                        paused_at: Utc::now(),
                    };

                    // Wait before retry with exponential backoff
                    let wait_time = std::time::Duration::from_secs(2u64.pow(retry_count));
                    tokio::time::sleep(wait_time).await;

                    // Get current downloaded bytes
                    if destination.exists() {
                        bytes_downloaded = tokio::fs::metadata(destination)
                            .await
                            .map(|m| m.len())
                            .unwrap_or(0);
                    }
                }
                Err(e) if retry_count >= self.max_retries => {
                    // Max retries reached, ask user
                    let mut state = self.state.write().await;
                    *state = DownloadState::Failed {
                        error: e.to_string(),
                        bytes_downloaded,
                        failed_at: Utc::now(),
                        retry_count,
                    };

                    return Err(FuseError::DownloadError(format!(
                        "Download failed after {} retries. Use 'fuse pull --resume {}' to continue.",
                        self.max_retries, url
                    )));
                }
                Err(e) => {
                    // Non-retryable error
                    let mut state = self.state.write().await;
                    *state = DownloadState::Failed {
                        error: e.to_string(),
                        bytes_downloaded,
                        failed_at: Utc::now(),
                        retry_count,
                    };
                    return Err(e);
                }
            }
        }
    }

    /// Download a chunk of the file
    async fn download_chunk<F>(
        &self,
        url: &str,
        destination: &Path,
        start_byte: u64,
        progress_callback: &mut F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress) + Send,
    {
        // Create parent directory if it doesn't exist
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Build request with range header for resume
        let mut request = self.client.get(url);
        if start_byte > 0 {
            request = request.header("Range", format!("bytes={}-", start_byte));
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| FuseError::NetworkError(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(FuseError::DownloadError(format!(
                "Server returned error: {}",
                response.status()
            )));
        }

        // Get total size
        let total_bytes = response.content_length().map(|len| start_byte + len);

        // Update state to in progress
        {
            let mut state = self.state.write().await;
            *state = DownloadState::InProgress {
                bytes_downloaded: start_byte,
                total_bytes,
                started_at: Utc::now(),
            };
        }

        // Open file for writing (append mode if resuming)
        let mut file = if start_byte > 0 {
            OpenOptions::new().append(true).open(destination).await?
        } else {
            File::create(destination).await?
        };

        // Download with progress tracking
        let mut bytes_downloaded = start_byte;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| FuseError::NetworkError(format!("Failed to read chunk: {}", e)))?;

            file.write_all(&chunk).await?;
            bytes_downloaded += chunk.len() as u64;

            // Calculate speed and progress
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (bytes_downloaded - start_byte) as f64 / elapsed
            } else {
                0.0
            };

            let progress = DownloadProgress::new(bytes_downloaded, total_bytes, speed);
            progress_callback(progress);

            // Update state
            {
                let mut state = self.state.write().await;
                *state = DownloadState::InProgress {
                    bytes_downloaded,
                    total_bytes,
                    started_at: Utc::now(),
                };
            }
        }

        file.flush().await?;

        Ok(())
    }

    /// Get current download state
    pub async fn get_state(&self) -> DownloadState {
        self.state.read().await.clone()
    }

    /// Pause the download
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.state.write().await;

        match *state {
            DownloadState::InProgress {
                bytes_downloaded,
                total_bytes,
                ..
            } => {
                *state = DownloadState::Paused {
                    bytes_downloaded,
                    total_bytes,
                    paused_at: Utc::now(),
                };
                Ok(())
            }
            _ => Err(FuseError::DownloadError(
                "Download is not in progress".to_string(),
            )),
        }
    }

    /// Check if download can be resumed
    pub async fn can_resume(&self, destination: &Path) -> bool {
        if !destination.exists() {
            return false;
        }

        let state = self.state.read().await;
        matches!(
            *state,
            DownloadState::Paused { .. } | DownloadState::Failed { .. }
        )
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_download_state_serialization() {
        let state = DownloadState::InProgress {
            bytes_downloaded: 1024,
            total_bytes: Some(2048),
            started_at: Utc::now(),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: DownloadState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_download_progress_calculation() {
        let progress = DownloadProgress::new(512, Some(1024), 100.0);

        assert_eq!(progress.bytes_downloaded, 512);
        assert_eq!(progress.total_bytes, Some(1024));
        assert_eq!(progress.percentage, Some(50.0));
        assert_eq!(progress.speed_bytes_per_sec, 100.0);
        assert!(progress.eta_seconds.is_some());
    }

    #[test]
    fn test_download_progress_no_total() {
        let progress = DownloadProgress::new(512, None, 100.0);

        assert_eq!(progress.bytes_downloaded, 512);
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.percentage, None);
        assert_eq!(progress.eta_seconds, None);
    }

    #[test]
    fn test_download_progress_zero_speed() {
        let progress = DownloadProgress::new(512, Some(1024), 0.0);

        assert_eq!(progress.eta_seconds, None);
    }

    #[tokio::test]
    async fn test_download_manager_creation() {
        let manager = DownloadManager::new();
        let state = manager.get_state().await;

        assert!(matches!(state, DownloadState::Pending));
    }

    #[tokio::test]
    async fn test_download_manager_pause() {
        let manager = DownloadManager::new();

        // Set state to in progress
        {
            let mut state = manager.state.write().await;
            *state = DownloadState::InProgress {
                bytes_downloaded: 512,
                total_bytes: Some(1024),
                started_at: Utc::now(),
            };
        }

        // Pause download
        manager.pause().await.unwrap();

        let state = manager.get_state().await;
        assert!(matches!(state, DownloadState::Paused { .. }));
    }

    #[tokio::test]
    async fn test_download_manager_pause_not_in_progress() {
        let manager = DownloadManager::new();

        let result = manager.pause().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_can_resume_no_file() {
        let manager = DownloadManager::new();
        let path = PathBuf::from("/nonexistent/file");

        assert!(!manager.can_resume(&path).await);
    }
}
