use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionState {
    pub execution_id: String,
    pub workflow_name: String,
    pub status: ExecutionStatus,
    pub current_step: Option<String>,
    pub completed_steps: Vec<String>,
    pub failed_steps: Vec<String>,
    pub step_results: HashMap<String, StepResult>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

pub struct WorkflowStateManager {
    state_dir: PathBuf,
    executions: RwLock<HashMap<String, WorkflowExecutionState>>,
}

impl WorkflowStateManager {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&state_dir)?;
        Ok(Self {
            state_dir,
            executions: RwLock::new(HashMap::new()),
        })
    }

    pub async fn initialize_execution(
        &self,
        execution_id: &str,
        workflow: &super::Workflow,
    ) -> Result<()> {
        let mut state = WorkflowExecutionState {
            execution_id: execution_id.to_string(),
            workflow_name: workflow.name.clone(),
            status: ExecutionStatus::Pending,
            current_step: None,
            completed_steps: Vec::new(),
            failed_steps: Vec::new(),
            step_results: HashMap::new(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            cancelled_at: None,
        };

        // Initialize step results
        for step in &workflow.steps {
            state.step_results.insert(
                step.id.clone(),
                StepResult {
                    step_id: step.id.clone(),
                    status: StepStatus::Pending,
                    output: None,
                    error: None,
                    started_at: Utc::now(),
                    completed_at: None,
                    retry_count: 0,
                },
            );
        }

        let mut executions = self.executions.write().await;
        executions.insert(execution_id.to_string(), state);

        // Persist to disk
        self.persist_execution_state(execution_id).await?;

        info!("Initialized workflow execution: {}", execution_id);
        Ok(())
    }

    pub async fn update_step_status(
        &self,
        execution_id: &str,
        step_id: &str,
        status: StepStatus,
        output: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions.get_mut(execution_id).ok_or_else(|| {
            FuseError::WorkflowError(format!("Execution not found: {}", execution_id))
        })?;

        if let Some(step_result) = execution.step_results.get_mut(step_id) {
            step_result.status = status.clone();
            step_result.output = output;
            step_result.error = error.clone();
            step_result.completed_at = Some(Utc::now());

            match status {
                StepStatus::Completed => {
                    execution.completed_steps.push(step_id.to_string());
                }
                StepStatus::Failed => {
                    execution.failed_steps.push(step_id.to_string());
                }
                _ => {}
            }
        }

        execution.updated_at = Utc::now();

        // Persist changes
        self.persist_execution_state(execution_id).await?;

        debug!("Updated step {} status to {:?}", step_id, status);
        Ok(())
    }

    pub async fn set_current_step(
        &self,
        execution_id: &str,
        step_id: Option<String>,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions.get_mut(execution_id).ok_or_else(|| {
            FuseError::WorkflowError(format!("Execution not found: {}", execution_id))
        })?;

        execution.current_step = step_id;
        execution.updated_at = Utc::now();

        self.persist_execution_state(execution_id).await?;
        Ok(())
    }

    pub async fn set_execution_status(
        &self,
        execution_id: &str,
        status: ExecutionStatus,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions.get_mut(execution_id).ok_or_else(|| {
            FuseError::WorkflowError(format!("Execution not found: {}", execution_id))
        })?;

        execution.status = status.clone();
        execution.updated_at = Utc::now();

        if matches!(status, ExecutionStatus::Cancelled) {
            execution.cancelled_at = Some(Utc::now());
        }

        self.persist_execution_state(execution_id).await?;
        Ok(())
    }

    pub async fn get_execution_state(
        &self,
        execution_id: &str,
    ) -> Result<Option<WorkflowExecutionState>> {
        let executions = self.executions.read().await;
        Ok(executions.get(execution_id).cloned())
    }

    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        self.set_execution_status(execution_id, ExecutionStatus::Cancelled)
            .await?;
        info!("Cancelled workflow execution: {}", execution_id);
        Ok(())
    }

    pub async fn cleanup_execution(&self, execution_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        if executions.remove(execution_id).is_some() {
            // Remove persisted state file
            let state_file = self.get_state_file_path(execution_id);
            if state_file.exists() {
                tokio::fs::remove_file(&state_file).await?;
            }
            debug!("Cleaned up execution state: {}", execution_id);
        }
        Ok(())
    }

    pub async fn list_active_executions(&self) -> Result<Vec<WorkflowExecutionState>> {
        let executions = self.executions.read().await;
        Ok(executions
            .values()
            .filter(|e| {
                matches!(
                    e.status,
                    ExecutionStatus::Pending | ExecutionStatus::Running
                )
            })
            .cloned()
            .collect())
    }

    async fn persist_execution_state(&self, execution_id: &str) -> Result<()> {
        let executions = self.executions.read().await;
        let execution = executions.get(execution_id).ok_or_else(|| {
            FuseError::WorkflowError(format!("Execution not found: {}", execution_id))
        })?;

        let state_file = self.get_state_file_path(execution_id);
        let json = serde_json::to_string_pretty(execution)?;
        fs::write(&state_file, json).await?;

        Ok(())
    }

    async fn load_execution_state(
        &self,
        execution_id: &str,
    ) -> Result<Option<WorkflowExecutionState>> {
        let state_file = self.get_state_file_path(execution_id);
        if !state_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&state_file).await?;
        let state: WorkflowExecutionState = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    fn get_state_file_path(&self, execution_id: &str) -> PathBuf {
        self.state_dir.join(format!("{}.json", execution_id))
    }

    pub async fn recover_executions(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.state_dir).await?;
        let mut recovered_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "json" {
                    if let Some(filename) = entry.path().file_stem() {
                        if let Some(execution_id) = filename.to_str() {
                            if let Some(state) = self.load_execution_state(execution_id).await? {
                                let mut executions = self.executions.write().await;
                                executions.insert(execution_id.to_string(), state);
                                recovered_count += 1;
                            }
                        }
                    }
                }
            }
        }

        if recovered_count > 0 {
            info!(
                "Recovered {} workflow executions from disk",
                recovered_count
            );
        }

        Ok(())
    }
}
