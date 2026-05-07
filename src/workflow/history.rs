use crate::error::Result;
use crate::workflow::{Workflow, WorkflowResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionRecord {
    pub execution_id: String,
    pub workflow_name: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: u64,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub errors: Vec<String>,
    pub metadata: serde_json::Value,
}

pub struct WorkflowHistoryManager {
    history_dir: PathBuf,
    #[allow(dead_code)]
    max_records: usize,
}

impl WorkflowHistoryManager {
    pub fn new(history_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&history_dir)?;
        Ok(Self {
            history_dir,
            max_records: 1000, // Configurable in future
        })
    }

    pub async fn record_execution(
        &self,
        execution_id: &str,
        workflow: &Workflow,
        result: &Result<WorkflowResult>,
        duration: Duration,
    ) -> Result<()> {
        let record = match result {
            Ok(workflow_result) => WorkflowExecutionRecord {
                execution_id: execution_id.to_string(),
                workflow_name: workflow.name.clone(),
                status: if workflow_result.success {
                    "success"
                } else {
                    "failed"
                }
                .to_string(),
                started_at: Utc::now() - chrono::Duration::from_std(duration).unwrap_or_default(),
                completed_at: Some(Utc::now()),
                duration_ms: duration.as_millis() as u64,
                steps_completed: workflow_result.steps_executed,
                steps_failed: workflow_result.errors.len(),
                errors: workflow_result.errors.clone(),
                metadata: serde_json::json!({
                    "max_iterations": workflow.max_iterations,
                    "timeout_secs": workflow.timeout_secs,
                    "parallel_execution": workflow.parallel_execution
                }),
            },
            Err(e) => WorkflowExecutionRecord {
                execution_id: execution_id.to_string(),
                workflow_name: workflow.name.clone(),
                status: "error".to_string(),
                started_at: Utc::now() - chrono::Duration::from_std(duration).unwrap_or_default(),
                completed_at: Some(Utc::now()),
                duration_ms: duration.as_millis() as u64,
                steps_completed: 0,
                steps_failed: 1,
                errors: vec![e.to_string()],
                metadata: serde_json::json!({
                    "error_type": "execution_error",
                    "max_iterations": workflow.max_iterations,
                    "timeout_secs": workflow.timeout_secs
                }),
            },
        };

        // Save to daily log file
        self.save_to_daily_log(&record).await?;

        // Update workflow summary
        self.update_workflow_summary(&record).await?;

        info!(
            "Recorded workflow execution: {} ({})",
            execution_id, record.status
        );
        Ok(())
    }

    pub async fn list_executions(&self, limit: usize) -> Result<Vec<WorkflowExecutionRecord>> {
        let mut all_records = Vec::new();

        // Read from recent daily log files
        let mut entries = fs::read_dir(&self.history_dir).await?;
        let mut log_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "jsonl" {
                    log_files.push(entry.path());
                }
            }
        }

        // Sort by modification time (newest first)
        log_files.sort_by(|a, b| {
            b.metadata()
                .unwrap()
                .modified()
                .unwrap()
                .cmp(&a.metadata().unwrap().modified().unwrap())
        });

        // Read from most recent files
        for log_file in log_files.iter().take(7) {
            // Last 7 days
            if let Ok(content) = fs::read_to_string(log_file).await {
                for line in content.lines() {
                    if let Ok(record) = serde_json::from_str::<WorkflowExecutionRecord>(line) {
                        all_records.push(record);
                        if all_records.len() >= limit {
                            break;
                        }
                    }
                }
            }
            if all_records.len() >= limit {
                break;
            }
        }

        all_records.truncate(limit);
        Ok(all_records)
    }

    pub async fn get_workflow_history(
        &self,
        workflow_name: &str,
    ) -> Result<Vec<WorkflowExecutionRecord>> {
        let mut records = Vec::new();

        // Read from summary file first
        let summary_file = self
            .history_dir
            .join("workflows")
            .join(format!("{}.json", workflow_name));
        if summary_file.exists() {
            if let Ok(content) = fs::read_to_string(&summary_file).await {
                if let Ok(summary) = serde_json::from_str::<WorkflowSummary>(&content) {
                    records.extend(summary.recent_executions);
                }
            }
        }

        // Also search through daily logs for completeness
        let mut entries = fs::read_dir(&self.history_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "jsonl" {
                    if let Ok(content) = fs::read_to_string(&entry.path()).await {
                        for line in content.lines() {
                            if let Ok(record) =
                                serde_json::from_str::<WorkflowExecutionRecord>(line)
                            {
                                if record.workflow_name == workflow_name {
                                    records.push(record);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by start time (newest first)
        records.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(records)
    }

    pub async fn get_execution_stats(&self) -> Result<ExecutionStats> {
        let records = self.list_executions(10000).await?; // Get all recent records

        let total_executions = records.len();
        let successful_executions = records.iter().filter(|r| r.status == "success").count();
        let failed_executions = records
            .iter()
            .filter(|r| r.status == "failed" || r.status == "error")
            .count();

        let avg_duration = if !records.is_empty() {
            records.iter().map(|r| r.duration_ms).sum::<u64>() / records.len() as u64
        } else {
            0
        };

        let workflows: std::collections::HashSet<String> =
            records.iter().map(|r| r.workflow_name.clone()).collect();

        Ok(ExecutionStats {
            total_executions,
            successful_executions,
            failed_executions,
            average_duration_ms: avg_duration,
            unique_workflows: workflows.len(),
        })
    }

    async fn save_to_daily_log(&self, record: &WorkflowExecutionRecord) -> Result<()> {
        let today = record.started_at.format("%Y%m%d");
        let log_file = self.history_dir.join(format!("executions_{}.jsonl", today));

        let json_line = serde_json::to_string(record)?;
        let content = format!("{}\n", json_line);

        // Append to file
        if log_file.exists() {
            let existing = fs::read_to_string(&log_file).await?;
            fs::write(&log_file, format!("{}{}", existing, content)).await?;
        } else {
            fs::write(&log_file, content).await?;
        }

        Ok(())
    }

    async fn update_workflow_summary(&self, record: &WorkflowExecutionRecord) -> Result<()> {
        let workflows_dir = self.history_dir.join("workflows");
        fs::create_dir_all(&workflows_dir).await?;

        let summary_file = workflows_dir.join(format!("{}.json", record.workflow_name));
        let mut summary = if summary_file.exists() {
            let content = fs::read_to_string(&summary_file).await?;
            serde_json::from_str::<WorkflowSummary>(&content).unwrap_or_default()
        } else {
            WorkflowSummary::default()
        };

        // Update stats
        summary.total_executions += 1;
        if record.status == "success" {
            summary.successful_executions += 1;
        } else {
            summary.failed_executions += 1;
        }

        summary.last_execution = Some(record.started_at);
        summary.average_duration_ms = if summary.average_duration_ms == 0 {
            record.duration_ms
        } else {
            (summary.average_duration_ms + record.duration_ms) / 2
        };

        // Keep recent executions (last 10)
        summary.recent_executions.insert(0, record.clone());
        summary.recent_executions.truncate(10);

        let json = serde_json::to_string_pretty(&summary)?;
        fs::write(&summary_file, json).await?;

        Ok(())
    }

    pub async fn cleanup_old_logs(&self, days_to_keep: u32) -> Result<usize> {
        let mut deleted_count = 0;
        let cutoff_date = Utc::now() - chrono::Duration::days(days_to_keep as i64);

        let mut entries = fs::read_dir(&self.history_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "jsonl" {
                    if let Some(filename) = entry.path().file_stem() {
                        if let Some(date_str) = filename.to_str() {
                            if let Some(stripped) = date_str.strip_prefix("executions_") {
                                if let Ok(file_date) =
                                    chrono::NaiveDate::parse_from_str(stripped, "%Y%m%d")
                                {
                                    let file_datetime = DateTime::<Utc>::from_naive_utc_and_offset(
                                        file_date.and_hms_opt(0, 0, 0).unwrap(),
                                        Utc,
                                    );

                                    if file_datetime < cutoff_date {
                                        fs::remove_file(&entry.path()).await?;
                                        deleted_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if deleted_count > 0 {
            info!("Cleaned up {} old log files", deleted_count);
        }

        Ok(deleted_count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkflowSummary {
    pub workflow_name: String,
    pub total_executions: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub average_duration_ms: u64,
    pub last_execution: Option<DateTime<Utc>>,
    pub recent_executions: Vec<WorkflowExecutionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total_executions: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub average_duration_ms: u64,
    pub unique_workflows: usize,
}
