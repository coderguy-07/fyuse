//! Agent permission system [10.5]
//!
//! Tiered permission modes with workspace boundary enforcement.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Permission tier for agent execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Read files and web access only. No writes.
    ReadOnly,
    /// File modifications allowed within workspace root only.
    #[default]
    WorkspaceWrite,
    /// Unrestricted access. Use with caution.
    FullAccess,
}

/// Tool categories for permission gating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolAction {
    ReadFile,
    WriteFile { path: PathBuf },
    DeleteFile { path: PathBuf },
    ExecuteBash { command: String },
    WebFetch,
    WebSearch,
}

/// Permission policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub mode: PermissionMode,
    pub workspace_root: PathBuf,
    pub allowed_paths: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            mode: PermissionMode::WorkspaceWrite,
            workspace_root: PathBuf::from("."),
            allowed_paths: vec![],
            denied_paths: vec![],
        }
    }
}

/// Permission policy enforcer.
pub struct PermissionPolicy {
    config: PermissionConfig,
    resolved_root: PathBuf,
}

impl PermissionPolicy {
    pub fn new(config: PermissionConfig) -> Result<Self> {
        let resolved_root = config
            .workspace_root
            .canonicalize()
            .unwrap_or_else(|_| config.workspace_root.clone());
        Ok(Self {
            config,
            resolved_root,
        })
    }

    /// Check if an action is allowed under the current permission mode.
    pub fn check(&self, action: &ToolAction) -> Result<()> {
        match self.config.mode {
            PermissionMode::FullAccess => Ok(()),
            PermissionMode::ReadOnly => self.check_read_only(action),
            PermissionMode::WorkspaceWrite => self.check_workspace_write(action),
        }
    }

    fn check_read_only(&self, action: &ToolAction) -> Result<()> {
        match action {
            ToolAction::ReadFile | ToolAction::WebFetch | ToolAction::WebSearch => Ok(()),
            ToolAction::WriteFile { path } => Err(FuseError::AgentError(format!(
                "Write denied in read-only mode: {}",
                path.display()
            ))),
            ToolAction::DeleteFile { path } => Err(FuseError::AgentError(format!(
                "Delete denied in read-only mode: {}",
                path.display()
            ))),
            ToolAction::ExecuteBash { command } => Err(FuseError::AgentError(format!(
                "Bash execution denied in read-only mode: {command}"
            ))),
        }
    }

    fn check_workspace_write(&self, action: &ToolAction) -> Result<()> {
        match action {
            ToolAction::ReadFile | ToolAction::WebFetch | ToolAction::WebSearch => Ok(()),
            ToolAction::WriteFile { path } | ToolAction::DeleteFile { path } => {
                self.check_within_workspace(path)
            }
            ToolAction::ExecuteBash { .. } => {
                // Bash allowed in workspace-write, but validated by BashValidator
                Ok(())
            }
        }
    }

    fn check_within_workspace(&self, path: &Path) -> Result<()> {
        // Resolve symlinks to prevent escape
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Check denied paths first
        for denied in &self.config.denied_paths {
            let denied_resolved = denied.canonicalize().unwrap_or_else(|_| denied.clone());
            if resolved.starts_with(&denied_resolved) {
                return Err(FuseError::AgentError(format!(
                    "Path is in denied list: {}",
                    path.display()
                )));
            }
        }

        // Check allowed paths (override workspace check)
        for allowed in &self.config.allowed_paths {
            let allowed_resolved = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
            if resolved.starts_with(&allowed_resolved) {
                return Ok(());
            }
        }

        // Must be within workspace root
        if resolved.starts_with(&self.resolved_root) {
            Ok(())
        } else {
            Err(FuseError::AgentError(format!(
                "Path outside workspace: {} (workspace: {})",
                path.display(),
                self.resolved_root.display()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with_mode(mode: PermissionMode) -> PermissionPolicy {
        PermissionPolicy::new(PermissionConfig {
            mode,
            workspace_root: std::env::temp_dir(),
            allowed_paths: vec![],
            denied_paths: vec![],
        })
        .unwrap()
    }

    #[test]
    fn test_read_only_allows_reads() {
        let p = policy_with_mode(PermissionMode::ReadOnly);
        assert!(p.check(&ToolAction::ReadFile).is_ok());
        assert!(p.check(&ToolAction::WebFetch).is_ok());
        assert!(p.check(&ToolAction::WebSearch).is_ok());
    }

    #[test]
    fn test_read_only_blocks_writes() {
        let p = policy_with_mode(PermissionMode::ReadOnly);
        assert!(p
            .check(&ToolAction::WriteFile {
                path: PathBuf::from("/tmp/test")
            })
            .is_err());
    }

    #[test]
    fn test_read_only_blocks_bash() {
        let p = policy_with_mode(PermissionMode::ReadOnly);
        assert!(p
            .check(&ToolAction::ExecuteBash {
                command: "ls".into()
            })
            .is_err());
    }

    #[test]
    fn test_workspace_write_allows_within() {
        let p = policy_with_mode(PermissionMode::WorkspaceWrite);
        // Use temp_dir itself (which exists and can be canonicalized)
        let path = std::env::temp_dir();
        assert!(p.check(&ToolAction::WriteFile { path }).is_ok());
    }

    #[test]
    fn test_workspace_write_blocks_outside() {
        let config = PermissionConfig {
            mode: PermissionMode::WorkspaceWrite,
            workspace_root: std::env::temp_dir().join("fuse-sandbox-test"),
            allowed_paths: vec![],
            denied_paths: vec![],
        };
        let p = PermissionPolicy::new(config).unwrap();
        // /etc is outside the temp workspace
        assert!(p
            .check(&ToolAction::WriteFile {
                path: PathBuf::from("/etc/passwd")
            })
            .is_err());
    }

    #[test]
    fn test_full_access_allows_everything() {
        let p = policy_with_mode(PermissionMode::FullAccess);
        assert!(p.check(&ToolAction::ReadFile).is_ok());
        assert!(p
            .check(&ToolAction::WriteFile {
                path: PathBuf::from("/anywhere")
            })
            .is_ok());
        assert!(p
            .check(&ToolAction::ExecuteBash {
                command: "rm -rf /".into()
            })
            .is_ok());
    }

    #[test]
    fn test_denied_paths_override() {
        let config = PermissionConfig {
            mode: PermissionMode::WorkspaceWrite,
            workspace_root: std::env::temp_dir(),
            allowed_paths: vec![],
            denied_paths: vec![std::env::temp_dir().join("secrets")],
        };
        let p = PermissionPolicy::new(config).unwrap();

        // Create the denied dir so canonicalize works
        let denied = std::env::temp_dir().join("secrets");
        let _ = std::fs::create_dir_all(&denied);

        let path = denied.join("api-key.txt");
        assert!(p.check(&ToolAction::WriteFile { path }).is_err());
        let _ = std::fs::remove_dir_all(&denied);
    }

    #[test]
    fn test_default_config() {
        let config = PermissionConfig::default();
        assert_eq!(config.mode, PermissionMode::WorkspaceWrite);
    }
}
