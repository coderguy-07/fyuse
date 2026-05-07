//! Agent framework — skills, tools, multi-agent orchestration, and agent harness.

pub mod traits;

pub mod mcp;
pub mod swarm;

// Phase 10: Agent Harness (inspired by claw-code)
pub mod bash_validator;
pub mod branch_awareness;
pub mod diagnostics;
pub mod failure;
pub mod lane;
pub mod permissions;
pub mod session;
pub mod task_packet;
pub mod worker;

pub use mcp::{McpClient, McpResource, McpServer, McpServerConfig, McpTool};
pub use swarm::{
    Agent, AgentOutput, AgentSwarm, AgentTask, ConsensusStrategy, SwarmConfig, SwarmResult,
};
pub use traits::{Skill, SkillContext, SkillInput, SkillOutput, SkillTrigger};

// Phase 10 re-exports
pub use bash_validator::{BashValidationConfig, BashValidator, RuleVerdict, ValidationResult};
pub use failure::{FailureKind, FailureReport, RecoveryAction, RecoveryEngine, RecoveryRecipe};
pub use permissions::{PermissionConfig, PermissionMode, PermissionPolicy, ToolAction};
pub use session::{MessageRole, Session, SessionMessage, SessionMeta, SessionStore};
pub use task_packet::{
    AcceptanceTests, BranchPolicy, CommitPolicy, EscalationPolicy, MergePolicy, TaskPacket,
    TaskScope,
};
pub use worker::{Worker, WorkerFailure, WorkerResult, WorkerState};

// Phase 10 P2 re-exports
pub use branch_awareness::{
    assess_branch, BranchAction, BranchAssessment, BranchMetrics, GreenLevel, TestResults,
};
pub use diagnostics::{run_diagnostics, DiagnosticReport, DiagnosticResult, Severity};
pub use lane::{BranchCollision, Lane, LaneBoard, LaneEvent, LaneEventType, LaneState};
