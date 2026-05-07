//! Branch/test awareness [10.10]
//!
//! Stale branch detection, green contract levels, and auto-suggest
//! merge-forward or rebase for agent workflows.

use serde::{Deserialize, Serialize};

/// Branch freshness metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMetrics {
    pub branch: String,
    pub base_branch: String,
    pub commits_ahead: u32,
    pub commits_behind: u32,
    pub has_conflicts: bool,
    pub last_commit_age_secs: u64,
}

impl BranchMetrics {
    /// Whether the branch is stale (behind base by threshold).
    pub fn is_stale(&self, max_behind: u32) -> bool {
        self.commits_behind > max_behind
    }

    /// Whether the branch is diverged (both ahead and behind).
    pub fn is_diverged(&self) -> bool {
        self.commits_ahead > 0 && self.commits_behind > 0
    }

    /// Suggest a recovery action based on metrics.
    pub fn suggest_action(&self) -> BranchAction {
        if !self.is_stale(0) {
            return BranchAction::None;
        }

        if self.has_conflicts {
            return BranchAction::ManualRebase {
                reason: "Branch has conflicts with base".into(),
            };
        }

        if self.commits_behind <= 5 && !self.is_diverged() {
            return BranchAction::MergeForward {
                from: self.base_branch.clone(),
            };
        }

        if self.commits_behind <= 20 {
            return BranchAction::Rebase {
                onto: self.base_branch.clone(),
            };
        }

        BranchAction::ManualRebase {
            reason: format!(
                "Branch is {} commits behind — too far for auto-rebase",
                self.commits_behind
            ),
        }
    }
}

/// Suggested action for a stale branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchAction {
    /// No action needed — branch is up to date.
    None,
    /// Merge base into branch (fast-forward safe).
    MergeForward { from: String },
    /// Rebase branch onto base.
    Rebase { onto: String },
    /// Manual intervention required.
    ManualRebase { reason: String },
}

/// Green contract levels — how "green" is a branch?
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GreenLevel {
    /// No tests have been run.
    Unknown,
    /// Targeted tests pass (only tests for changed files).
    Targeted,
    /// All tests in the changed package/crate pass.
    Package,
    /// Full workspace test suite passes.
    Workspace,
    /// Workspace green + branch is up-to-date with base = merge-ready.
    MergeReady,
}

impl GreenLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Targeted => "targeted",
            Self::Package => "package",
            Self::Workspace => "workspace",
            Self::MergeReady => "merge-ready",
        }
    }

    /// Whether this level is sufficient for merge.
    pub fn is_merge_ready(&self) -> bool {
        *self == Self::MergeReady
    }
}

/// Test results for green level classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub targeted_pass: bool,
    pub package_pass: bool,
    pub workspace_pass: bool,
    pub branch_up_to_date: bool,
}

impl TestResults {
    /// Classify the green level from test results.
    pub fn classify(&self) -> GreenLevel {
        if self.workspace_pass && self.branch_up_to_date {
            GreenLevel::MergeReady
        } else if self.workspace_pass {
            GreenLevel::Workspace
        } else if self.package_pass {
            GreenLevel::Package
        } else if self.targeted_pass {
            GreenLevel::Targeted
        } else {
            GreenLevel::Unknown
        }
    }
}

/// Branch awareness checker — combines freshness and green level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchAssessment {
    pub metrics: BranchMetrics,
    pub green_level: GreenLevel,
    pub action: BranchAction,
    pub merge_ready: bool,
}

/// Assess a branch for readiness.
pub fn assess_branch(metrics: BranchMetrics, test_results: &TestResults) -> BranchAssessment {
    let green_level = test_results.classify();
    let action = metrics.suggest_action();
    let merge_ready = green_level.is_merge_ready() && action == BranchAction::None;

    BranchAssessment {
        metrics,
        green_level,
        action,
        merge_ready,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_metrics() -> BranchMetrics {
        BranchMetrics {
            branch: "feat/thing".into(),
            base_branch: "main".into(),
            commits_ahead: 3,
            commits_behind: 0,
            has_conflicts: false,
            last_commit_age_secs: 300,
        }
    }

    fn stale_metrics(behind: u32) -> BranchMetrics {
        BranchMetrics {
            commits_behind: behind,
            ..fresh_metrics()
        }
    }

    #[test]
    fn test_fresh_branch_not_stale() {
        let m = fresh_metrics();
        assert!(!m.is_stale(0));
        assert!(!m.is_diverged());
    }

    #[test]
    fn test_stale_branch() {
        let m = stale_metrics(10);
        assert!(m.is_stale(5));
        assert!(!m.is_stale(15));
    }

    #[test]
    fn test_diverged_branch() {
        let m = BranchMetrics {
            commits_ahead: 3,
            commits_behind: 5,
            ..fresh_metrics()
        };
        assert!(m.is_diverged());
    }

    #[test]
    fn test_suggest_none_for_fresh() {
        let m = fresh_metrics();
        assert_eq!(m.suggest_action(), BranchAction::None);
    }

    #[test]
    fn test_suggest_merge_forward() {
        // Non-diverged: ahead=0, behind=3
        let m = BranchMetrics {
            commits_ahead: 0,
            commits_behind: 3,
            ..fresh_metrics()
        };
        match m.suggest_action() {
            BranchAction::MergeForward { from } => assert_eq!(from, "main"),
            other => panic!("Expected MergeForward, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_rebase() {
        let m = stale_metrics(15);
        match m.suggest_action() {
            BranchAction::Rebase { onto } => assert_eq!(onto, "main"),
            other => panic!("Expected Rebase, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_manual_for_conflicts() {
        let m = BranchMetrics {
            has_conflicts: true,
            commits_behind: 3,
            ..fresh_metrics()
        };
        assert!(matches!(
            m.suggest_action(),
            BranchAction::ManualRebase { .. }
        ));
    }

    #[test]
    fn test_suggest_manual_for_very_stale() {
        let m = stale_metrics(50);
        assert!(matches!(
            m.suggest_action(),
            BranchAction::ManualRebase { .. }
        ));
    }

    #[test]
    fn test_green_level_unknown() {
        let r = TestResults {
            targeted_pass: false,
            package_pass: false,
            workspace_pass: false,
            branch_up_to_date: false,
        };
        assert_eq!(r.classify(), GreenLevel::Unknown);
    }

    #[test]
    fn test_green_level_targeted() {
        let r = TestResults {
            targeted_pass: true,
            package_pass: false,
            workspace_pass: false,
            branch_up_to_date: false,
        };
        assert_eq!(r.classify(), GreenLevel::Targeted);
    }

    #[test]
    fn test_green_level_package() {
        let r = TestResults {
            targeted_pass: true,
            package_pass: true,
            workspace_pass: false,
            branch_up_to_date: false,
        };
        assert_eq!(r.classify(), GreenLevel::Package);
    }

    #[test]
    fn test_green_level_workspace() {
        let r = TestResults {
            targeted_pass: true,
            package_pass: true,
            workspace_pass: true,
            branch_up_to_date: false,
        };
        assert_eq!(r.classify(), GreenLevel::Workspace);
    }

    #[test]
    fn test_green_level_merge_ready() {
        let r = TestResults {
            targeted_pass: true,
            package_pass: true,
            workspace_pass: true,
            branch_up_to_date: true,
        };
        assert_eq!(r.classify(), GreenLevel::MergeReady);
        assert!(r.classify().is_merge_ready());
    }

    #[test]
    fn test_green_level_ordering() {
        assert!(GreenLevel::Unknown < GreenLevel::Targeted);
        assert!(GreenLevel::Targeted < GreenLevel::Package);
        assert!(GreenLevel::Package < GreenLevel::Workspace);
        assert!(GreenLevel::Workspace < GreenLevel::MergeReady);
    }

    #[test]
    fn test_assess_merge_ready() {
        let metrics = fresh_metrics();
        let tests = TestResults {
            targeted_pass: true,
            package_pass: true,
            workspace_pass: true,
            branch_up_to_date: true,
        };
        let assessment = assess_branch(metrics, &tests);
        assert!(assessment.merge_ready);
        assert_eq!(assessment.green_level, GreenLevel::MergeReady);
        assert_eq!(assessment.action, BranchAction::None);
    }

    #[test]
    fn test_assess_not_merge_ready_stale() {
        let metrics = stale_metrics(10);
        let tests = TestResults {
            targeted_pass: true,
            package_pass: true,
            workspace_pass: true,
            branch_up_to_date: true,
        };
        let assessment = assess_branch(metrics, &tests);
        assert!(!assessment.merge_ready); // stale, needs action
    }

    #[test]
    fn test_green_level_label() {
        assert_eq!(GreenLevel::Unknown.label(), "unknown");
        assert_eq!(GreenLevel::MergeReady.label(), "merge-ready");
    }

    #[test]
    fn test_serde_roundtrip() {
        let m = fresh_metrics();
        let json = serde_json::to_string(&m).unwrap();
        let back: BranchMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(back.branch, "feat/thing");
    }
}
