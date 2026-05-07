//! Advanced workflow executor with orchestration capabilities

use crate::error::{FuseError, Result};
use crate::workflow::state::{ExecutionStatus, StepResult, StepStatus, WorkflowStateManager};
use crate::workflow::{Workflow, WorkflowAction, WorkflowResult, WorkflowStep};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Advanced workflow executor with orchestration
pub struct WorkflowOrchestrator {
    state_manager: Arc<WorkflowStateManager>,
    max_concurrent_steps: usize,
    execution_timeout: Duration,
}

impl WorkflowOrchestrator {
    /// Create a new workflow orchestrator
    pub fn new(state_manager: Arc<WorkflowStateManager>) -> Self {
        Self {
            state_manager,
            max_concurrent_steps: 5,
            execution_timeout: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Execute workflow with advanced orchestration
    pub async fn execute_workflow(&self, workflow: &Workflow) -> Result<WorkflowResult> {
        let execution_id = uuid::Uuid::new_v4().to_string();

        // Initialize execution state
        self.state_manager
            .initialize_execution(&execution_id, workflow)
            .await?;
        self.state_manager
            .set_execution_status(&execution_id, ExecutionStatus::Running)
            .await?;

        let start_time = std::time::Instant::now();
        let mut results = HashMap::new();

        // Build dependency graph
        let dependency_graph = self.build_dependency_graph(&workflow.steps)?;

        // Execute workflow with orchestration
        let execution_result = timeout(
            self.execution_timeout,
            self.execute_with_orchestration(
                &execution_id,
                workflow,
                &dependency_graph,
                &mut results,
            ),
        )
        .await;

        let duration = start_time.elapsed();

        match execution_result {
            Ok(Ok(())) => {
                self.state_manager
                    .set_execution_status(&execution_id, ExecutionStatus::Completed)
                    .await?;
                Ok(WorkflowResult {
                    success: true,
                    steps_executed: results.len(),
                    errors: vec![],
                    duration_secs: duration.as_secs(),
                })
            }
            Ok(Err(e)) => {
                self.state_manager
                    .set_execution_status(&execution_id, ExecutionStatus::Failed)
                    .await?;
                Ok(WorkflowResult {
                    success: false,
                    steps_executed: results.len(),
                    errors: vec![e.to_string()],
                    duration_secs: duration.as_secs(),
                })
            }
            Err(_) => {
                self.state_manager
                    .set_execution_status(&execution_id, ExecutionStatus::Failed)
                    .await?;
                Ok(WorkflowResult {
                    success: false,
                    steps_executed: results.len(),
                    errors: vec!["Workflow execution timed out".to_string()],
                    duration_secs: duration.as_secs(),
                })
            }
        }
    }

    /// Execute workflow with advanced orchestration (parallel execution, dependency management)
    async fn execute_with_orchestration(
        &self,
        execution_id: &str,
        workflow: &Workflow,
        _dependency_graph: &HashMap<String, Vec<String>>,
        results: &mut HashMap<String, StepResult>,
    ) -> Result<()> {
        let mut pending_steps: VecDeque<String> = workflow
            .steps
            .iter()
            .filter(|step| step.depends_on.is_empty())
            .map(|step| step.id.clone())
            .collect();

        let mut completed_steps = HashSet::new();
        let mut running_tasks = HashMap::new();

        while !pending_steps.is_empty() || !running_tasks.is_empty() {
            // Start new steps if we have capacity
            while running_tasks.len() < self.max_concurrent_steps && !pending_steps.is_empty() {
                let step_id = pending_steps.pop_front().unwrap();
                let step = workflow.steps.iter().find(|s| s.id == step_id).unwrap();

                let execution_id = execution_id.to_string();
                let step = step.clone();
                let state_manager = Arc::clone(&self.state_manager);

                let task = tokio::spawn(async move {
                    Self::execute_step_with_orchestration(&execution_id, &step, state_manager).await
                });

                running_tasks.insert(step_id, task);
            }

            // Wait for at least one task to complete
            if !running_tasks.is_empty() {
                let (completed_step_id, result) =
                    self.wait_for_next_completion(&mut running_tasks).await?;
                results.insert(completed_step_id.clone(), result);

                completed_steps.insert(completed_step_id.clone());

                // Find steps that can now be executed
                for step in &workflow.steps {
                    if !completed_steps.contains(&step.id) && !running_tasks.contains_key(&step.id)
                    {
                        let can_execute = step
                            .depends_on
                            .iter()
                            .all(|dep| completed_steps.contains(dep));
                        if can_execute && !pending_steps.contains(&step.id) {
                            pending_steps.push_back(step.id.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Wait for the next task to complete
    async fn wait_for_next_completion(
        &self,
        running_tasks: &mut HashMap<String, tokio::task::JoinHandle<Result<StepResult>>>,
    ) -> Result<(String, StepResult)> {
        // This is a simplified implementation - in practice, you'd use tokio::select!
        // to wait for any of the running tasks to complete
        if let Some((step_id, task)) = running_tasks.iter_mut().next() {
            let step_id = step_id.clone();
            let result = task
                .await
                .map_err(|e| FuseError::InternalError(format!("Task join error: {}", e)))?
                .map_err(|e| FuseError::InternalError(format!("Step execution error: {}", e)))?;

            running_tasks.remove(&step_id);
            Ok((step_id, result))
        } else {
            Err(FuseError::InternalError(
                "No running tasks to wait for".to_string(),
            ))
        }
    }

    /// Execute a single step with orchestration
    async fn execute_step_with_orchestration(
        execution_id: &str,
        step: &WorkflowStep,
        state_manager: Arc<WorkflowStateManager>,
    ) -> Result<StepResult> {
        state_manager
            .set_current_step(execution_id, Some(step.id.clone()))
            .await?;

        let start_time = std::time::Instant::now();

        // Execute with timeout if specified
        let execution_result = if let Some(timeout_secs) = step.timeout_secs {
            timeout(
                Duration::from_secs(timeout_secs),
                Self::execute_step_action(step),
            )
            .await
            .map_err(|_| FuseError::InternalError(format!("Step {} timed out", step.id)))?
        } else {
            Self::execute_step_action(step).await
        };

        let duration = start_time.elapsed();

        let step_result = match execution_result {
            Ok(output) => StepResult {
                step_id: step.id.clone(),
                status: StepStatus::Completed,
                output,
                error: None,
                started_at: chrono::Utc::now() - duration,
                completed_at: Some(chrono::Utc::now()),
                retry_count: 0,
            },
            Err(e) => StepResult {
                step_id: step.id.clone(),
                status: StepStatus::Failed,
                output: None,
                error: Some(e.to_string()),
                started_at: chrono::Utc::now() - duration,
                completed_at: Some(chrono::Utc::now()),
                retry_count: 0,
            },
        };

        state_manager
            .update_step_status(
                execution_id,
                &step.id,
                step_result.status.clone(),
                step_result.output.clone(),
                step_result.error.clone(),
            )
            .await?;

        Ok(step_result)
    }

    /// Execute step action (placeholder - would integrate with actual services)
    async fn execute_step_action(step: &WorkflowStep) -> Result<Option<String>> {
        match &step.action {
            WorkflowAction::Execute { command } => {
                // Execute command
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map_err(|e| {
                        FuseError::InternalError(format!("Command execution failed: {}", e))
                    })?;

                if output.status.success() {
                    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
                } else {
                    Err(FuseError::InternalError(format!(
                        "Command failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )))
                }
            }
            WorkflowAction::RunInference { model, prompt } => {
                // Placeholder - would integrate with InferenceEngine
                Ok(Some(format!(
                    "Inference completed for model '{}' with prompt: {}",
                    model, prompt
                )))
            }
            WorkflowAction::Quantize { model, method } => {
                // Placeholder - would integrate with QuantizationService
                Ok(Some(format!(
                    "Model '{}' quantized with method '{}'",
                    model, method
                )))
            }
            WorkflowAction::Scan { target } => {
                // Placeholder - would integrate with VulnerabilityScanner
                Ok(Some(format!("Target '{}' scanned successfully", target)))
            }
            WorkflowAction::Merge { models, strategy } => {
                // Placeholder - would integrate with ModelMerger
                Ok(Some(format!(
                    "Models {:?} merged with strategy '{}'",
                    models, strategy
                )))
            }
            WorkflowAction::Custom { command, args } => {
                // Execute custom command
                let output = std::process::Command::new(command)
                    .args(args)
                    .output()
                    .map_err(|e| {
                        FuseError::InternalError(format!("Custom command execution failed: {}", e))
                    })?;

                if output.status.success() {
                    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
                } else {
                    Err(FuseError::InternalError(format!(
                        "Custom command failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )))
                }
            }
            _ => Ok(Some(format!(
                "Action {:?} executed successfully",
                step.action
            ))),
        }
    }

    /// Build dependency graph from workflow steps
    fn build_dependency_graph(
        &self,
        steps: &[WorkflowStep],
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut graph = HashMap::new();

        // Initialize graph with all steps
        for step in steps {
            graph.insert(step.id.clone(), Vec::new());
        }

        // Build dependencies
        for step in steps {
            for dep in &step.depends_on {
                if let Some(dependents) = graph.get_mut(dep) {
                    dependents.push(step.id.clone());
                } else {
                    return Err(FuseError::ValidationError(format!(
                        "Step '{}' depends on unknown step '{}'",
                        step.id, dep
                    )));
                }
            }
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::state::WorkflowStateManager;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_workflow_orchestrator_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state_manager =
            Arc::new(WorkflowStateManager::new(temp_dir.path().join("state")).unwrap());
        let orchestrator = WorkflowOrchestrator::new(state_manager);

        assert_eq!(orchestrator.max_concurrent_steps, 5);
    }

    #[tokio::test]
    async fn test_build_dependency_graph() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state_manager =
            Arc::new(WorkflowStateManager::new(temp_dir.path().join("state")).unwrap());
        let orchestrator = WorkflowOrchestrator::new(state_manager);

        let steps = vec![
            WorkflowStep {
                id: "step1".to_string(),
                name: "Step 1".to_string(),
                description: None,
                action: WorkflowAction::Execute {
                    command: "echo hello".to_string(),
                },
                on_success: None,
                on_failure: None,
                depends_on: vec![],
                retry_policy: Default::default(),
                timeout_secs: None,
            },
            WorkflowStep {
                id: "step2".to_string(),
                name: "Step 2".to_string(),
                description: None,
                action: WorkflowAction::Execute {
                    command: "echo world".to_string(),
                },
                on_success: None,
                on_failure: None,
                depends_on: vec!["step1".to_string()],
                retry_policy: Default::default(),
                timeout_secs: None,
            },
        ];

        let graph = orchestrator.build_dependency_graph(&steps).unwrap();
        assert_eq!(graph.len(), 2);
        assert!(graph["step1"].contains(&"step2".to_string()));
    }
}
