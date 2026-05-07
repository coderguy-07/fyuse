pub mod discovery;
pub mod executor;
pub mod history;
pub mod parser;
pub mod state;

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<WorkflowStep>,
    pub max_iterations: usize,
    pub timeout_secs: u64,
    pub parallel_execution: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub action: WorkflowAction,
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
    pub retry_policy: RetryPolicy,
    pub depends_on: Vec<String>, // For parallel execution dependencies
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowAction {
    Compile,
    Test,
    Fix {
        error_context: String,
    },
    Execute {
        command: String,
    },
    RunInference {
        model: String,
        prompt: String,
    },
    Quantize {
        model: String,
        method: String,
    },
    Scan {
        target: String,
    },
    Merge {
        models: Vec<String>,
        strategy: String,
    },
    Custom {
        command: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub backoff_secs: u64,
    pub exponential_backoff: bool,
    pub max_backoff_secs: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_secs: 1,
            exponential_backoff: true,
            max_backoff_secs: 60,
        }
    }
}

pub struct WorkflowService {
    workflow_dir: PathBuf,
    state_manager: Arc<state::WorkflowStateManager>,
    history_manager: history::WorkflowHistoryManager,
}

impl WorkflowService {
    pub fn new(workflow_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            workflow_dir: workflow_dir.clone(),
            state_manager: Arc::new(state::WorkflowStateManager::new(
                workflow_dir.join(".fuse/workflows/state"),
            )?),
            history_manager: history::WorkflowHistoryManager::new(
                workflow_dir.join(".fuse/workflows/history"),
            )?,
        })
    }

    pub async fn discover_workflow(&self) -> Result<Option<PathBuf>> {
        discovery::discover_workflow_file(&self.workflow_dir).await
    }

    pub async fn parse_workflow(&self, path: &Path) -> Result<Workflow> {
        parser::parse_workflow(path).await
    }

    pub async fn execute_workflow(&self, workflow: &Workflow) -> Result<WorkflowResult> {
        let execution_id = uuid::Uuid::new_v4().to_string();
        let start_time = std::time::Instant::now();

        // Initialize execution state
        self.state_manager
            .initialize_execution(&execution_id, workflow)
            .await?;

        // Execute workflow using orchestrator
        let orchestrator = executor::WorkflowOrchestrator::new(Arc::clone(&self.state_manager));
        let result = orchestrator.execute_workflow(workflow).await;

        // Record execution result
        let duration = start_time.elapsed();
        self.history_manager
            .record_execution(&execution_id, workflow, &result, duration)
            .await?;

        // Clean up state
        self.state_manager.cleanup_execution(&execution_id).await?;

        result
    }

    pub async fn get_workflow_state(
        &self,
        execution_id: &str,
    ) -> Result<Option<state::WorkflowExecutionState>> {
        self.state_manager.get_execution_state(execution_id).await
    }

    pub async fn list_executions(
        &self,
        limit: usize,
    ) -> Result<Vec<history::WorkflowExecutionRecord>> {
        self.history_manager.list_executions(limit).await
    }

    pub async fn get_execution_history(
        &self,
        workflow_name: &str,
    ) -> Result<Vec<history::WorkflowExecutionRecord>> {
        self.history_manager
            .get_workflow_history(workflow_name)
            .await
    }

    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        self.state_manager.cancel_execution(execution_id).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub success: bool,
    pub steps_executed: usize,
    pub errors: Vec<String>,
    pub duration_secs: u64,
}
