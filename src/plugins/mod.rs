//! Plugin system — extensibility via dynamic libraries and WASM.

pub mod traits;

pub use traits::{Plugin, PluginContext};

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Output from a plugin execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOutput {
    /// Output data.
    pub data: serde_json::Value,
    /// Whether the execution was successful.
    pub success: bool,
    /// Optional message.
    pub message: Option<String>,
}

/// Input to a plugin execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInput {
    /// The action to perform.
    pub action: String,
    /// Input parameters.
    pub params: serde_json::Value,
}

/// Plugin manifest — describes a plugin package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name.
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Entry point (e.g., shared library path or WASM module).
    pub entry_point: String,
}

impl PluginManifest {
    /// Validate the manifest.
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(FuseError::ValidationError(
                "Plugin name must not be empty".to_string(),
            ));
        }
        if self.version.is_empty() {
            return Err(FuseError::ValidationError(
                "Plugin version must not be empty".to_string(),
            ));
        }
        if self.entry_point.is_empty() {
            return Err(FuseError::ValidationError(
                "Plugin entry_point must not be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Parse a manifest from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let manifest: PluginManifest =
            toml::from_str(toml_str).map_err(|e| FuseError::ConfigError(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
}

/// Trait for an executable plugin (extends the base Plugin trait with execution).
pub trait ExecutablePlugin: Plugin {
    /// Execute the plugin with the given input.
    fn execute(&self, input: &PluginInput) -> Result<PluginOutput>;
}

/// Manages plugin lifecycle — load, unload, list, execute.
pub struct PluginManager {
    /// Loaded plugins by name.
    plugins: HashMap<String, PluginEntry>,
}

struct PluginEntry {
    manifest: PluginManifest,
    /// Stub — in the future this will hold a dylib handle or WASM instance.
    loaded: bool,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Load a plugin from a manifest path.
    ///
    /// Stub implementation — reads and validates the manifest but does not
    /// actually load a dynamic library. Real dylib/WASM loading comes later.
    pub fn load_plugin(&mut self, manifest_path: &Path) -> Result<()> {
        // Read the manifest file
        let content = std::fs::read_to_string(manifest_path).map_err(|e| {
            FuseError::IoError(std::io::Error::new(
                e.kind(),
                format!("Failed to read plugin manifest: {e}"),
            ))
        })?;
        let manifest = PluginManifest::from_toml(&content)?;

        if self.plugins.contains_key(&manifest.name) {
            return Err(FuseError::ValidationError(format!(
                "Plugin already loaded: {}",
                manifest.name
            )));
        }

        self.plugins.insert(
            manifest.name.clone(),
            PluginEntry {
                manifest,
                loaded: true,
            },
        );
        Ok(())
    }

    /// Load a plugin directly from a manifest (no file I/O).
    pub fn load_from_manifest(&mut self, manifest: PluginManifest) -> Result<()> {
        manifest.validate()?;

        if self.plugins.contains_key(&manifest.name) {
            return Err(FuseError::ValidationError(format!(
                "Plugin already loaded: {}",
                manifest.name
            )));
        }

        self.plugins.insert(
            manifest.name.clone(),
            PluginEntry {
                manifest,
                loaded: true,
            },
        );
        Ok(())
    }

    /// Unload a plugin by name.
    pub fn unload_plugin(&mut self, name: &str) -> Result<()> {
        self.plugins
            .remove(name)
            .ok_or_else(|| FuseError::ValidationError(format!("Plugin not loaded: {name}")))?;
        Ok(())
    }

    /// List all loaded plugins.
    pub fn list_plugins(&self) -> Vec<&PluginManifest> {
        self.plugins.values().map(|e| &e.manifest).collect()
    }

    /// Check if a plugin is loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.get(name).is_some_and(|e| e.loaded)
    }

    /// Get a plugin manifest by name.
    pub fn get_manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name).map(|e| &e.manifest)
    }

    /// Execute a plugin by name (stub — returns a placeholder output).
    pub fn execute(&self, plugin_name: &str, input: &PluginInput) -> Result<PluginOutput> {
        let entry = self.plugins.get(plugin_name).ok_or_else(|| {
            FuseError::ValidationError(format!("Plugin not loaded: {plugin_name}"))
        })?;

        if !entry.loaded {
            return Err(FuseError::ValidationError(format!(
                "Plugin not active: {plugin_name}"
            )));
        }

        // Stub — real implementation will call into dylib/WASM
        Ok(PluginOutput {
            data: serde_json::json!({
                "plugin": plugin_name,
                "action": input.action,
                "status": "stub"
            }),
            success: true,
            message: Some(format!(
                "Stub execution of {} v{}",
                entry.manifest.name, entry.manifest.version
            )),
        })
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            entry_point: "libtest_plugin.so".to_string(),
        }
    }

    #[test]
    fn test_manifest_validation_valid() {
        assert!(sample_manifest().validate().is_ok());
    }

    #[test]
    fn test_manifest_validation_empty_name() {
        let mut m = sample_manifest();
        m.name = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_validation_empty_version() {
        let mut m = sample_manifest();
        m.version = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_validation_empty_entry_point() {
        let mut m = sample_manifest();
        m.entry_point = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_from_toml() {
        let toml = r#"
            name = "my-plugin"
            version = "0.2.0"
            description = "Does things"
            entry_point = "libmy_plugin.dylib"
        "#;
        let manifest = PluginManifest::from_toml(toml);
        assert!(manifest.is_ok());
        let m = manifest.expect("parse failed");
        assert_eq!(m.name, "my-plugin");
        assert_eq!(m.version, "0.2.0");
    }

    #[test]
    fn test_manifest_from_toml_invalid() {
        let toml = "not valid toml {{{}}}";
        assert!(PluginManifest::from_toml(toml).is_err());
    }

    #[test]
    fn test_manifest_from_toml_missing_fields() {
        let toml = r#"
            name = "incomplete"
        "#;
        assert!(PluginManifest::from_toml(toml).is_err());
    }

    #[test]
    fn test_plugin_manager_creation() {
        let manager = PluginManager::new();
        assert!(manager.list_plugins().is_empty());
    }

    #[test]
    fn test_plugin_manager_default() {
        let manager = PluginManager::default();
        assert!(manager.list_plugins().is_empty());
    }

    #[test]
    fn test_load_from_manifest() {
        let mut manager = PluginManager::new();
        let result = manager.load_from_manifest(sample_manifest());
        assert!(result.is_ok());
        assert_eq!(manager.list_plugins().len(), 1);
        assert!(manager.is_loaded("test-plugin"));
    }

    #[test]
    fn test_load_duplicate_plugin() {
        let mut manager = PluginManager::new();
        manager
            .load_from_manifest(sample_manifest())
            .expect("first load failed");
        let result = manager.load_from_manifest(sample_manifest());
        assert!(result.is_err());
    }

    #[test]
    fn test_unload_plugin() {
        let mut manager = PluginManager::new();
        manager
            .load_from_manifest(sample_manifest())
            .expect("load failed");
        assert!(manager.is_loaded("test-plugin"));

        let result = manager.unload_plugin("test-plugin");
        assert!(result.is_ok());
        assert!(!manager.is_loaded("test-plugin"));
        assert!(manager.list_plugins().is_empty());
    }

    #[test]
    fn test_unload_nonexistent_plugin() {
        let mut manager = PluginManager::new();
        let result = manager.unload_plugin("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_manifest() {
        let mut manager = PluginManager::new();
        manager
            .load_from_manifest(sample_manifest())
            .expect("load failed");
        let manifest = manager.get_manifest("test-plugin");
        assert!(manifest.is_some());
        assert_eq!(manifest.expect("no manifest").version, "1.0.0");

        assert!(manager.get_manifest("nonexistent").is_none());
    }

    #[test]
    fn test_execute_plugin() {
        let mut manager = PluginManager::new();
        manager
            .load_from_manifest(sample_manifest())
            .expect("load failed");

        let input = PluginInput {
            action: "process".to_string(),
            params: serde_json::json!({"key": "value"}),
        };
        let result = manager.execute("test-plugin", &input);
        assert!(result.is_ok());

        let output = result.expect("execute failed");
        assert!(output.success);
        assert_eq!(output.data["plugin"], "test-plugin");
        assert_eq!(output.data["action"], "process");
    }

    #[test]
    fn test_execute_nonexistent_plugin() {
        let manager = PluginManager::new();
        let input = PluginInput {
            action: "test".to_string(),
            params: serde_json::json!({}),
        };
        let result = manager.execute("nonexistent", &input);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_output_serialization() {
        let output = PluginOutput {
            data: serde_json::json!({"result": 42}),
            success: true,
            message: Some("done".to_string()),
        };
        let json = serde_json::to_string(&output).expect("serialize failed");
        let deserialized: PluginOutput = serde_json::from_str(&json).expect("deserialize failed");
        assert!(deserialized.success);
        assert_eq!(deserialized.data["result"], 42);
    }

    #[test]
    fn test_plugin_input_serialization() {
        let input = PluginInput {
            action: "run".to_string(),
            params: serde_json::json!({"x": 1}),
        };
        let json = serde_json::to_string(&input).expect("serialize failed");
        let deserialized: PluginInput = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.action, "run");
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).expect("serialize failed");
        let deserialized: PluginManifest = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.name, manifest.name);
    }

    #[test]
    fn test_plugin_lifecycle_full() {
        let mut manager = PluginManager::new();

        // Load
        manager
            .load_from_manifest(sample_manifest())
            .expect("load failed");
        assert!(manager.is_loaded("test-plugin"));

        // Execute
        let input = PluginInput {
            action: "test".to_string(),
            params: serde_json::json!({}),
        };
        let output = manager
            .execute("test-plugin", &input)
            .expect("execute failed");
        assert!(output.success);

        // Unload
        manager.unload_plugin("test-plugin").expect("unload failed");
        assert!(!manager.is_loaded("test-plugin"));

        // Execute after unload should fail
        let result = manager.execute("test-plugin", &input);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_plugin_from_file() {
        let manager = PluginManager::new();
        // Loading from a nonexistent path should fail
        let mut manager = manager;
        let result = manager.load_plugin(Path::new("/nonexistent/plugin.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_plugin_from_tempfile() {
        let dir = tempfile::tempdir().expect("create tempdir failed");
        let manifest_path = dir.path().join("plugin.toml");
        std::fs::write(
            &manifest_path,
            r#"
name = "temp-plugin"
version = "0.1.0"
description = "Temporary test plugin"
entry_point = "libtemp.so"
"#,
        )
        .expect("write manifest failed");

        let mut manager = PluginManager::new();
        let result = manager.load_plugin(&manifest_path);
        assert!(result.is_ok());
        assert!(manager.is_loaded("temp-plugin"));
    }
}
