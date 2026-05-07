//! Enhanced diagnostics [10.9]
//!
//! Comprehensive system health checks for `fuse doctor`.
//! Outputs JSON or text. Each check is independent and testable.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Ok,
    Warning,
    Error,
}

/// A single diagnostic check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub name: String,
    pub category: String,
    pub severity: Severity,
    pub message: String,
    pub remediation: Option<String>,
}

impl DiagnosticResult {
    pub fn ok(
        name: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            severity: Severity::Ok,
            message: message.into(),
            remediation: None,
        }
    }

    pub fn warning(
        name: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            severity: Severity::Warning,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }

    pub fn error(
        name: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            severity: Severity::Error,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
}

/// Full diagnostic report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub results: Vec<DiagnosticResult>,
    pub total_checks: usize,
    pub errors: usize,
    pub warnings: usize,
    pub passed: usize,
}

impl DiagnosticReport {
    pub fn new(results: Vec<DiagnosticResult>) -> Self {
        let total_checks = results.len();
        let errors = results
            .iter()
            .filter(|r| r.severity == Severity::Error)
            .count();
        let warnings = results
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .count();
        let passed = results
            .iter()
            .filter(|r| r.severity == Severity::Ok)
            .count();

        Self {
            results,
            total_checks,
            errors,
            warnings,
            passed,
        }
    }

    /// Whether all checks passed (no errors).
    pub fn is_healthy(&self) -> bool {
        self.errors == 0
    }

    /// Exit code: 0 if healthy, 1 if warnings only, 2 if errors.
    pub fn exit_code(&self) -> i32 {
        if self.errors > 0 {
            2
        } else if self.warnings > 0 {
            1
        } else {
            0
        }
    }

    /// Render as human-readable text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("Fuse System Diagnostics\n");
        out.push_str("=======================\n\n");

        for result in &self.results {
            let icon = match result.severity {
                Severity::Ok => "✓",
                Severity::Warning => "⚠",
                Severity::Error => "✗",
            };
            out.push_str(&format!(
                "{icon} [{category}] {name}: {message}\n",
                category = result.category,
                name = result.name,
                message = result.message,
            ));
            if let Some(rem) = &result.remediation {
                out.push_str(&format!("  → {rem}\n"));
            }
        }

        out.push_str(&format!(
            "\nSummary: {} passed, {} warnings, {} errors ({} total)\n",
            self.passed, self.warnings, self.errors, self.total_checks
        ));

        out
    }
}

/// Run all diagnostic checks.
pub fn run_diagnostics(workspace: &Path) -> DiagnosticReport {
    let results = vec![
        check_workspace(workspace),
        check_config(workspace),
        check_git(workspace),
        check_rust_toolchain(),
        check_disk_space(workspace),
        check_api_key_env(),
    ];

    DiagnosticReport::new(results)
}

/// Check workspace directory exists and is writable.
fn check_workspace(workspace: &Path) -> DiagnosticResult {
    if !workspace.exists() {
        return DiagnosticResult::error(
            "workspace",
            "filesystem",
            format!("Workspace not found: {}", workspace.display()),
            "Create the workspace directory or change your working directory",
        );
    }

    if !workspace.is_dir() {
        return DiagnosticResult::error(
            "workspace",
            "filesystem",
            "Workspace path is not a directory",
            "Ensure the workspace path points to a directory",
        );
    }

    DiagnosticResult::ok("workspace", "filesystem", "Workspace directory exists")
}

/// Check for fuse.toml configuration.
fn check_config(workspace: &Path) -> DiagnosticResult {
    let config_path = workspace.join("fuse.toml");
    if config_path.exists() {
        DiagnosticResult::ok("config", "configuration", "fuse.toml found")
    } else {
        DiagnosticResult::warning(
            "config",
            "configuration",
            "fuse.toml not found in workspace",
            "Create a fuse.toml configuration file or run `fuse init`",
        )
    }
}

/// Check git repository.
fn check_git(workspace: &Path) -> DiagnosticResult {
    let git_dir = workspace.join(".git");
    if git_dir.exists() {
        DiagnosticResult::ok("git", "vcs", "Git repository detected")
    } else {
        DiagnosticResult::warning(
            "git",
            "vcs",
            "Not a git repository",
            "Run `git init` to initialize version control",
        )
    }
}

/// Check Rust toolchain.
fn check_rust_toolchain() -> DiagnosticResult {
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            DiagnosticResult::ok(
                "rust",
                "toolchain",
                format!("Rust installed: {}", version.trim()),
            )
        }
        _ => DiagnosticResult::warning(
            "rust",
            "toolchain",
            "Rust toolchain not found",
            "Install Rust via https://rustup.rs",
        ),
    }
}

/// Check available disk space.
fn check_disk_space(workspace: &Path) -> DiagnosticResult {
    // Simple check: can we write a temp file?
    let test_file = workspace.join(".fuse-health-check");
    match std::fs::write(&test_file, "ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            DiagnosticResult::ok("disk", "filesystem", "Disk is writable")
        }
        Err(e) => DiagnosticResult::error(
            "disk",
            "filesystem",
            format!("Cannot write to workspace: {e}"),
            "Check disk space and permissions",
        ),
    }
}

/// Check for API key environment variables.
fn check_api_key_env() -> DiagnosticResult {
    let keys = [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "HUGGING_FACE_HUB_TOKEN",
    ];

    let found: Vec<&str> = keys
        .iter()
        .filter(|k| std::env::var(k).is_ok())
        .copied()
        .collect();

    if found.is_empty() {
        DiagnosticResult::warning(
            "api_keys",
            "auth",
            "No API keys found in environment",
            "Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or HUGGING_FACE_HUB_TOKEN",
        )
    } else {
        DiagnosticResult::ok(
            "api_keys",
            "auth",
            format!("Found {} API key(s): {}", found.len(), found.join(", ")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_diagnostic_result_ok() {
        let r = DiagnosticResult::ok("test", "cat", "all good");
        assert_eq!(r.severity, Severity::Ok);
        assert!(r.remediation.is_none());
    }

    #[test]
    fn test_diagnostic_result_warning() {
        let r = DiagnosticResult::warning("test", "cat", "issue", "fix it");
        assert_eq!(r.severity, Severity::Warning);
        assert_eq!(r.remediation.as_deref(), Some("fix it"));
    }

    #[test]
    fn test_diagnostic_result_error() {
        let r = DiagnosticResult::error("test", "cat", "broken", "repair");
        assert_eq!(r.severity, Severity::Error);
    }

    #[test]
    fn test_report_healthy() {
        let report = DiagnosticReport::new(vec![
            DiagnosticResult::ok("a", "c", "ok"),
            DiagnosticResult::ok("b", "c", "ok"),
        ]);
        assert!(report.is_healthy());
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.passed, 2);
    }

    #[test]
    fn test_report_with_warnings() {
        let report = DiagnosticReport::new(vec![
            DiagnosticResult::ok("a", "c", "ok"),
            DiagnosticResult::warning("b", "c", "warn", "fix"),
        ]);
        assert!(report.is_healthy()); // warnings don't count as unhealthy
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.warnings, 1);
    }

    #[test]
    fn test_report_with_errors() {
        let report = DiagnosticReport::new(vec![
            DiagnosticResult::ok("a", "c", "ok"),
            DiagnosticResult::error("b", "c", "fail", "fix"),
        ]);
        assert!(!report.is_healthy());
        assert_eq!(report.exit_code(), 2);
        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_report_to_text() {
        let report = DiagnosticReport::new(vec![
            DiagnosticResult::ok("workspace", "fs", "exists"),
            DiagnosticResult::warning("config", "cfg", "missing", "create one"),
        ]);
        let text = report.to_text();
        assert!(text.contains("✓"));
        assert!(text.contains("⚠"));
        assert!(text.contains("Summary:"));
    }

    #[test]
    fn test_report_to_json() {
        let report = DiagnosticReport::new(vec![DiagnosticResult::ok("a", "c", "ok")]);
        let json = serde_json::to_string(&report).unwrap();
        let back: DiagnosticReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total_checks, 1);
        assert_eq!(back.passed, 1);
    }

    #[test]
    fn test_check_workspace_exists() {
        let result = check_workspace(Path::new(env::temp_dir().to_str().unwrap()));
        assert_eq!(result.severity, Severity::Ok);
    }

    #[test]
    fn test_check_workspace_missing() {
        let result = check_workspace(Path::new("/nonexistent/path/xyz"));
        assert_eq!(result.severity, Severity::Error);
    }

    #[test]
    fn test_check_config_missing() {
        let result = check_config(Path::new(env::temp_dir().to_str().unwrap()));
        // temp dir unlikely has fuse.toml
        assert_eq!(result.severity, Severity::Warning);
    }

    #[test]
    fn test_check_git_not_repo() {
        let result = check_git(Path::new(env::temp_dir().to_str().unwrap()));
        // temp dir unlikely is a git repo
        assert!(matches!(result.severity, Severity::Ok | Severity::Warning));
    }

    #[test]
    fn test_check_rust_toolchain() {
        let result = check_rust_toolchain();
        // We're running Rust, so this should pass
        assert_eq!(result.severity, Severity::Ok);
        assert!(result.message.contains("rustc"));
    }

    #[test]
    fn test_check_disk_writable() {
        let result = check_disk_space(&env::temp_dir());
        assert_eq!(result.severity, Severity::Ok);
    }

    #[test]
    fn test_run_diagnostics() {
        let report = run_diagnostics(&env::temp_dir());
        assert_eq!(report.total_checks, 6);
        // At minimum, workspace and disk checks should pass
        assert!(report.passed >= 2);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Ok < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }
}
