use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DirectoryError {
    #[error("Failed to create directory: {0}")]
    CreateError(String),

    #[error("Failed to access directory: {0}")]
    AccessError(String),

    #[error("Home directory not found")]
    HomeNotFound,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DirectoryError>;

/// Manages directory paths for both global and project-specific Fuse directories
#[derive(Debug, Clone)]
pub struct DirectoryManager {
    /// Global Fuse directory (~/.fuse_cli)
    global_dir: PathBuf,

    /// Project-specific Fuse directory (./.fuse) - optional
    project_dir: Option<PathBuf>,
}

impl DirectoryManager {
    /// Create a new DirectoryManager
    ///
    /// This will:
    /// - Detect the global directory (~/.fuse_cli)
    /// - Detect if we're in a project with a .fuse directory
    /// - Handle migration from old ~/.fuse to ~/.fuse_cli if needed
    pub fn new() -> Result<Self> {
        let global_dir = Self::detect_global_dir()?;
        let project_dir = Self::detect_project_dir();

        Ok(Self {
            global_dir,
            project_dir,
        })
    }

    /// Get the global Fuse directory path
    pub fn global_dir(&self) -> &Path {
        &self.global_dir
    }

    /// Get the project-specific Fuse directory path (if any)
    pub fn project_dir(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    /// Ensure the global directory exists, creating it if necessary
    pub fn ensure_global_dir(&self) -> Result<()> {
        self.ensure_dir(&self.global_dir)
    }

    /// Ensure the project directory exists, creating it if necessary
    pub fn ensure_project_dir(&self) -> Result<()> {
        if let Some(project_dir) = &self.project_dir {
            self.ensure_dir(project_dir)
        } else {
            Err(DirectoryError::AccessError(
                "No project directory detected".to_string(),
            ))
        }
    }

    /// Find the configuration file with priority resolution
    /// Priority: project config > global config > env variable
    pub fn find_config(&self) -> Result<PathBuf> {
        // 1. Check for environment variable override
        if let Ok(config_path) = env::var("FUSE_CONFIG") {
            let path = PathBuf::from(config_path);
            if path.exists() {
                return Ok(path);
            }
        }

        // 2. Check for project-specific config
        if let Some(project_dir) = &self.project_dir {
            let project_config = project_dir.join("config.toml");
            if project_config.exists() {
                return Ok(project_config);
            }

            let project_config_yaml = project_dir.join("config.yaml");
            if project_config_yaml.exists() {
                return Ok(project_config_yaml);
            }
        }

        // 3. Check for global config
        let global_config = self.global_dir.join("config.toml");
        if global_config.exists() {
            return Ok(global_config);
        }

        let global_config_yaml = self.global_dir.join("config.yaml");
        if global_config_yaml.exists() {
            return Ok(global_config_yaml);
        }

        // 4. Return default global config path (even if it doesn't exist yet)
        Ok(global_config)
    }

    /// Get the models directory path
    /// Can be overridden by FUSE_MODELS_DIR environment variable
    pub fn models_dir(&self) -> PathBuf {
        if let Ok(models_dir) = env::var("FUSE_MODELS_DIR") {
            return PathBuf::from(models_dir);
        }

        self.global_dir.join("models")
    }

    /// Get the cache directory path
    /// Can be overridden by FUSE_CACHE_DIR environment variable
    pub fn cache_dir(&self) -> PathBuf {
        if let Ok(cache_dir) = env::var("FUSE_CACHE_DIR") {
            return PathBuf::from(cache_dir);
        }

        self.global_dir.join("cache")
    }

    /// Get the logs directory path
    /// Can be overridden by FUSE_LOGS_DIR environment variable
    pub fn logs_dir(&self) -> PathBuf {
        if let Ok(logs_dir) = env::var("FUSE_LOGS_DIR") {
            return PathBuf::from(logs_dir);
        }

        self.global_dir.join("logs")
    }

    /// Get the reports directory path (project-specific)
    /// Falls back to global directory if no project directory exists
    pub fn reports_dir(&self) -> PathBuf {
        if let Some(project_dir) = &self.project_dir {
            project_dir.join("report")
        } else {
            self.global_dir.join("report")
        }
    }

    /// Get the specs directory path (project-specific)
    /// Falls back to global directory if no project directory exists
    pub fn specs_dir(&self) -> PathBuf {
        if let Some(project_dir) = &self.project_dir {
            project_dir.join("specs")
        } else {
            self.global_dir.join("specs")
        }
    }

    /// Get the vibe (workflow logs) directory path (project-specific)
    /// Falls back to global directory if no project directory exists
    pub fn vibe_dir(&self) -> PathBuf {
        if let Some(project_dir) = &self.project_dir {
            project_dir.join("vibe")
        } else {
            self.global_dir.join("vibe")
        }
    }

    /// Detect the global Fuse directory
    /// Checks for ~/.fuse_cli first, then ~/.fuse (for migration)
    fn detect_global_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or(DirectoryError::HomeNotFound)?;

        let new_dir = home.join(".fuse_cli");
        let old_dir = home.join(".fuse");

        // If new directory exists, use it
        if new_dir.exists() {
            return Ok(new_dir);
        }

        // If old directory exists, we'll use new path but note migration is needed
        // (Migration will be handled separately)
        if old_dir.exists() {
            // For now, return the new path - migration will be prompted later
            return Ok(new_dir);
        }

        // Neither exists, return new path
        Ok(new_dir)
    }

    /// Detect project-specific Fuse directory
    /// Walks up from current directory looking for .fuse directory
    fn detect_project_dir() -> Option<PathBuf> {
        let mut current = env::current_dir().ok()?;

        loop {
            let fuse_dir = current.join(".fuse");
            if fuse_dir.exists() && fuse_dir.is_dir() {
                return Some(fuse_dir);
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Ensure a directory exists, creating it if necessary
    fn ensure_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| DirectoryError::CreateError(format!("{}: {}", path.display(), e)))?;
        }
        Ok(())
    }

    /// Check if migration from old ~/.fuse to ~/.fuse_cli is needed
    pub fn needs_migration(&self) -> bool {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return false,
        };

        let old_dir = home.join(".fuse");
        let new_dir = home.join(".fuse_cli");

        // Migration needed if old exists and new doesn't
        old_dir.exists() && !new_dir.exists()
    }

    /// Get the old directory path for migration
    pub fn old_global_dir(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let old_dir = home.join(".fuse");

        if old_dir.exists() {
            Some(old_dir)
        } else {
            None
        }
    }
}

impl Default for DirectoryManager {
    fn default() -> Self {
        Self::new().expect("Failed to create DirectoryManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_directory_manager_creation() {
        let manager = DirectoryManager::new();
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert!(manager.global_dir().to_string_lossy().contains(".fuse_cli"));
    }

    #[test]
    fn test_global_dir_path() {
        let manager = DirectoryManager::new().unwrap();
        let global_dir = manager.global_dir();

        assert!(global_dir.ends_with(".fuse_cli"));
    }

    #[test]
    fn test_models_dir() {
        // Clear env var to ensure test consistency
        env::remove_var("FUSE_MODELS_DIR");

        let manager = DirectoryManager::new().unwrap();
        let models_dir = manager.models_dir();

        assert!(models_dir.ends_with("models"));
        assert!(models_dir.to_string_lossy().contains(".fuse_cli"));
    }

    #[test]
    fn test_cache_dir() {
        // Clear env var to ensure test consistency
        env::remove_var("FUSE_CACHE_DIR");

        let manager = DirectoryManager::new().unwrap();
        let cache_dir = manager.cache_dir();

        assert!(cache_dir.ends_with("cache"));
        assert!(cache_dir.to_string_lossy().contains(".fuse_cli"));
    }

    #[test]
    fn test_logs_dir() {
        // Clear env var to ensure test consistency
        env::remove_var("FUSE_LOGS_DIR");

        let manager = DirectoryManager::new().unwrap();
        let logs_dir = manager.logs_dir();

        assert!(logs_dir.ends_with("logs"));
        assert!(logs_dir.to_string_lossy().contains(".fuse_cli"));
    }

    #[test]
    fn test_reports_dir_without_project() {
        // Clear env vars to ensure test consistency
        env::remove_var("FUSE_MODELS_DIR");
        env::remove_var("FUSE_CACHE_DIR");
        env::remove_var("FUSE_LOGS_DIR");

        let manager = DirectoryManager::new().unwrap();
        let reports_dir = manager.reports_dir();

        // Without project dir, should use global
        assert!(reports_dir.to_string_lossy().contains(".fuse_cli"));
        assert!(reports_dir.ends_with("report"));
    }

    #[test]
    fn test_specs_dir_without_project() {
        // Clear env vars to ensure test consistency
        env::remove_var("FUSE_MODELS_DIR");
        env::remove_var("FUSE_CACHE_DIR");
        env::remove_var("FUSE_LOGS_DIR");

        let manager = DirectoryManager::new().unwrap();
        let specs_dir = manager.specs_dir();

        // Without project dir, should use global
        assert!(specs_dir.to_string_lossy().contains(".fuse_cli"));
        assert!(specs_dir.ends_with("specs"));
    }

    #[test]
    fn test_vibe_dir_without_project() {
        // Clear env vars to ensure test consistency
        env::remove_var("FUSE_MODELS_DIR");
        env::remove_var("FUSE_CACHE_DIR");
        env::remove_var("FUSE_LOGS_DIR");

        let manager = DirectoryManager::new().unwrap();
        let vibe_dir = manager.vibe_dir();

        // Without project dir, should use global
        assert!(vibe_dir.to_string_lossy().contains(".fuse_cli"));
        assert!(vibe_dir.ends_with("vibe"));
    }

    #[test]
    fn test_ensure_global_dir() {
        let temp_dir = TempDir::new().unwrap();
        let global_dir = temp_dir.path().join(".fuse_cli");

        let manager = DirectoryManager {
            global_dir: global_dir.clone(),
            project_dir: None,
        };

        assert!(!global_dir.exists());

        let result = manager.ensure_global_dir();
        assert!(result.is_ok());
        assert!(global_dir.exists());
    }

    #[test]
    fn test_find_config_priority() {
        let temp_dir = TempDir::new().unwrap();
        let global_dir = temp_dir.path().join(".fuse_cli");
        let project_dir = temp_dir.path().join(".fuse");

        fs::create_dir_all(&global_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();

        // Create global config
        let global_config = global_dir.join("config.toml");
        fs::write(&global_config, "# global config").unwrap();

        // Create project config
        let project_config = project_dir.join("config.toml");
        fs::write(&project_config, "# project config").unwrap();

        let manager = DirectoryManager {
            global_dir: global_dir.clone(),
            project_dir: Some(project_dir.clone()),
        };

        // Should return project config (higher priority)
        let config_path = manager.find_config().unwrap();
        assert_eq!(config_path, project_config);
    }

    #[test]
    fn test_find_config_global_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let global_dir = temp_dir.path().join(".fuse_cli");

        fs::create_dir_all(&global_dir).unwrap();

        // Create only global config
        let global_config = global_dir.join("config.toml");
        fs::write(&global_config, "# global config").unwrap();

        let manager = DirectoryManager {
            global_dir: global_dir.clone(),
            project_dir: None,
        };

        // Should return global config
        let config_path = manager.find_config().unwrap();
        assert_eq!(config_path, global_config);
    }

    #[test]
    fn test_env_var_override_models_dir() {
        env::set_var("FUSE_MODELS_DIR", "/custom/models");

        let manager = DirectoryManager::new().unwrap();
        let models_dir = manager.models_dir();

        assert_eq!(models_dir, PathBuf::from("/custom/models"));

        env::remove_var("FUSE_MODELS_DIR");
    }

    #[test]
    fn test_env_var_override_cache_dir() {
        env::set_var("FUSE_CACHE_DIR", "/custom/cache");

        let manager = DirectoryManager::new().unwrap();
        let cache_dir = manager.cache_dir();

        assert_eq!(cache_dir, PathBuf::from("/custom/cache"));

        env::remove_var("FUSE_CACHE_DIR");
    }

    #[test]
    fn test_needs_migration() {
        // This test would need to mock the home directory
        // For now, just test the logic doesn't panic
        let manager = DirectoryManager::new().unwrap();
        let _ = manager.needs_migration();
    }
}
