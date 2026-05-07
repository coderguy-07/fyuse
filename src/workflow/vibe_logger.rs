use crate::error::Result;
use crate::workflow::ExecutionResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeEntry {
    pub timestamp: DateTime<Utc>,
    pub workflow_name: String,
    pub status: String,
    pub duration_ms: u64,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub details: String,
}

pub struct VibeLogger {
    vibe_dir: PathBuf,
}

impl VibeLogger {
    pub fn new(workspace_dir: &Path) -> Result<Self> {
        let vibe_dir = workspace_dir.join(".fuse/vibe");
        Ok(Self { vibe_dir })
    }
    
    pub async fn log_execution(&self, result: &ExecutionResult) -> Result<()> {
        fs::create_dir_all(&self.vibe_dir).await?;
        
        let entry = self.create_entry(result);
        let log_file = self.get_log_file().await?;
        
        let entry_json = serde_json::to_string_pretty(&entry)?;
        let mut content = format!("{}\n", entry_json);
        
        if log_file.exists() {
            let existing = fs::read_to_string(&log_file).await?;
            content = format!("{}{}", existing, content);
        }
        
        fs::write(&log_file, content).await?;
        
        info!("Logged workflow execution to {}", log_file.display());
        Ok(())
    }
    
    fn create_entry(&self, result: &ExecutionResult) -> VibeEntry {
        let duration_ms = result.completed_at
            .map(|end| (end - result.started_at).num_milliseconds() as u64)
            .unwrap_or(0);
        
        let steps_completed = result.steps.iter()
            .filter(|s| matches!(s.status, crate::workflow::executor::StepStatus::Completed))
            .count();
        
        let steps_failed = result.steps.iter()
            .filter(|s| matches!(s.status, crate::workflow::executor::StepStatus::Failed))
            .count();
        
        let status = format!("{:?}", result.status);
        let details = format!("Completed {} steps, {} failed", steps_completed, steps_failed);
        
        VibeEntry {
            timestamp: result.started_at,
            workflow_name: result.workflow_name.clone(),
            status,
            duration_ms,
            steps_completed,
            steps_failed,
            details,
        }
    }
    
    async fn get_log_file(&self) -> Result<PathBuf> {
        let now = Utc::now();
        let filename = format!("vibe_{}.jsonl", now.format("%Y%m%d"));
        Ok(self.vibe_dir.join(filename))
    }
    
    pub async fn get_recent_entries(&self, limit: usize) -> Result<Vec<VibeEntry>> {
        let log_file = self.get_log_file().await?;
        
        if !log_file.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&log_file).await?;
        let mut entries = Vec::new();
        
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<VibeEntry>(line) {
                entries.push(entry);
            }
        }
        
        entries.truncate(limit);
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::workflow::{ExecutionStatus, ExecutionContext};

    #[tokio::test]
    async fn test_vibe_logger() {
        let temp_dir = TempDir::new().unwrap();
        let logger = VibeLogger::new(temp_dir.path()).unwrap();
        
        let result = ExecutionResult {
            workflow_name: "test".to_string(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: ExecutionStatus::Completed,
            steps: Vec::new(),
            context: ExecutionContext::default(),
        };
        
        logger.log_execution(&result).await.unwrap();
        
        let entries = logger.get_recent_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].workflow_name, "test");
    }
}
