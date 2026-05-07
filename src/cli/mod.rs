pub mod commands;
pub mod handlers;
pub mod progress;
pub mod slash_commands;
pub mod validation;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[clap(
    name = "fuse",
    version,
    author = "Fuse Contributors",
    about = "A comprehensive AI model management platform",
    long_about = "Fuse is a Swiss Army knife for AI model operations - pull, run, manage, and interact with AI models from various sources"
)]
pub struct Cli {
    /// Path to configuration file
    #[clap(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[clap(short, long, global = true)]
    pub log_level: Option<String>,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize Fuse configuration
    Init {
        /// Copy configuration from file instead of interactive setup
        #[clap(short, long)]
        from_file: Option<PathBuf>,

        /// Skip interactive prompts and use defaults
        #[clap(short = 'y', long)]
        yes: bool,
    },

    /// Pull a model from a registry
    Pull {
        /// Model name or URL
        model: String,

        /// Source provider (huggingface, unsloth, remote)
        #[clap(short, long)]
        source: Option<String>,

        /// Specific model format to download (e.g., gguf, safetensors, onnx, pytorch)
        #[clap(short = 'f', long)]
        format: Option<String>,

        /// Resume a paused or failed download
        #[clap(short, long)]
        resume: bool,
    },

    /// Run a model and start inference server
    Run {
        /// Model name
        model: String,

        /// Port to run server on
        #[clap(short, long)]
        port: Option<u16>,
    },

    /// Remove a model
    Rm {
        /// Model name
        model: String,

        /// Skip confirmation prompt
        #[clap(short = 'y', long)]
        yes: bool,
    },

    /// Update a model to the latest version
    Update {
        /// Model name
        model: String,
    },

    /// List all models
    List {
        /// Show detailed information
        #[clap(short, long)]
        verbose: bool,

        /// Filter by source
        #[clap(short, long)]
        source: Option<String>,
    },

    /// Inspect model architecture and details
    Inspect {
        /// Model name
        model: String,

        /// Export inspection results to JSON
        #[clap(short, long)]
        json: bool,
    },

    /// Quantize a model
    Quantize {
        /// Model name
        model: String,

        /// Quantization method (gguf, gptq, awq, ggml)
        #[clap(short, long)]
        method: String,

        /// Quantization format (e.g., Q4_0, Q4_1, Q5_0, Q5_1, Q8_0)
        #[clap(short, long)]
        format: Option<String>,

        /// Output model name
        #[clap(short, long)]
        output: Option<String>,
    },

    /// Manage model layers
    Layer {
        #[clap(subcommand)]
        action: LayerAction,
    },

    /// Check model compatibility for merging
    CompCheck {
        /// Model names to check compatibility
        models: Vec<String>,
    },

    /// Merge multiple models
    Merge {
        /// Model names to merge
        models: Vec<String>,

        /// Output model name
        #[clap(short, long)]
        output: String,

        /// Merge strategy (average, weighted, slerp)
        #[clap(short, long, default_value = "average")]
        strategy: String,

        /// Weights for weighted merge (comma-separated)
        #[clap(short, long)]
        weights: Option<String>,
    },

    /// Scan model for vulnerabilities
    Scan {
        /// Model name or URL
        model: String,

        /// Scan remote model
        #[clap(short, long)]
        remote: bool,

        /// Output format (html, json, cyclonedx)
        #[clap(short, long, default_value = "html")]
        format: String,

        /// Output file path
        #[clap(short, long)]
        output: Option<PathBuf>,
    },

    /// Manage remote model endpoints
    Remote {
        #[clap(subcommand)]
        action: RemoteAction,
    },

    /// Manage workflows
    Workflow {
        #[clap(subcommand)]
        action: WorkflowAction,
    },

    /// Start web UI
    Ui {
        /// Port to run UI on
        #[clap(short, long)]
        port: Option<u16>,

        /// Host to bind to
        #[clap(short = 'H', long)]
        host: Option<String>,

        /// Open browser automatically
        #[clap(short, long)]
        open: bool,
    },

    /// Index repository for RAG (Retrieval-Augmented Generation)
    Learn {
        /// Repository path to index
        #[clap(default_value = ".")]
        path: PathBuf,

        /// Show progress during indexing
        #[clap(short, long)]
        verbose: bool,

        /// Force re-index even if already indexed
        #[clap(short, long)]
        force: bool,
    },

    /// Manage chat history
    History {
        /// Number of recent messages to show
        #[clap(short, long)]
        limit: Option<usize>,

        /// Clear all history
        #[clap(short, long)]
        clear: bool,

        /// Filter by model name
        #[clap(short, long)]
        model: Option<String>,
    },

    /// Start MCP server
    Mcp {
        #[clap(subcommand)]
        action: McpAction,
    },

    /// Manage feature flags
    Features {
        #[clap(subcommand)]
        action: FeatureAction,
    },

    /// Manage request queue
    Queue {
        #[clap(subcommand)]
        action: QueueAction,
    },

    /// System diagnostics and health checks
    System {
        #[clap(subcommand)]
        action: SystemAction,
    },

    /// Monitor system performance and resources
    Monitor {
        #[clap(subcommand)]
        action: MonitorAction,
    },

    /// Backup and restore configuration
    Backup {
        #[clap(subcommand)]
        action: BackupAction,
    },

    /// Debug and troubleshooting tools
    Debug {
        #[clap(subcommand)]
        action: DebugAction,
    },

    /// Merge multiple models
    MergeModels {
        /// Model names to merge
        models: Vec<String>,

        /// Output model name
        #[clap(short, long)]
        output: String,

        /// Merge strategy (average, weighted, slerp)
        #[clap(short, long, default_value = "average")]
        strategy: String,

        /// Weights for weighted merge (comma-separated)
        #[clap(short, long)]
        weights: Option<String>,

        /// SLERP interpolation parameter (0.0 to 1.0)
        #[clap(long)]
        slerp_t: Option<f32>,

        /// Base model index for SLERP
        #[clap(long)]
        base_model: Option<usize>,

        /// Validate merged model
        #[clap(short, long)]
        validate: bool,

        /// Preserve metadata in merged model
        #[clap(long)]
        preserve_metadata: bool,
    },

    /// Show configuration
    Config {
        /// Show configuration path
        #[clap(short, long)]
        path: bool,

        /// Validate configuration
        #[clap(short, long)]
        validate: bool,
    },

    /// Start the Fuse API server
    Serve {
        /// Port to listen on
        #[clap(short, long)]
        port: Option<u16>,

        /// Host to bind to
        #[clap(short = 'H', long)]
        host: Option<String>,

        /// Enable Ollama-compatible API
        #[clap(long, default_value = "true")]
        ollama_api: bool,

        /// Enable OpenAI-compatible API
        #[clap(long, default_value = "true")]
        openai_api: bool,

        /// Enable Anthropic-compatible API
        #[clap(long, default_value = "true")]
        anthropic_api: bool,
    },

    /// Diagnose system health and configuration
    Doctor,
}

#[derive(Subcommand)]
pub enum LayerAction {
    /// Inspect model layers
    Inspect {
        /// Model name
        model: String,

        /// Show detailed information (wide format)
        #[clap(short = 'o', long)]
        wide: bool,
    },

    /// Remove a layer from a model
    Remove {
        /// Model name
        model: String,

        /// Layer ID to remove
        layer_id: String,
    },

    /// Add a layer to a model
    Add {
        /// Model name
        model: String,

        /// Layer type (geo-restriction, content-filter, custom)
        #[clap(short, long)]
        layer_type: String,

        /// Layer configuration file
        #[clap(short, long)]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum RemoteAction {
    /// Add a remote endpoint
    Add {
        /// Endpoint name
        name: String,

        /// Endpoint URL
        url: String,

        /// API key for authentication
        #[clap(short, long)]
        api_key: Option<String>,
    },

    /// Remove a remote endpoint
    Remove {
        /// Endpoint name
        name: String,
    },

    /// List all remote endpoints
    List,
}

#[derive(Subcommand)]
pub enum WorkflowAction {
    /// Run a workflow
    Run {
        /// Workflow file path
        workflow: PathBuf,

        /// Show detailed execution logs
        #[clap(short, long)]
        verbose: bool,
    },

    /// List available workflows
    List,

    /// Validate a workflow file
    Validate {
        /// Workflow file path
        workflow: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum McpAction {
    /// Start MCP server
    Start {
        /// Port to run MCP server on
        #[clap(short, long)]
        port: Option<u16>,

        /// Configuration file
        #[clap(short, long)]
        config: Option<PathBuf>,
    },

    /// Stop MCP server
    Stop,

    /// Show MCP server status
    Status,
}

#[derive(Subcommand)]
pub enum FeatureAction {
    /// List all feature flags and their status
    List,

    /// Enable a feature
    Enable {
        /// Feature name
        feature: String,
    },

    /// Disable a feature
    Disable {
        /// Feature name
        feature: String,
    },
}

#[derive(Subcommand)]
pub enum QueueAction {
    /// Show queue statistics
    Stats,

    /// Flush pending requests
    Flush,

    /// Check queue health
    Health,
}

#[derive(Subcommand)]
pub enum SystemAction {
    /// Check system capabilities
    Check,

    /// Show resource usage
    Resources,

    /// Show system health
    Health,
}

#[derive(Subcommand)]
pub enum MonitorAction {
    /// Show performance metrics
    Performance,

    /// Monitor resource usage
    Resources,

    /// Monitor queue status
    Queue,
}

#[derive(Subcommand)]
pub enum BackupAction {
    /// Create configuration backup
    Create,

    /// Restore from backup
    Restore {
        /// Backup file path
        file: PathBuf,
    },

    /// List available backups
    List,
}

#[derive(Subcommand)]
pub enum DebugAction {
    /// Show recent logs
    Logs,

    /// Validate configuration
    Config,

    /// Check connection pools
    Connections,

    /// Model loading diagnostics
    Models,
}

impl Cli {
    pub fn print_logo() {
        let logo = r#"
··································
:   __                       _ _ :
:  / _|_   _ ___  ___    ___| (_):
: | |_| | | / __|/ _ \  / __| | |:
: |  _| |_| \__ \  __/ | (__| | |:
: |_|  \__,_|___/\___|  \___|_|_|:
:                                :
··································
"#;
        println!("{}", logo);
    }
}
