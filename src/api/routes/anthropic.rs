//! Anthropic-compatible API endpoints.
//!
//! Implements the Anthropic Messages API so existing Anthropic clients can talk
//! to Fuse without modification.  Real inference is wired in a later milestone;
//! for now every endpoint returns a structurally-correct placeholder response so
//! the API contract and tests are established first (TDD RED → GREEN).

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;

use crate::api::server::ApiState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// A single message in the conversation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Message content — either a plain string or an array of content blocks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<Content>),
}

/// Content block within a message.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
}

/// Image source for image content blocks.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// Metadata attached to a messages request.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MessageMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// POST /v1/messages — request body.
#[derive(Debug, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: Option<MessageMetadata>,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// POST /v1/messages — response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<Content>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

// ---------------------------------------------------------------------------
// Streaming types
// ---------------------------------------------------------------------------

/// Delta within a content block during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

/// Delta for the message-level update at the end of streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Server-sent event types for the Anthropic streaming protocol.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessagesResponse },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: u32, content_block: Content },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: ContentDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDelta, usage: Usage },
    #[serde(rename = "message_stop")]
    MessageStop {},
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

impl MessagesRequest {
    fn validate(&self) -> Result<(), String> {
        if self.messages.is_empty() {
            return Err("messages must not be empty".to_string());
        }
        if self.max_tokens == 0 {
            return Err("max_tokens must be greater than 0".to_string());
        }
        if let Some(t) = self.temperature {
            if !(0.0..=1.0).contains(&t) {
                return Err("temperature must be between 0.0 and 1.0".to_string());
            }
        }
        if let Some(tp) = self.top_p {
            if !(0.0..=1.0).contains(&tp) {
                return Err("top_p must be between 0.0 and 1.0".to_string());
            }
        }
        Ok(())
    }
}

fn invalid_request_error(msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": msg
            }
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn new_message_id() -> String {
    format!("msg_{}", uuid::Uuid::new_v4())
}

fn placeholder_response(model: String) -> MessagesResponse {
    MessagesResponse {
        id: new_message_id(),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![Content::Text {
            text: "[placeholder — inference not yet wired]".to_string(),
        }],
        model,
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 0,
            output_tokens: 0,
        },
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /v1/messages`
///
/// If `stream` is false (or absent), returns a single JSON response.
/// If `stream` is true, returns an SSE stream following the Anthropic
/// streaming protocol.
pub async fn messages(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<MessagesRequest>,
) -> Response {
    if let Err(msg) = req.validate() {
        return invalid_request_error(&msg);
    }

    tracing::info!(model = %req.model, messages = req.messages.len(), "anthropic messages request");

    let stream = req.stream.unwrap_or(false);

    if stream {
        let model = req.model.clone();
        let msg_id = new_message_id();

        let events: Vec<Result<Event, Infallible>> = vec![
            // message_start
            Ok(Event::default().event("message_start").data(
                serde_json::to_string(&StreamEvent::MessageStart {
                    message: MessagesResponse {
                        id: msg_id,
                        response_type: "message".to_string(),
                        role: "assistant".to_string(),
                        content: vec![],
                        model: model.clone(),
                        stop_reason: None,
                        stop_sequence: None,
                        usage: Usage {
                            input_tokens: 0,
                            output_tokens: 0,
                        },
                    },
                })
                .unwrap_or_default(),
            )),
            // content_block_start
            Ok(Event::default().event("content_block_start").data(
                serde_json::to_string(&StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text {
                        text: String::new(),
                    },
                })
                .unwrap_or_default(),
            )),
            // content_block_delta
            Ok(Event::default().event("content_block_delta").data(
                serde_json::to_string(&StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta {
                        text: "[placeholder — inference not yet wired]".to_string(),
                    },
                })
                .unwrap_or_default(),
            )),
            // content_block_stop
            Ok(Event::default().event("content_block_stop").data(
                serde_json::to_string(&StreamEvent::ContentBlockStop { index: 0 })
                    .unwrap_or_default(),
            )),
            // message_delta
            Ok(Event::default().event("message_delta").data(
                serde_json::to_string(&StreamEvent::MessageDelta {
                    delta: MessageDelta {
                        stop_reason: Some("end_turn".to_string()),
                        stop_sequence: None,
                    },
                    usage: Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                })
                .unwrap_or_default(),
            )),
            // message_stop
            Ok(Event::default()
                .event("message_stop")
                .data(serde_json::to_string(&StreamEvent::MessageStop {}).unwrap_or_default())),
        ];

        let event_stream = stream::iter(events);
        Sse::new(event_stream).into_response()
    } else {
        Json(placeholder_response(req.model)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Route builder
// ---------------------------------------------------------------------------

/// Return an [`axum::Router`] containing all Anthropic-compatible routes.
///
/// Merge this into the main application router via `Router::merge`.
pub fn router() -> axum::Router<Arc<ApiState>> {
    use axum::routing::post;

    axum::Router::new().route("/v1/messages", post(messages))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::server::{ApiConfig, ApiServer, ApiState};
    use std::path::PathBuf;

    fn test_state() -> ApiState {
        ApiState {
            models_dir: PathBuf::from("/tmp/test-models"),
            config: ApiConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_messages_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [{"role": "user", "content": "Hello!"}],
                "max_tokens": 1024
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["role"], "assistant");
        assert!(body["content"].is_array());
        assert!(!body["content"].as_array().unwrap().is_empty());
        assert_eq!(body["content"][0]["type"], "text");
        assert!(body["content"][0]["text"].as_str().is_some());
        assert!(body["id"].as_str().unwrap().starts_with("msg_"));
        assert!(body["usage"]["input_tokens"].is_number());
        assert!(body["usage"]["output_tokens"].is_number());
    }

    #[tokio::test]
    async fn test_messages_streaming() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [{"role": "user", "content": "Hello!"}],
                "max_tokens": 1024,
                "stream": true
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        assert!(resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("text/event-stream"));

        let body = resp.text().await.unwrap();

        // Verify all expected event types are present
        assert!(
            body.contains("event: message_start"),
            "missing message_start"
        );
        assert!(
            body.contains("event: content_block_start"),
            "missing content_block_start"
        );
        assert!(
            body.contains("event: content_block_delta"),
            "missing content_block_delta"
        );
        assert!(
            body.contains("event: content_block_stop"),
            "missing content_block_stop"
        );
        assert!(
            body.contains("event: message_delta"),
            "missing message_delta"
        );
        assert!(body.contains("event: message_stop"), "missing message_stop");

        // Verify the delta contains placeholder text
        assert!(body.contains("placeholder"));
    }

    #[tokio::test]
    async fn test_messages_with_system() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [{"role": "user", "content": "Hello!"}],
                "max_tokens": 1024,
                "system": "You are a helpful assistant."
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["role"], "assistant");
        assert!(body["content"].is_array());
        assert!(!body["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_messages_multi_turn() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [
                    {"role": "user", "content": "What is 2+2?"},
                    {"role": "assistant", "content": "4"},
                    {"role": "user", "content": "And what is 3+3?"}
                ],
                "max_tokens": 1024
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["role"], "assistant");
        assert_eq!(body["model"], "claude-3-sonnet");
        assert!(body["content"].is_array());
        assert!(!body["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_messages_empty_messages_rejected() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [],
                "max_tokens": 100
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["type"], "error");
    }

    #[tokio::test]
    async fn test_messages_zero_max_tokens_rejected() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/messages", addr))
            .json(&serde_json::json!({
                "model": "claude-3-sonnet",
                "messages": [{"role": "user", "content": "hi"}],
                "max_tokens": 0
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("max_tokens"));
    }
}
