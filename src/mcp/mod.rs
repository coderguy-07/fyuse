//! Model Context Protocol (MCP) Server Implementation
//!
//! This module provides MCP server functionality that allows external tools
//! and IDEs to interact with Fuse's AI model management capabilities.

pub mod server;
pub mod tools;
pub mod protocol;

pub use server::McpServer;
pub use tools::{Tool, ToolResult};
pub use protocol::{McpRequest, McpResponse, McpNotification};

use crate::error::{FuseError, Result};
use crate::config::FuseConfig;
use std::sync::Arc;

/// Configuration for MCP server
#[derive(Debug, Clone)]
pub struct McpConfig {
    pub host: String,
    pub port: u16,
    pub auth_required: bool,
    pub allowed_origins: Vec<String>,
    pub max_connections: usize,
    pub request_timeout_secs: u64,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            auth_required: false,
            allowed_origins: vec!["*".to_string()],
            max_connections: 100,
            request_timeout_secs: 30,
        }
    }
}

impl From<&FuseConfig> for McpConfig {
    fn from(config: &FuseConfig) -> Self {
        Self {
            host: config.server.host.clone(),
            port: 3000, // MCP-specific port
            auth_required: false, // TODO: Configure from feature flags
            allowed_origins: vec!["*".to_string()],
            max_connections: config.server.max_connections,
            request_timeout_secs: 30,
        }
    }
}

/// Main MCP server struct
pub struct McpService {
    config: McpConfig,
    server: Option<McpServer>,
}

impl McpService {
    /// Create a new MCP service
    pub fn new(config: McpConfig) -> Self {
        Self {
            config,
            server: None,
        }
    }

    /// Create from Fuse configuration
    pub fn from_config(config: &FuseConfig) -> Self {
        Self::new(config.into())
    }

    /// Start the MCP server
    pub async fn start(&mut self) -> Result<()> {
        let server = McpServer::new(self.config.clone()).await?;
        self.server = Some(server);
        Ok(())
    }

    /// Stop the MCP server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(server) = self.server.take() {
            server.stop().await?;
        }
        Ok(())
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.server.is_some()
    }

    /// Get server status
    pub fn status(&self) -> McpStatus {
        if self.is_running() {
            McpStatus::Running {
                host: self.config.host.clone(),
                port: self.config.port,
            }
        } else {
            McpStatus::Stopped
        }
    }
}

/// MCP server status
#[derive(Debug, Clone)]
pub enum McpStatus {
    Running { host: String, port: u16 },
    Stopped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_default() {
        let config = McpConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(!config.auth_required);
    }

    #[test]
    fn test_mcp_service_creation() {
        let config = McpConfig::default();
        let service = McpService::new(config);
        assert!(!service.is_running());
        assert!(matches!(service.status(), McpStatus::Stopped));
    }
}