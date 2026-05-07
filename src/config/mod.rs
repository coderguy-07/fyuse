use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub mod directory;
pub mod feature_flags;

pub use directory::{DirectoryError, DirectoryManager};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuseConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,

    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default)]
    pub feature_flags: feature_flags::FeatureFlags,

    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub registries: Vec<RegistryConfig>,

    #[serde(default)]
    pub inference: InferenceConfig,

    #[serde(default)]
    pub resource_management: ResourceManagementConfig,
}

impl Default for FuseConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            models_dir: default_models_dir(),
            cache_dir: default_cache_dir(),
            log_level: default_log_level(),
            feature_flags: feature_flags::FeatureFlags::default(),
            server: ServerConfig::default(),
            registries: vec![],
            inference: InferenceConfig::default(),
            resource_management: ResourceManagementConfig::default(),
        }
    }
}

impl FuseConfig {
    /// Load configuration from a TOML file
    pub fn from_toml_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: FuseConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("TOML parse error: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a YAML file
    pub fn from_yaml_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: FuseConfig = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("YAML parse error: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file (auto-detect format)
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            Self::from_yaml_file(path)
        } else {
            Self::from_toml_file(path)
        }
    }

    /// Save configuration to a TOML file
    pub fn to_toml_file(&self, path: &PathBuf) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::ParseError(format!("TOML serialization error: {}", e)))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Save configuration to file (defaults to TOML)
    pub fn to_file(&self, path: &PathBuf) -> Result<()> {
        self.to_toml_file(path)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.log_level.is_empty() {
            return Err(ConfigError::ValidationError(
                "log_level cannot be empty".to_string(),
            ));
        }

        // Validate log level
        match self.log_level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid log_level: {}. Must be one of: trace, debug, info, warn, error",
                    self.log_level
                )))
            }
        }

        Ok(())
    }

    /// Get or create default configuration with interactive setup
    /// Uses DirectoryManager for proper path resolution
    pub fn load_or_default() -> Result<Self> {
        let dir_manager = DirectoryManager::new().map_err(|e| {
            ConfigError::LoadError(format!("Failed to initialize directories: {}", e))
        })?;

        // Ensure global directory exists
        dir_manager.ensure_global_dir().map_err(|e| {
            ConfigError::LoadError(format!("Failed to create global directory: {}", e))
        })?;

        // Find config with priority resolution
        let config_path = dir_manager
            .find_config()
            .map_err(|e| ConfigError::LoadError(format!("Failed to find config: {}", e)))?;

        if config_path.exists() {
            let mut config = Self::from_file(&config_path)?;
            config.apply_env_overrides();
            Ok(config)
        } else {
            // Create config directory if it doesn't exist
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Run interactive setup
            let mut config = Self::interactive_setup()?;

            // Apply environment overrides
            config.apply_env_overrides();

            // Update paths to use DirectoryManager
            config.models_dir = dir_manager.models_dir();
            config.cache_dir = dir_manager.cache_dir();

            // Save the configured file
            config.to_file(&config_path)?;

            println!("\n✓ Configuration saved to: {}", config_path.display());

            Ok(config)
        }
    }

    /// Apply environment variable overrides to configuration
    pub fn apply_env_overrides(&mut self) {
        use std::env;

        if let Ok(val) = env::var("FUSE_LOG_LEVEL") {
            self.log_level = val;
        }

        if let Ok(val) = env::var("FUSE_MODELS_DIR") {
            self.models_dir = PathBuf::from(val);
        }

        if let Ok(val) = env::var("FUSE_CACHE_DIR") {
            self.cache_dir = PathBuf::from(val);
        }

        if let Ok(val) = env::var("FUSE_DATA_DIR") {
            self.data_dir = PathBuf::from(val);
        }
    }

    /// Interactive setup for first-time configuration
    fn interactive_setup() -> Result<Self> {
        use std::io::{self, Write};

        println!("\n╔════════════════════════════════════════════════════════════╗");
        println!("║          Welcome to Fuse - First Time Setup               ║");
        println!("╚════════════════════════════════════════════════════════════╝\n");

        println!("Let's configure Fuse. Press Enter to use default values.\n");

        let mut config = Self::default();

        // Models directory
        print!("Models directory [{}]: ", config.models_dir.display());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            config.models_dir = PathBuf::from(input.trim());
        }

        // Cache directory
        print!("Cache directory [{}]: ", config.cache_dir.display());
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            config.cache_dir = PathBuf::from(input.trim());
        }

        // Log level
        print!(
            "Log level (trace/debug/info/warn/error) [{}]: ",
            config.log_level
        );
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            config.log_level = input.trim().to_string();
        }

        // Server configuration
        println!("\n--- Server Configuration ---");
        print!("Server host [{}]: ", config.server.host);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            config.server.host = input.trim().to_string();
        }

        print!("Server port [{}]: ", config.server.port);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            if let Ok(port) = input.trim().parse() {
                config.server.port = port;
            }
        }

        // Feature flags
        println!("\n--- Feature Flags ---");
        println!("Enable optional features? (y/n for each)\n");

        config.feature_flags.agentic_coding =
            Self::prompt_yes_no("Enable agentic coding (automated workflows)?", false)?;

        config.feature_flags.thinking_visualization =
            Self::prompt_yes_no("Enable thinking visualization?", false)?;

        config.feature_flags.generative_ui = Self::prompt_yes_no("Enable generative UI?", false)?;

        config.feature_flags.mcp_server = Self::prompt_yes_no("Enable MCP server support?", false)?;

        config.feature_flags.vulnerability_scanning =
            Self::prompt_yes_no("Enable vulnerability scanning?", false)?;

        println!("\n✓ Configuration complete!");

        Ok(config)
    }

    /// Helper function to prompt for yes/no
    fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
        use std::io::{self, Write};

        let default_str = if default { "Y/n" } else { "y/N" };
        print!("{} [{}]: ", prompt, default_str);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input.is_empty() {
            Ok(default)
        } else {
            Ok(input == "y" || input == "yes")
        }
    }

    /// Get default configuration file path
    /// Uses DirectoryManager for proper path resolution
    pub fn default_config_path() -> PathBuf {
        if let Ok(dir_manager) = DirectoryManager::new() {
            dir_manager.find_config().unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".fuse_cli")
                    .join("config.toml")
            })
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".fuse_cli")
                .join("config.toml")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    #[serde(default)]
    pub rate_limit: RateLimitConfig,

    pub tls: Option<TlsConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_connections: default_max_connections(),
            rate_limit: RateLimitConfig::default(),
            tls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: default_requests_per_minute(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub auth_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    #[serde(default = "default_max_tokens")]
    pub default_max_tokens: usize,

    #[serde(default = "default_temperature")]
    pub default_temperature: f32,

    #[serde(default = "default_context_window")]
    pub context_window: usize,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            default_max_tokens: default_max_tokens(),
            default_temperature: default_temperature(),
            context_window: default_context_window(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceManagementConfig {
    /// Time before considering a model idle (seconds)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,

    /// Maximum memory usage before triggering cleanup (bytes)
    #[serde(default = "default_max_memory")]
    pub max_memory_bytes: u64,

    /// Maximum number of models to keep loaded
    #[serde(default = "default_max_loaded")]
    pub max_loaded_models: usize,

    /// Enable automatic unloading of idle models
    #[serde(default = "default_true")]
    pub auto_unload_idle: bool,

    /// Enable memory optimization for idle models
    #[serde(default = "default_true")]
    pub optimize_idle_memory: bool,

    /// Enable GPU offloading for idle models
    #[serde(default = "default_true")]
    pub offload_to_cpu: bool,
}

impl Default for ResourceManagementConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: default_idle_timeout(),
            max_memory_bytes: default_max_memory(),
            max_loaded_models: default_max_loaded(),
            auto_unload_idle: true,
            optimize_idle_memory: true,
            offload_to_cpu: true,
        }
    }
}

impl ResourceManagementConfig {
    /// Convert to ResourcePolicy
    pub fn to_policy(&self) -> crate::model::ResourcePolicy {
        crate::model::ResourcePolicy {
            idle_timeout: std::time::Duration::from_secs(self.idle_timeout_secs),
            max_memory_bytes: self.max_memory_bytes,
            max_loaded_models: self.max_loaded_models,
            auto_unload_idle: self.auto_unload_idle,
            optimize_idle_memory: self.optimize_idle_memory,
            offload_to_cpu: self.offload_to_cpu,
        }
    }
}

// Default value functions
fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fuse_cli")
        .join("data")
}

fn default_models_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fuse_cli")
        .join("models")
}

fn default_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fuse_cli")
        .join("cache")
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_max_connections() -> usize {
    100
}

fn default_requests_per_minute() -> u32 {
    60
}

fn default_max_tokens() -> usize {
    2048
}

fn default_temperature() -> f32 {
    0.7
}

fn default_context_window() -> usize {
    4096
}

fn default_idle_timeout() -> u64 {
    300 // 5 minutes
}

fn default_max_memory() -> u64 {
    8 * 1024 * 1024 * 1024 // 8GB
}

fn default_max_loaded() -> usize {
    3
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> FuseConfig {
        FuseConfig {
            data_dir: PathBuf::from("/tmp/test/data"),
            models_dir: PathBuf::from("/tmp/test/models"),
            cache_dir: PathBuf::from("/tmp/test/cache"),
            log_level: "debug".to_string(),
            feature_flags: feature_flags::FeatureFlags::default(),
            server: ServerConfig::default(),
            registries: vec![],
            inference: InferenceConfig::default(),
            resource_management: ResourceManagementConfig::default(),
        }
    }

    #[test]
    fn test_default_config() {
        let config = FuseConfig::default();
        assert_eq!(config.log_level, "info");
        assert!(!config.feature_flags.agentic_coding);
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = create_test_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_log_level() {
        let mut config = create_test_config();
        config.log_level = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let mut config = create_test_config();
        config.log_level = "invalid".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid log_level"));
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = create_test_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("models_dir"));
        assert!(toml_str.contains("log_level"));
        assert!(toml_str.contains("[feature_flags]"));
    }

    #[test]
    fn test_config_toml_deserialization() {
        let toml_str = r#"
            models_dir = "/tmp/models"
            cache_dir = "/tmp/cache"
            log_level = "info"
            
            [feature_flags]
            agentic_coding = true
            
            [server]
            host = "0.0.0.0"
            port = 9090
        "#;

        let config: FuseConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.models_dir, PathBuf::from("/tmp/models"));
        assert_eq!(config.log_level, "info");
        assert!(config.feature_flags.agentic_coding);
        assert_eq!(config.server.port, 9090);
    }

    #[test]
    fn test_config_yaml_serialization() {
        let config = create_test_config();
        let yaml_str = serde_yaml::to_string(&config).unwrap();
        assert!(yaml_str.contains("models_dir"));
        assert!(yaml_str.contains("log_level"));
    }

    #[test]
    fn test_config_yaml_deserialization() {
        let yaml_str = r#"
            models_dir: /tmp/models
            cache_dir: /tmp/cache
            log_level: info
            feature_flags:
              agentic_coding: true
              thinking_visualization: false
            server:
              host: 0.0.0.0
              port: 9090
        "#;

        let config: FuseConfig = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(config.models_dir, PathBuf::from("/tmp/models"));
        assert_eq!(config.log_level, "info");
        assert!(config.feature_flags.agentic_coding);
    }

    #[test]
    fn test_config_file_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = create_test_config();
        config.to_toml_file(&config_path).unwrap();

        assert!(config_path.exists());

        let loaded_config = FuseConfig::from_toml_file(&config_path).unwrap();
        assert_eq!(loaded_config.log_level, config.log_level);
        assert_eq!(loaded_config.models_dir, config.models_dir);
    }

    #[test]
    fn test_config_yaml_file_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");

        let config = create_test_config();
        let yaml_str = serde_yaml::to_string(&config).unwrap();
        fs::write(&config_path, yaml_str).unwrap();

        let loaded_config = FuseConfig::from_yaml_file(&config_path).unwrap();
        assert_eq!(loaded_config.log_level, config.log_level);
        assert_eq!(loaded_config.models_dir, config.models_dir);
    }

    #[test]
    fn test_server_config_defaults() {
        let server = ServerConfig::default();
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 8080);
        assert_eq!(server.max_connections, 100);
        assert_eq!(server.rate_limit.requests_per_minute, 60);
    }

    #[test]
    fn test_inference_config_defaults() {
        let inference = InferenceConfig::default();
        assert_eq!(inference.default_max_tokens, 2048);
        assert_eq!(inference.default_temperature, 0.7);
        assert_eq!(inference.context_window, 4096);
    }

    #[test]
    fn test_registry_config() {
        let registry = RegistryConfig {
            name: "test".to_string(),
            url: "https://test.com".to_string(),
            auth_required: true,
        };

        assert_eq!(registry.name, "test");
        assert!(registry.auth_required);
    }

    #[test]
    fn test_config_with_registries() {
        let mut config = create_test_config();
        config.registries.push(RegistryConfig {
            name: "huggingface".to_string(),
            url: "https://huggingface.co".to_string(),
            auth_required: false,
        });

        assert_eq!(config.registries.len(), 1);
        assert_eq!(config.registries[0].name, "huggingface");
    }

    #[test]
    fn test_tls_config() {
        let tls = TlsConfig {
            cert_path: PathBuf::from("/path/to/cert.pem"),
            key_path: PathBuf::from("/path/to/key.pem"),
        };

        assert_eq!(tls.cert_path, PathBuf::from("/path/to/cert.pem"));
        assert_eq!(tls.key_path, PathBuf::from("/path/to/key.pem"));
    }
}
