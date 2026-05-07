//! Failure taxonomy & recovery system [10.4]
//!
//! Classified failure types with mapped recovery recipes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Failure classification taxonomy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FailureKind {
    /// Trust prompt not resolved within timeout.
    TrustGate,
    /// Prompt delivered to wrong target (e.g., shell instead of agent).
    PromptDelivery,
    /// Protocol-level failure (unexpected message format).
    Protocol,
    /// Provider API failure (rate limit, auth, network).
    Provider,
    /// Branch is stale (behind main).
    StaleBranch,
    /// Compilation failed after changes.
    CompileError,
    /// Tests failed after changes.
    TestFailure,
    /// MCP server handshake failure.
    McpHandshake,
    /// Timeout waiting for response.
    Timeout,
    /// Unknown / uncategorized failure.
    Unknown,
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrustGate => write!(f, "trust_gate"),
            Self::PromptDelivery => write!(f, "prompt_delivery"),
            Self::Protocol => write!(f, "protocol"),
            Self::Provider => write!(f, "provider"),
            Self::StaleBranch => write!(f, "stale_branch"),
            Self::CompileError => write!(f, "compile_error"),
            Self::TestFailure => write!(f, "test_failure"),
            Self::McpHandshake => write!(f, "mcp_handshake"),
            Self::Timeout => write!(f, "timeout"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A structured failure report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureReport {
    pub kind: FailureKind,
    pub message: String,
    pub context: Option<String>,
    pub recoverable: bool,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl FailureReport {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        let recoverable = kind.is_recoverable();
        let max_retries = kind.default_max_retries();
        Self {
            kind,
            message: message.into(),
            context: None,
            recoverable,
            retry_count: 0,
            max_retries,
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    /// Whether retries are exhausted.
    pub fn retries_exhausted(&self) -> bool {
        self.retry_count >= self.max_retries
    }

    /// Increment retry count. Returns true if still retriable.
    pub fn increment_retry(&mut self) -> bool {
        self.retry_count += 1;
        !self.retries_exhausted()
    }
}

impl FailureKind {
    /// Whether this failure type is generally recoverable.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::TrustGate => true,
            Self::PromptDelivery => true,
            Self::Protocol => false,
            Self::Provider => true,
            Self::StaleBranch => true,
            Self::CompileError => true,
            Self::TestFailure => true,
            Self::McpHandshake => true,
            Self::Timeout => true,
            Self::Unknown => false,
        }
    }

    /// Default max retries for this failure type.
    pub fn default_max_retries(&self) -> u32 {
        match self {
            Self::TrustGate => 1,
            Self::PromptDelivery => 3,
            Self::Protocol => 0,
            Self::Provider => 3,
            Self::StaleBranch => 1,
            Self::CompileError => 2,
            Self::TestFailure => 2,
            Self::McpHandshake => 3,
            Self::Timeout => 2,
            Self::Unknown => 0,
        }
    }
}

/// A recovery recipe for a failure kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRecipe {
    pub kind: FailureKind,
    pub description: String,
    pub steps: Vec<String>,
    pub escalation: String,
}

/// Registry of recovery recipes.
pub struct RecoveryEngine {
    recipes: std::collections::HashMap<FailureKind, RecoveryRecipe>,
}

impl RecoveryEngine {
    /// Create a new recovery engine with default recipes.
    pub fn new() -> Self {
        let mut recipes = std::collections::HashMap::new();

        recipes.insert(
            FailureKind::TrustGate,
            RecoveryRecipe {
                kind: FailureKind::TrustGate,
                description: "Trust prompt not resolved".into(),
                steps: vec![
                    "Check if workspace is in trusted roots".into(),
                    "Auto-resolve trust if in allowlist".into(),
                    "Escalate to user for manual approval".into(),
                ],
                escalation: "User must manually approve trust prompt".into(),
            },
        );

        recipes.insert(
            FailureKind::PromptDelivery,
            RecoveryRecipe {
                kind: FailureKind::PromptDelivery,
                description: "Prompt delivered to wrong target".into(),
                steps: vec![
                    "Detect misdelivery (shell echo, unexpected output)".into(),
                    "Wait for agent ready state".into(),
                    "Replay prompt to correct target".into(),
                ],
                escalation: "Restart worker and retry from beginning".into(),
            },
        );

        recipes.insert(
            FailureKind::Provider,
            RecoveryRecipe {
                kind: FailureKind::Provider,
                description: "Provider API failure".into(),
                steps: vec![
                    "Check rate limit headers".into(),
                    "Exponential backoff (1s, 2s, 4s)".into(),
                    "Retry with same request".into(),
                ],
                escalation: "Switch to fallback provider or notify user".into(),
            },
        );

        recipes.insert(
            FailureKind::StaleBranch,
            RecoveryRecipe {
                kind: FailureKind::StaleBranch,
                description: "Branch is behind main".into(),
                steps: vec![
                    "Run git fetch".into(),
                    "Check ahead/behind count".into(),
                    "Auto merge-forward if no conflicts".into(),
                ],
                escalation: "Rebase required — escalate to user".into(),
            },
        );

        recipes.insert(
            FailureKind::CompileError,
            RecoveryRecipe {
                kind: FailureKind::CompileError,
                description: "Compilation failed after changes".into(),
                steps: vec![
                    "Parse compiler errors".into(),
                    "Identify affected files and lines".into(),
                    "Apply targeted fix".into(),
                ],
                escalation: "Unable to fix compilation — revert changes and escalate".into(),
            },
        );

        recipes.insert(
            FailureKind::TestFailure,
            RecoveryRecipe {
                kind: FailureKind::TestFailure,
                description: "Tests failed after changes".into(),
                steps: vec![
                    "Identify which tests failed".into(),
                    "Check if failure is in changed code or pre-existing".into(),
                    "Fix if caused by current changes".into(),
                ],
                escalation: "Tests still failing — escalate with failure report".into(),
            },
        );

        recipes.insert(
            FailureKind::McpHandshake,
            RecoveryRecipe {
                kind: FailureKind::McpHandshake,
                description: "MCP server handshake failure".into(),
                steps: vec![
                    "Check MCP server process is running".into(),
                    "Verify transport (stdio/websocket) is accessible".into(),
                    "Retry handshake with backoff".into(),
                ],
                escalation: "Start in degraded mode without failed MCP server".into(),
            },
        );

        recipes.insert(
            FailureKind::Timeout,
            RecoveryRecipe {
                kind: FailureKind::Timeout,
                description: "Operation timed out".into(),
                steps: vec![
                    "Check if operation is still in progress".into(),
                    "Extend timeout if operation is making progress".into(),
                    "Retry with increased timeout".into(),
                ],
                escalation: "Operation consistently timing out — escalate".into(),
            },
        );

        Self { recipes }
    }

    /// Get the recovery recipe for a failure kind.
    pub fn get_recipe(&self, kind: &FailureKind) -> Option<&RecoveryRecipe> {
        self.recipes.get(kind)
    }

    /// Attempt recovery for a failure. Returns the next step description.
    pub fn next_step(&self, report: &FailureReport) -> RecoveryAction {
        if !report.recoverable || report.retries_exhausted() {
            if let Some(recipe) = self.get_recipe(&report.kind) {
                return RecoveryAction::Escalate(recipe.escalation.clone());
            }
            return RecoveryAction::Escalate(format!(
                "Unrecoverable failure: {} — {}",
                report.kind, report.message
            ));
        }

        if let Some(recipe) = self.get_recipe(&report.kind) {
            let step_idx = report.retry_count as usize;
            if step_idx < recipe.steps.len() {
                return RecoveryAction::Retry(recipe.steps[step_idx].clone());
            }
            return RecoveryAction::Escalate(recipe.escalation.clone());
        }

        RecoveryAction::Escalate(format!("No recovery recipe for: {}", report.kind))
    }
}

impl Default for RecoveryEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Action returned by the recovery engine.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    Retry(String),
    Escalate(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_kind_display() {
        assert_eq!(format!("{}", FailureKind::TrustGate), "trust_gate");
        assert_eq!(format!("{}", FailureKind::Provider), "provider");
    }

    #[test]
    fn test_failure_kind_recoverable() {
        assert!(FailureKind::TrustGate.is_recoverable());
        assert!(FailureKind::Provider.is_recoverable());
        assert!(!FailureKind::Protocol.is_recoverable());
        assert!(!FailureKind::Unknown.is_recoverable());
    }

    #[test]
    fn test_failure_report_new() {
        let r = FailureReport::new(FailureKind::Provider, "rate limited");
        assert!(r.recoverable);
        assert_eq!(r.max_retries, 3);
        assert_eq!(r.retry_count, 0);
        assert!(!r.retries_exhausted());
    }

    #[test]
    fn test_failure_report_retry_exhaustion() {
        let mut r = FailureReport::new(FailureKind::Provider, "rate limited");
        assert!(r.increment_retry()); // 1/3
        assert!(r.increment_retry()); // 2/3
        assert!(!r.increment_retry()); // 3/3 — exhausted
        assert!(r.retries_exhausted());
    }

    #[test]
    fn test_recovery_engine_has_recipes() {
        let engine = RecoveryEngine::new();
        assert!(engine.get_recipe(&FailureKind::TrustGate).is_some());
        assert!(engine.get_recipe(&FailureKind::Provider).is_some());
        assert!(engine.get_recipe(&FailureKind::StaleBranch).is_some());
        assert!(engine.get_recipe(&FailureKind::CompileError).is_some());
        assert!(engine.get_recipe(&FailureKind::TestFailure).is_some());
        assert!(engine.get_recipe(&FailureKind::McpHandshake).is_some());
        assert!(engine.get_recipe(&FailureKind::Timeout).is_some());
        assert!(engine.get_recipe(&FailureKind::Unknown).is_none());
    }

    #[test]
    fn test_recovery_engine_next_step_retry() {
        let engine = RecoveryEngine::new();
        let report = FailureReport::new(FailureKind::Provider, "rate limited");
        let action = engine.next_step(&report);
        match action {
            RecoveryAction::Retry(step) => assert!(step.contains("rate limit")),
            _ => panic!("Expected retry"),
        }
    }

    #[test]
    fn test_recovery_engine_escalates_when_exhausted() {
        let engine = RecoveryEngine::new();
        let mut report = FailureReport::new(FailureKind::Provider, "rate limited");
        report.retry_count = 3; // Exhausted
        let action = engine.next_step(&report);
        match action {
            RecoveryAction::Escalate(msg) => assert!(msg.contains("fallback")),
            _ => panic!("Expected escalation"),
        }
    }

    #[test]
    fn test_recovery_engine_unrecoverable() {
        let engine = RecoveryEngine::new();
        let report = FailureReport::new(FailureKind::Protocol, "bad format");
        let action = engine.next_step(&report);
        assert!(matches!(action, RecoveryAction::Escalate(_)));
    }

    #[test]
    fn test_failure_report_with_context() {
        let r = FailureReport::new(FailureKind::CompileError, "missing type")
            .with_context("src/main.rs:42");
        assert_eq!(r.context, Some("src/main.rs:42".into()));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let r = FailureReport::new(FailureKind::Timeout, "took too long");
        let json = serde_json::to_string(&r).unwrap();
        let back: FailureReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, FailureKind::Timeout);
        assert_eq!(back.message, "took too long");
    }

    #[test]
    fn test_all_recoverable_kinds_have_recipes() {
        let engine = RecoveryEngine::new();
        let kinds = vec![
            FailureKind::TrustGate,
            FailureKind::PromptDelivery,
            FailureKind::Provider,
            FailureKind::StaleBranch,
            FailureKind::CompileError,
            FailureKind::TestFailure,
            FailureKind::McpHandshake,
            FailureKind::Timeout,
        ];
        for kind in kinds {
            assert!(
                engine.get_recipe(&kind).is_some(),
                "Missing recipe for: {kind}"
            );
        }
    }
}
