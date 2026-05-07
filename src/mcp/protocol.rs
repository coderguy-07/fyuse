//! MCP Protocol definitions and message handling

use crate::error::{FuseError, Result};
use crate::mcp::tools::{Tool, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP protocol version
pub const MCP_VERSION: &str = "1.0.0";

/// MCP request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub version: String,
}

impl McpRequest {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
            version: MCP_VERSION.to_string(),
        }
    }

    pub fn parse_tool_call(&self) -> Result<Tool> {
        match self.method.as_str() {
            "fuse.pull_model" => {
                let params: PullModelParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::PullModel {
                    model_name: params.model_name,
                    source: params.source,
                })
            }
            "fuse.run_inference" => {
                let params: RunInferenceParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::RunInference {
                    model_name: params.model_name,
                    prompt: params.prompt,
                    max_tokens: params.max_tokens,
                })
            }
            "fuse.list_models" => Ok(Tool::ListModels),
            "fuse.inspect_model" => {
                let params: InspectModelParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::InspectModel {
                    model_name: params.model_name,
                })
            }
            "fuse.quantize_model" => {
                let params: QuantizeModelParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::QuantizeModel {
                    model_name: params.model_name,
                    method: params.method,
                    format: params.format,
                })
            }
            "fuse.scan_model" => {
                let params: ScanModelParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::ScanModel {
                    model_name: params.model_name,
                })
            }
            "fuse.merge_models" => {
                let params: MergeModelsParams = serde_json::from_value(
                    self.params.clone().unwrap_or_default()
                )?;
                Ok(Tool::MergeModels {
                    models: params.models,
                    strategy: params.strategy,
                })
            }
            _ => Err(FuseError::InternalError(format!("Unknown MCP method: {}", self.method))),
        }
    }
}

/// MCP response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
    pub version: String,
}

impl McpResponse {
    pub fn success(id: String, result: serde_json::Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
            version: MCP_VERSION.to_string(),
        }
    }

    pub fn error(id: String, error: McpError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
            version: MCP_VERSION.to_string(),
        }
    }

    pub fn from_tool_result(request_id: String, result: ToolResult) -> Self {
        if result.success {
            Self::success(request_id, serde_json::json!({
                "message": result.message,
                "data": result.data
            }))
        } else {
            Self::error(request_id, McpError {
                code: -32000,
                message: result.message,
                data: result.data,
            })
        }
    }
}

/// MCP error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// MCP notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub version: String,
}

impl McpNotification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            method: method.into(),
            params,
            version: MCP_VERSION.to_string(),
        }
    }
}

/// Tool parameter structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PullModelParams {
    model_name: String,
    source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunInferenceParams {
    model_name: String,
    prompt: String,
    max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InspectModelParams {
    model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuantizeModelParams {
    model_name: String,
    method: String,
    format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScanModelParams {
    model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeModelsParams {
    models: Vec<String>,
    strategy: String,
}

/// MCP server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Vec<ToolCapability>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            tools: vec![
                ToolCapability {
                    name: "fuse.pull_model".to_string(),
                    description: "Pull a model from a registry".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "model_name": {"type": "string"},
                            "source": {"type": "string", "optional": true}
                        },
                        "required": ["model_name"]
                    }),
                },
                ToolCapability {
                    name: "fuse.run_inference".to_string(),
                    description: "Run inference with a model".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "model_name": {"type": "string"},
                            "prompt": {"type": "string"},
                            "max_tokens": {"type": "integer", "optional": true}
                        },
                        "required": ["model_name", "prompt"]
                    }),
                },
                ToolCapability {
                    name: "fuse.list_models".to_string(),
                    description: "List all available models".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
                ToolCapability {
                    name: "fuse.inspect_model".to_string(),
                    description: "Inspect model architecture and metadata".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "model_name": {"type": "string"}
                        },
                        "required": ["model_name"]
                    }),
                },
                ToolCapability {
                    name: "fuse.quantize_model".to_string(),
                    description: "Quantize a model for efficiency".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "model_name": {"type": "string"},
                            "method": {"type": "string"},
                            "format": {"type": "string", "optional": true}
                        },
                        "required": ["model_name", "method"]
                    }),
                },
                ToolCapability {
                    name: "fuse.scan_model".to_string(),
                    description: "Scan model for vulnerabilities".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "model_name": {"type": "string"}
                        },
                        "required": ["model_name"]
                    }),
                },
                ToolCapability {
                    name: "fuse.merge_models".to_string(),
                    description: "Merge multiple models".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "models": {"type": "array", "items": {"type": "string"}},
                            "strategy": {"type": "string"}
                        },
                        "required": ["models", "strategy"]
                    }),
                },
            ],
            version: MCP_VERSION.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_request_creation() {
        let request = McpRequest::new("test.method", Some(serde_json::json!({"key": "value"})));
        assert_eq!(request.method, "test.method");
        assert!(request.params.is_some());
        assert_eq!(request.version, MCP_VERSION);
    }

    #[test]
    fn test_mcp_response_success() {
        let response = McpResponse::success("test-id".to_string(), serde_json::json!({"result": "ok"}));
        assert_eq!(response.id, "test-id");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_mcp_response_error() {
        let error = McpError {
            code: -32000,
            message: "Test error".to_string(),
            data: None,
        };
        let response = McpResponse::error("test-id".to_string(), error);
        assert_eq!(response.id, "test-id");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_server_capabilities() {
        let capabilities = ServerCapabilities::default();
        assert_eq!(capabilities.version, MCP_VERSION);
        assert!(!capabilities.tools.is_empty());

        // Check that all expected tools are present
        let tool_names: Vec<String> = capabilities.tools.iter().map(|t| t.name.clone()).collect();
        assert!(tool_names.contains(&"fuse.pull_model".to_string()));
        assert!(tool_names.contains(&"fuse.run_inference".to_string()));
        assert!(tool_names.contains(&"fuse.list_models".to_string()));
    }
}