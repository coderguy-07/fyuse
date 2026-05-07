//! MCP (Model Context Protocol) server and client [8.3].
//!
//! Exposes Fuse tools via MCP protocol and connects to external MCP tool servers.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// An MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for tool input.
    pub input_schema: serde_json::Value,
}

/// An MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Resource URI.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the resource.
    pub description: String,
    /// MIME type of the resource content.
    pub mime_type: String,
}

/// Configuration for the MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Port to listen on.
    pub port: u16,
    /// Registered tools.
    pub tools: Vec<McpTool>,
    /// Registered resources.
    pub resources: Vec<McpResource>,
}

impl McpServerConfig {
    /// Validate the server configuration.
    pub fn validate(&self) -> Result<()> {
        if self.port == 0 {
            return Err(FuseError::ValidationError(
                "MCP server port must be non-zero".to_string(),
            ));
        }

        // Check for duplicate tool names
        let mut tool_names = std::collections::HashSet::new();
        for tool in &self.tools {
            if tool.name.is_empty() {
                return Err(FuseError::ValidationError(
                    "Tool name must not be empty".to_string(),
                ));
            }
            if !tool_names.insert(&tool.name) {
                return Err(FuseError::ValidationError(format!(
                    "Duplicate tool name: {}",
                    tool.name
                )));
            }
        }

        // Check for duplicate resource URIs
        let mut resource_uris = std::collections::HashSet::new();
        for resource in &self.resources {
            if resource.uri.is_empty() {
                return Err(FuseError::ValidationError(
                    "Resource URI must not be empty".to_string(),
                ));
            }
            if !resource_uris.insert(&resource.uri) {
                return Err(FuseError::ValidationError(format!(
                    "Duplicate resource URI: {}",
                    resource.uri
                )));
            }
        }

        Ok(())
    }
}

/// MCP tool execution handler.
pub type McpToolHandler = Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value> + Send + Sync>;

/// MCP Server — exposes Fuse tools via the Model Context Protocol.
pub struct McpServer {
    config: McpServerConfig,
    handlers: HashMap<String, McpToolHandler>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration.
    pub fn new(config: McpServerConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            handlers: HashMap::new(),
        })
    }

    /// Register a tool handler.
    pub fn register_tool_handler(
        &mut self,
        tool_name: &str,
        handler: McpToolHandler,
    ) -> Result<()> {
        if !self.config.tools.iter().any(|t| t.name == tool_name) {
            return Err(FuseError::ValidationError(format!(
                "Tool not found in config: {tool_name}"
            )));
        }
        self.handlers.insert(tool_name.to_string(), handler);
        Ok(())
    }

    /// List registered tools.
    pub fn list_tools(&self) -> &[McpTool] {
        &self.config.tools
    }

    /// List registered resources.
    pub fn list_resources(&self) -> &[McpResource] {
        &self.config.resources
    }

    /// Execute a tool by name.
    pub fn execute_tool(&self, name: &str, input: serde_json::Value) -> Result<serde_json::Value> {
        let handler = self.handlers.get(name).ok_or_else(|| {
            FuseError::ValidationError(format!("No handler registered for tool: {name}"))
        })?;
        handler(input)
    }

    /// Get the server configuration.
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }
}

/// MCP Client — connects to external MCP tool servers.
pub struct McpClient {
    /// Server endpoint URL.
    server_url: String,
    /// Cached tool list from the server.
    tools: Vec<McpTool>,
    /// Cached resource list from the server.
    resources: Vec<McpResource>,
}

impl McpClient {
    /// Create a new MCP client targeting the given server URL.
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            tools: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// Get the server URL.
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// Refresh the tool and resource lists from the server.
    ///
    /// Stub — real implementation will use HTTP/JSON-RPC.
    pub async fn refresh(&mut self) -> Result<()> {
        // TODO: Connect to MCP server and fetch tool/resource lists
        Ok(())
    }

    /// Get cached tools.
    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    /// Get cached resources.
    pub fn resources(&self) -> &[McpResource] {
        &self.resources
    }

    /// Call a tool on the remote server.
    ///
    /// Stub — real implementation will send JSON-RPC request.
    pub async fn call_tool(
        &self,
        _name: &str,
        _input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        Err(FuseError::FeatureDisabled(
            "MCP client remote calls not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool() -> McpTool {
        McpTool {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        }
    }

    fn sample_resource() -> McpResource {
        McpResource {
            uri: "file:///docs/readme.md".to_string(),
            name: "README".to_string(),
            description: "Project readme".to_string(),
            mime_type: "text/markdown".to_string(),
        }
    }

    fn sample_config() -> McpServerConfig {
        McpServerConfig {
            port: 8080,
            tools: vec![sample_tool()],
            resources: vec![sample_resource()],
        }
    }

    #[test]
    fn test_mcp_tool_creation() {
        let tool = sample_tool();
        assert_eq!(tool.name, "search");
        assert!(!tool.description.is_empty());
    }

    #[test]
    fn test_mcp_resource_creation() {
        let resource = sample_resource();
        assert_eq!(resource.uri, "file:///docs/readme.md");
        assert_eq!(resource.mime_type, "text/markdown");
    }

    #[test]
    fn test_config_validation_valid() {
        let config = sample_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_zero_port() {
        let config = McpServerConfig {
            port: 0,
            tools: vec![],
            resources: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_duplicate_tool_names() {
        let tool = sample_tool();
        let config = McpServerConfig {
            port: 8080,
            tools: vec![tool.clone(), tool],
            resources: vec![],
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate tool name"));
    }

    #[test]
    fn test_config_validation_empty_tool_name() {
        let config = McpServerConfig {
            port: 8080,
            tools: vec![McpTool {
                name: String::new(),
                description: "empty".to_string(),
                input_schema: serde_json::json!({}),
            }],
            resources: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_duplicate_resource_uris() {
        let resource = sample_resource();
        let config = McpServerConfig {
            port: 8080,
            tools: vec![],
            resources: vec![resource.clone(), resource],
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate resource URI"));
    }

    #[test]
    fn test_server_creation() {
        let config = sample_config();
        let server = McpServer::new(config);
        assert!(server.is_ok());
    }

    #[test]
    fn test_server_list_tools() {
        let config = sample_config();
        let server = McpServer::new(config).expect("server creation failed");
        assert_eq!(server.list_tools().len(), 1);
        assert_eq!(server.list_tools()[0].name, "search");
    }

    #[test]
    fn test_server_list_resources() {
        let config = sample_config();
        let server = McpServer::new(config).expect("server creation failed");
        assert_eq!(server.list_resources().len(), 1);
        assert_eq!(server.list_resources()[0].name, "README");
    }

    #[test]
    fn test_tool_registration_and_execution() {
        let config = sample_config();
        let mut server = McpServer::new(config).expect("server creation failed");

        let handler: McpToolHandler = Arc::new(|input| {
            let query = input
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            Ok(serde_json::json!({ "results": [query] }))
        });

        server
            .register_tool_handler("search", handler)
            .expect("handler registration failed");

        let result = server
            .execute_tool("search", serde_json::json!({ "query": "rust" }))
            .expect("tool execution failed");

        assert_eq!(result["results"][0], "rust");
    }

    #[test]
    fn test_tool_registration_unknown_tool() {
        let config = sample_config();
        let mut server = McpServer::new(config).expect("server creation failed");
        let handler: McpToolHandler = Arc::new(|_| Ok(serde_json::json!({})));
        let result = server.register_tool_handler("nonexistent", handler);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_unregistered_tool() {
        let config = sample_config();
        let server = McpServer::new(config).expect("server creation failed");
        let result = server.execute_tool("search", serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_client_creation() {
        let client = McpClient::new("http://localhost:8080");
        assert_eq!(client.server_url(), "http://localhost:8080");
        assert!(client.tools().is_empty());
        assert!(client.resources().is_empty());
    }

    #[tokio::test]
    async fn test_client_call_tool_stub() {
        let client = McpClient::new("http://localhost:8080");
        let result = client
            .call_tool("search", serde_json::json!({"query": "test"}))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_tool_serialization() {
        let tool = sample_tool();
        let json = serde_json::to_string(&tool).expect("serialize failed");
        let deserialized: McpTool = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.name, tool.name);
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = sample_resource();
        let json = serde_json::to_string(&resource).expect("serialize failed");
        let deserialized: McpResource = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.uri, resource.uri);
    }

    #[test]
    fn test_server_config_accessor() {
        let config = sample_config();
        let server = McpServer::new(config.clone()).expect("server creation failed");
        assert_eq!(server.config().port, 8080);
    }
}
