//! Typed task packet format [10.3]
//!
//! Structured task definitions for autonomous agent execution.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Scope of a task — what the agent is allowed to modify.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskScope {
    /// Entire workspace.
    Workspace,
    /// Specific module/crate.
    Module { path: String },
    /// Single file.
    SingleFile { path: String },
    /// Custom scope with explicit paths.
    Custom { paths: Vec<String> },
}

/// Branch policy for the task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchPolicy {
    /// Create a new branch for this task.
    pub create_branch: bool,
    /// Branch name pattern (e.g., "feat/{task_id}").
    pub branch_pattern: Option<String>,
    /// Base branch to branch from.
    pub base_branch: String,
}

impl Default for BranchPolicy {
    fn default() -> Self {
        Self {
            create_branch: true,
            branch_pattern: None,
            base_branch: "main".into(),
        }
    }
}

/// Commit policy for the task.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum CommitPolicy {
    /// One commit per task.
    SingleCommit,
    /// One commit per logical unit of work.
    #[default]
    PerUnit,
    /// No commits (user will commit).
    NoCommit,
}

/// Merge policy for the task.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum MergePolicy {
    /// Auto-merge when all tests pass.
    AutoMerge,
    /// Create PR for review.
    #[default]
    PullRequest,
    /// No merge (leave on branch).
    Manual,
}

/// Test requirements for acceptance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcceptanceTests {
    /// Commands to run for acceptance.
    pub commands: Vec<String>,
    /// All tests must pass for acceptance.
    pub require_all_pass: bool,
    /// Minimum coverage percentage (if applicable).
    pub min_coverage: Option<f64>,
}

impl Default for AcceptanceTests {
    fn default() -> Self {
        Self {
            commands: vec!["cargo test".into()],
            require_all_pass: true,
            min_coverage: None,
        }
    }
}

/// Escalation policy when the agent gets stuck.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EscalationPolicy {
    /// Max retries before escalating.
    pub max_retries: u32,
    /// Timeout in seconds before escalating.
    pub timeout_secs: u64,
    /// Escalation target (e.g., "user", "team-lead").
    pub target: String,
}

impl Default for EscalationPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_secs: 3600,
            target: "user".into(),
        }
    }
}

/// A typed task packet for autonomous agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPacket {
    /// Unique task identifier.
    pub id: String,
    /// Human-readable objective.
    pub objective: String,
    /// Detailed description of what to do.
    pub description: String,
    /// Scope of modifications allowed.
    pub scope: TaskScope,
    /// Branch policy.
    pub branch_policy: BranchPolicy,
    /// Commit policy.
    pub commit_policy: CommitPolicy,
    /// Merge policy.
    pub merge_policy: MergePolicy,
    /// Acceptance tests.
    pub acceptance: AcceptanceTests,
    /// Escalation policy.
    pub escalation: EscalationPolicy,
    /// Arbitrary metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskPacket {
    /// Create a new task packet with required fields.
    pub fn new(
        id: impl Into<String>,
        objective: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            objective: objective.into(),
            description: description.into(),
            scope: TaskScope::Workspace,
            branch_policy: BranchPolicy::default(),
            commit_policy: CommitPolicy::default(),
            merge_policy: MergePolicy::default(),
            acceptance: AcceptanceTests::default(),
            escalation: EscalationPolicy::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_scope(mut self, scope: TaskScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_branch_policy(mut self, policy: BranchPolicy) -> Self {
        self.branch_policy = policy;
        self
    }

    pub fn with_commit_policy(mut self, policy: CommitPolicy) -> Self {
        self.commit_policy = policy;
        self
    }

    pub fn with_merge_policy(mut self, policy: MergePolicy) -> Self {
        self.merge_policy = policy;
        self
    }

    /// Validate the task packet for completeness and consistency.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(FuseError::AgentError("Task ID cannot be empty".into()));
        }
        if self.objective.is_empty() {
            return Err(FuseError::AgentError(
                "Task objective cannot be empty".into(),
            ));
        }
        if self.description.is_empty() {
            return Err(FuseError::AgentError(
                "Task description cannot be empty".into(),
            ));
        }

        // Validate scope paths exist (basic check)
        match &self.scope {
            TaskScope::SingleFile { path } if path.is_empty() => {
                return Err(FuseError::AgentError(
                    "SingleFile scope must have a non-empty path".into(),
                ));
            }
            TaskScope::Module { path } if path.is_empty() => {
                return Err(FuseError::AgentError(
                    "Module scope must have a non-empty path".into(),
                ));
            }
            TaskScope::Custom { paths } if paths.is_empty() => {
                return Err(FuseError::AgentError(
                    "Custom scope must have at least one path".into(),
                ));
            }
            _ => {}
        }

        // Validate acceptance tests
        if self.acceptance.commands.is_empty() {
            return Err(FuseError::AgentError(
                "Acceptance tests must have at least one command".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task_packet() {
        let task = TaskPacket::new("t1", "Fix bug", "Fix the null pointer in main.rs");
        assert_eq!(task.id, "t1");
        assert_eq!(task.scope, TaskScope::Workspace);
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_task_with_scope() {
        let task =
            TaskPacket::new("t2", "Refactor", "Refactor module").with_scope(TaskScope::Module {
                path: "src/agents".into(),
            });
        assert!(matches!(task.scope, TaskScope::Module { .. }));
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_task_validation_empty_id() {
        let task = TaskPacket::new("", "Fix", "desc");
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_validation_empty_objective() {
        let task = TaskPacket::new("t1", "", "desc");
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_validation_empty_description() {
        let task = TaskPacket::new("t1", "Fix", "");
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_validation_empty_single_file_path() {
        let task = TaskPacket::new("t1", "Fix", "desc")
            .with_scope(TaskScope::SingleFile { path: "".into() });
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_validation_empty_custom_paths() {
        let task =
            TaskPacket::new("t1", "Fix", "desc").with_scope(TaskScope::Custom { paths: vec![] });
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_with_policies() {
        let task = TaskPacket::new("t1", "Fix", "desc")
            .with_commit_policy(CommitPolicy::SingleCommit)
            .with_merge_policy(MergePolicy::AutoMerge)
            .with_branch_policy(BranchPolicy {
                create_branch: true,
                branch_pattern: Some("fix/{task_id}".into()),
                base_branch: "develop".into(),
            });
        assert_eq!(task.commit_policy, CommitPolicy::SingleCommit);
        assert_eq!(task.merge_policy, MergePolicy::AutoMerge);
        assert_eq!(task.branch_policy.base_branch, "develop");
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let task = TaskPacket::new("t1", "Fix bug", "Fix null pointer").with_scope(
            TaskScope::SingleFile {
                path: "src/main.rs".into(),
            },
        );
        let json = serde_json::to_string(&task).unwrap();
        let back: TaskPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "t1");
        assert_eq!(back.objective, "Fix bug");
        assert!(matches!(back.scope, TaskScope::SingleFile { .. }));
    }

    #[test]
    fn test_default_policies() {
        let branch = BranchPolicy::default();
        assert!(branch.create_branch);
        assert_eq!(branch.base_branch, "main");

        let commit = CommitPolicy::default();
        assert_eq!(commit, CommitPolicy::PerUnit);

        let merge = MergePolicy::default();
        assert_eq!(merge, MergePolicy::PullRequest);

        let acceptance = AcceptanceTests::default();
        assert_eq!(acceptance.commands, vec!["cargo test".to_string()]);
        assert!(acceptance.require_all_pass);

        let escalation = EscalationPolicy::default();
        assert_eq!(escalation.max_retries, 3);
        assert_eq!(escalation.target, "user");
    }

    #[test]
    fn test_metadata() {
        let mut task = TaskPacket::new("t1", "Fix", "desc");
        task.metadata
            .insert("priority".into(), serde_json::json!("P1"));
        assert_eq!(task.metadata.get("priority").unwrap(), "P1");
    }
}
