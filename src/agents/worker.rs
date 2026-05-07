//! Worker boot state machine [10.1]
//!
//! Typed state machine for agent worker lifecycle.
//! Inspired by claw-code's worker boot protocol.

use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Result of a completed worker task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerResult {
    Success {
        summary: String,
    },
    Partial {
        summary: String,
        remaining: Vec<String>,
    },
}

/// Classified worker failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerFailure {
    pub kind: FailureKind,
    pub message: String,
    pub recoverable: bool,
}

/// Failure classification taxonomy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum FailureKind {
    TrustGate,
    PromptDelivery,
    Protocol,
    Provider,
    Timeout,
    Internal,
}

/// Worker lifecycle states.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum WorkerState {
    Spawning,
    TrustRequired {
        prompt: String,
    },
    ReadyForPrompt,
    PromptAccepted {
        task_id: String,
    },
    Running {
        task_id: String,
        started_at: DateTime<Utc>,
    },
    Finished {
        task_id: String,
        result: WorkerResult,
    },
    Failed {
        error: WorkerFailure,
    },
}

impl WorkerState {
    /// Returns the state name for display/logging.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::TrustRequired { .. } => "trust_required",
            Self::ReadyForPrompt => "ready_for_prompt",
            Self::PromptAccepted { .. } => "prompt_accepted",
            Self::Running { .. } => "running",
            Self::Finished { .. } => "finished",
            Self::Failed { .. } => "failed",
        }
    }

    /// Whether this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished { .. } | Self::Failed { .. })
    }
}

/// Worker manages the lifecycle state machine.
pub struct Worker {
    pub id: String,
    pub state: WorkerState,
    state_file: Option<PathBuf>,
}

impl Worker {
    /// Create a new worker in Spawning state.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: WorkerState::Spawning,
            state_file: None,
        }
    }

    /// Enable state persistence to a file.
    pub fn with_state_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_file = Some(path.into());
        self
    }

    /// Transition: Spawning → TrustRequired
    pub fn require_trust(&mut self, prompt: String) -> Result<()> {
        match &self.state {
            WorkerState::Spawning => {
                self.state = WorkerState::TrustRequired { prompt };
                self.persist_state()?;
                Ok(())
            }
            _ => Err(FuseError::AgentError(format!(
                "Cannot require trust from state: {}",
                self.state.name()
            ))),
        }
    }

    /// Transition: Spawning | TrustRequired → ReadyForPrompt
    pub fn mark_ready(&mut self) -> Result<()> {
        match &self.state {
            WorkerState::Spawning | WorkerState::TrustRequired { .. } => {
                self.state = WorkerState::ReadyForPrompt;
                self.persist_state()?;
                Ok(())
            }
            _ => Err(FuseError::AgentError(format!(
                "Cannot mark ready from state: {}",
                self.state.name()
            ))),
        }
    }

    /// Transition: ReadyForPrompt → PromptAccepted
    pub fn accept_prompt(&mut self, task_id: String) -> Result<()> {
        match &self.state {
            WorkerState::ReadyForPrompt => {
                self.state = WorkerState::PromptAccepted { task_id };
                self.persist_state()?;
                Ok(())
            }
            _ => Err(FuseError::AgentError(format!(
                "Cannot accept prompt from state: {} (must be ready_for_prompt)",
                self.state.name()
            ))),
        }
    }

    /// Transition: PromptAccepted → Running
    pub fn start_running(&mut self) -> Result<()> {
        match &self.state {
            WorkerState::PromptAccepted { task_id } => {
                self.state = WorkerState::Running {
                    task_id: task_id.clone(),
                    started_at: Utc::now(),
                };
                self.persist_state()?;
                Ok(())
            }
            _ => Err(FuseError::AgentError(format!(
                "Cannot start running from state: {}",
                self.state.name()
            ))),
        }
    }

    /// Transition: Running → Finished
    pub fn finish(&mut self, result: WorkerResult) -> Result<()> {
        match &self.state {
            WorkerState::Running { task_id, .. } => {
                self.state = WorkerState::Finished {
                    task_id: task_id.clone(),
                    result,
                };
                self.persist_state()?;
                Ok(())
            }
            _ => Err(FuseError::AgentError(format!(
                "Cannot finish from state: {}",
                self.state.name()
            ))),
        }
    }

    /// Transition: any non-terminal → Failed
    pub fn fail(&mut self, error: WorkerFailure) -> Result<()> {
        if self.state.is_terminal() {
            return Err(FuseError::AgentError(format!(
                "Cannot fail from terminal state: {}",
                self.state.name()
            )));
        }
        self.state = WorkerState::Failed { error };
        self.persist_state()?;
        Ok(())
    }

    /// Persist state to file if configured.
    fn persist_state(&self) -> Result<()> {
        if let Some(path) = &self.state_file {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    FuseError::AgentError(format!("Failed to create state dir: {e}"))
                })?;
            }
            let json = serde_json::to_string_pretty(&self.state)
                .map_err(|e| FuseError::AgentError(format!("Failed to serialize state: {e}")))?;
            std::fs::write(path, json)
                .map_err(|e| FuseError::AgentError(format!("Failed to write state file: {e}")))?;
        }
        Ok(())
    }

    /// Load state from file.
    pub fn load_state(path: &Path) -> Result<WorkerState> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| FuseError::AgentError(format!("Failed to read state file: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| FuseError::AgentError(format!("Failed to parse state file: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_lifecycle() {
        let mut w = Worker::new("w1");
        assert_eq!(w.state.name(), "spawning");
        assert!(!w.state.is_terminal());

        w.mark_ready().unwrap();
        assert_eq!(w.state.name(), "ready_for_prompt");

        w.accept_prompt("task-1".into()).unwrap();
        assert_eq!(w.state.name(), "prompt_accepted");

        w.start_running().unwrap();
        assert_eq!(w.state.name(), "running");

        w.finish(WorkerResult::Success {
            summary: "done".into(),
        })
        .unwrap();
        assert_eq!(w.state.name(), "finished");
        assert!(w.state.is_terminal());
    }

    #[test]
    fn test_trust_gate_path() {
        let mut w = Worker::new("w2");
        w.require_trust("Allow workspace access?".into()).unwrap();
        assert_eq!(w.state.name(), "trust_required");

        w.mark_ready().unwrap();
        assert_eq!(w.state.name(), "ready_for_prompt");
    }

    #[test]
    fn test_invalid_transition_prompt_before_ready() {
        let mut w = Worker::new("w3");
        let err = w.accept_prompt("task-1".into());
        assert!(err.is_err());
    }

    #[test]
    fn test_invalid_transition_run_before_accept() {
        let mut w = Worker::new("w4");
        w.mark_ready().unwrap();
        let err = w.start_running();
        assert!(err.is_err());
    }

    #[test]
    fn test_fail_from_any_non_terminal() {
        let mut w = Worker::new("w5");
        w.mark_ready().unwrap();
        w.fail(WorkerFailure {
            kind: FailureKind::Timeout,
            message: "timed out".into(),
            recoverable: true,
        })
        .unwrap();
        assert!(w.state.is_terminal());
    }

    #[test]
    fn test_cannot_fail_from_terminal() {
        let mut w = Worker::new("w6");
        w.mark_ready().unwrap();
        w.accept_prompt("t1".into()).unwrap();
        w.start_running().unwrap();
        w.finish(WorkerResult::Success {
            summary: "ok".into(),
        })
        .unwrap();

        let err = w.fail(WorkerFailure {
            kind: FailureKind::Internal,
            message: "late".into(),
            recoverable: false,
        });
        assert!(err.is_err());
    }

    #[test]
    fn test_state_persistence() {
        let dir = std::env::temp_dir().join("fuse-test-worker");
        let state_path = dir.join("worker-state.json");
        let _ = std::fs::remove_dir_all(&dir);

        let mut w = Worker::new("w7").with_state_file(&state_path);
        w.mark_ready().unwrap();

        let loaded = Worker::load_state(&state_path).unwrap();
        assert_eq!(loaded, WorkerState::ReadyForPrompt);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let state = WorkerState::Running {
            task_id: "t1".into(),
            started_at: Utc::now(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: WorkerState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), "running");
    }
}
