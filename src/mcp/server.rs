//! MCP Server implementation using WebSocket transport

use crate::error::{FuseError, Result};
use crate::mcp::protocol::{McpRequest, McpResponse, McpNotification, ServerCapabilities};
use crate::mcp::tools::{Tool, ToolResult, ToolContext, execute_tool};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::accept_async;

/// MCP WebSocket server
pub struct McpServer {
    config: crate::mcp::McpConfig,
    tool_context: Arc<ToolContext>,
    shutdown_sender: tokio::sync::mpsc::Sender<()>,
    shutdown_receiver: Arc<RwLock<Option<tokio::sync::mpsc::Receiver<()>>>>,
}

impl McpServer {
    /// Create a new MCP server
    pub async fn new(config: crate::mcp::McpConfig) -> Result<Self> {
        let (shutdown_sender, shutdown_receiver) = tokio::sync::mpsc::channel(1);

        // TODO: Initialize tool context with actual services
        // For now, create placeholder context
        let tool_context = Arc::new(ToolContext {
            model_manager: Arc::new(todo!("Initialize ModelManager")),
            quantization_service: Arc::new(todo!("Initialize QuantizationService")),
            layer_inspector: Arc::new(todo!("Initialize LayerInspector")),
            vulnerability_scanner: Arc::new(todo!("Initialize VulnerabilityScanner")),
        });

        Ok(Self {
            config,
            tool_context,
            shutdown_sender,
            shutdown_receiver: Arc::new(RwLock::new(Some(shutdown_receiver))),
        })
    }

    /// Start the MCP server
    pub async fn start(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| FuseError::NetworkError(e.to_string()))?;

        log::info!("MCP server listening on {}", addr);

        let tool_context = Arc::clone(&self.tool_context);
        let mut shutdown_receiver = self.shutdown_receiver.write().await.take().unwrap();

        tokio::select! {
            _ = async {
                loop {
                    match listener.accept().await {
                        Ok((stream, addr)) => {
                            log::debug!("New MCP connection from {}", addr);
                            let tool_context = Arc::clone(&tool_context);

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, tool_context).await {
                                    log::error!("MCP connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            log::error!("MCP accept error: {}", e);
                            break;
                        }
                    }
                }
            } => {},
            _ = shutdown_receiver.recv() => {
                log::info!("MCP server shutting down");
            }
        }

        Ok(())
    }

    /// Stop the MCP server
    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_sender.send(()).await;
        Ok(())
    }

    /// Get server capabilities
    pub fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities::default()
    }
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: tokio::net::TcpStream,
    tool_context: Arc<ToolContext>,
) -> Result<()> {
    let ws_stream = accept_async(stream).await
        .map_err(|e| FuseError::NetworkError(e.to_string()))?;

    let (mut write, mut read) = ws_stream.split();

    // Send server capabilities on connection
    let capabilities = ServerCapabilities::default();
    let init_message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "initialize",
        "params": {
            "capabilities": capabilities,
            "version": capabilities.version
        }
    });

    write.send(Message::Text(serde_json::to_string(&init_message)?)).await
        .map_err(|e| FuseError::NetworkError(e.to_string()))?;

    // Handle incoming messages
    while let Some(message) = read.next().await {
        let message = message.map_err(|e| FuseError::NetworkError(e.to_string()))?;

        match message {
            Message::Text(text) => {
                if let Err(e) = handle_message(&text, &mut write, &tool_context).await {
                    log::error!("Error handling MCP message: {}", e);

                    // Send error response
                    let error_response = McpResponse::error(
                        "unknown".to_string(),
                        crate::mcp::protocol::McpError {
                            code: -32603,
                            message: e.to_string(),
                            data: None,
                        }
                    );

                    let response_json = serde_json::to_string(&error_response)?;
                    write.send(Message::Text(response_json)).await
                        .map_err(|e| FuseError::NetworkError(e.to_string()))?;
                }
            }
            Message::Close(_) => break,
            _ => {} // Ignore other message types
        }
    }

    Ok(())
}

/// Handle a single MCP message
async fn handle_message(
    text: &str,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message
    >,
    tool_context: &ToolContext,
) -> Result<()> {
    // Parse the incoming message
    let request: McpRequest = serde_json::from_str(text)?;

    // Parse tool call from request
    let tool = request.parse_tool_call()?;

    // Execute the tool
    let result = execute_tool(tool, tool_context).await?;

    // Create response
    let response = McpResponse::from_tool_result(request.id, result);

    // Send response
    let response_json = serde_json::to_string(&response)?;
    write.send(Message::Text(response_json)).await
        .map_err(|e| FuseError::NetworkError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpConfig;

    #[tokio::test]
    async fn test_mcp_server_creation() {
        let config = McpConfig::default();
        let server = McpServer::new(config).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_capabilities() {
        let config = McpConfig::default();
        let server = McpServer::new(config).await.unwrap();
        let capabilities = server.capabilities();
        assert_eq!(capabilities.version, crate::mcp::protocol::MCP_VERSION);
        assert!(!capabilities.tools.is_empty());
    }
}