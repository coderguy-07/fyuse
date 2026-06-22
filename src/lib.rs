// Core modules
pub mod cli;
pub mod config;
pub mod error;
pub mod logging;
pub mod model;
pub mod pool;
pub mod queue;
pub mod server;
pub mod storage;
pub mod system;

#[cfg(feature = "yew-ui")]
pub mod ui;

#[cfg(feature = "dioxus-ui")]
pub mod ui_dioxus;

pub mod compatibility;
pub mod layer;
pub mod quantization;
pub mod rag;
pub mod workflow;

pub mod observability;
pub mod security;
pub mod tui;

/// MCP server — gated behind the "mcp" feature until ToolContext is fully wired.
#[cfg(feature = "mcp")]
pub mod mcp;

// New architecture modules
pub mod agents;
pub mod api;
pub mod channels;
pub mod devices;
pub mod fleet;
pub mod inference;
pub mod k8s;
pub mod platform;
pub mod plugins;

#[cfg(test)]
pub mod test_helpers;

// Re-export commonly used types
pub use cli::Cli;
pub use config::{
    feature_flags::{Feature, FeatureFlagManager},
    DirectoryManager, FuseConfig,
};
pub use error::{ErrorResponse, FuseError, Result};
pub use model::{Auth, ModelManager, ModelMetadata, ModelSource, Provider};
pub use pool::{ConnectionPool, HttpConnectionPool, ModelPool, PoolConfig, PoolStats};
pub use queue::{Priority, QueueConfig, QueueStats, RequestQueue, ThreadId};
pub use server::{start_server, AppState};
pub use storage::{
    ConfigRepository, Database, DownloadManager, DownloadProgress, DownloadState,
    DownloadStateRepository, HistoryRepository, ModelRepository,
};
pub use system::{
    ModelCompatibility, ModelRequirements, QuantizationRecommendation, SystemCapabilities,
    SystemDetector,
};
