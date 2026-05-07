/// Command definitions and argument structures
/// This module contains all CLI command definitions in a centralized location
use std::path::PathBuf;

/// Arguments for the init command
#[derive(Debug, Clone)]
pub struct InitArgs {
    pub from_file: Option<PathBuf>,
    pub yes: bool,
}

/// Arguments for the pull command
#[derive(Debug, Clone)]
pub struct PullArgs {
    pub model: String,
    pub source: Option<String>,
    pub resume: bool,
}

/// Arguments for the run command
#[derive(Debug, Clone)]
pub struct RunArgs {
    pub model: String,
    pub port: Option<u16>,
}

/// Arguments for the rm command
#[derive(Debug, Clone)]
pub struct RmArgs {
    pub model: String,
    pub yes: bool,
}

/// Arguments for the update command
#[derive(Debug, Clone)]
pub struct UpdateArgs {
    pub model: String,
}

/// Arguments for the list command
#[derive(Debug, Clone)]
pub struct ListArgs {
    pub verbose: bool,
    pub source: Option<String>,
}

/// Arguments for the config command
#[derive(Debug, Clone)]
pub struct ConfigArgs {
    pub path: bool,
    pub validate: bool,
}

/// Arguments for the inspect command
#[derive(Debug, Clone)]
pub struct InspectArgs {
    pub model: String,
    pub json: bool,
}

/// Arguments for the quantize command
#[derive(Debug, Clone)]
pub struct QuantizeArgs {
    pub model: String,
    pub method: String,
    pub format: Option<String>,
    pub output: Option<String>,
}

/// Arguments for layer commands
#[derive(Debug, Clone)]
pub enum LayerArgs {
    Inspect {
        model: String,
        wide: bool,
    },
    Remove {
        model: String,
        layer_id: String,
    },
    Add {
        model: String,
        layer_type: String,
        config: std::path::PathBuf,
    },
}

/// Arguments for compatibility check
#[derive(Debug, Clone)]
pub struct CompCheckArgs {
    pub models: Vec<String>,
}

/// Arguments for merge command
#[derive(Debug, Clone)]
pub struct MergeArgs {
    pub models: Vec<String>,
    pub output: String,
    pub strategy: String,
    pub weights: Option<String>,
}

/// Arguments for scan command
#[derive(Debug, Clone)]
pub struct ScanArgs {
    pub model: String,
    pub remote: bool,
    pub format: String,
    pub output: Option<std::path::PathBuf>,
}

/// Arguments for remote commands
#[derive(Debug, Clone)]
pub enum RemoteArgs {
    Add {
        name: String,
        url: String,
        api_key: Option<String>,
    },
    Remove {
        name: String,
    },
    List,
}

/// Arguments for workflow commands
#[derive(Debug, Clone)]
pub enum WorkflowArgs {
    Run {
        workflow: std::path::PathBuf,
        verbose: bool,
    },
    List,
    Validate {
        workflow: std::path::PathBuf,
    },
}

/// Arguments for UI command
#[derive(Debug, Clone)]
pub struct UiArgs {
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open: bool,
}

/// Arguments for history command
#[derive(Debug, Clone)]
pub struct HistoryArgs {
    pub limit: Option<usize>,
    pub clear: bool,
    pub model: Option<String>,
}

/// Arguments for MCP commands
#[derive(Debug, Clone)]
pub enum McpArgs {
    Start {
        port: Option<u16>,
        config: Option<std::path::PathBuf>,
    },
    Stop,
    Status,
}

/// Arguments for feature commands
#[derive(Debug, Clone)]
pub enum FeatureArgs {
    List,
    Enable { feature: String },
    Disable { feature: String },
}

/// Arguments for queue commands
#[derive(Debug, Clone)]
pub enum QueueArgs {
    /// Show queue statistics
    Stats,
    /// Flush pending requests
    Flush,
    /// Check queue health
    Health,
}

/// Arguments for system commands
#[derive(Debug, Clone)]
pub enum SystemArgs {
    /// Check system capabilities
    Check,
    /// Show resource usage
    Resources,
    /// Show system health
    Health,
}

/// Arguments for monitor commands
#[derive(Debug, Clone)]
pub enum MonitorArgs {
    /// Show performance metrics
    Performance,
    /// Monitor resource usage
    Resources,
    /// Monitor queue status
    Queue,
}

/// Arguments for backup commands
#[derive(Debug, Clone)]
pub enum BackupArgs {
    /// Create configuration backup
    Create,
    /// Restore from backup
    Restore { file: std::path::PathBuf },
    /// List available backups
    List,
}

/// Arguments for debug commands
#[derive(Debug, Clone)]
pub enum DebugArgs {
    /// Show recent logs
    Logs,
    /// Validate configuration
    Config,
    /// Check connection pools
    Connections,
    /// Model loading diagnostics
    Models,
}

/// Arguments for serve command
#[derive(Debug, Clone)]
pub struct ServeArgs {
    pub port: Option<u16>,
    pub host: Option<String>,
    pub ollama_api: bool,
    pub openai_api: bool,
    pub anthropic_api: bool,
}

/// Arguments for model merging
#[derive(Debug, Clone)]
pub struct MergeModelsArgs {
    pub models: Vec<String>,
    pub output: String,
    pub strategy: String,
    pub weights: Option<String>,
    pub slerp_t: Option<f32>,
    pub base_model: Option<usize>,
    pub validate: bool,
    pub preserve_metadata: bool,
}
