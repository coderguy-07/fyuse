//! OpenAI-compatible API endpoints.
//!
//! Implements the OpenAI REST API so existing OpenAI clients (and tools like
//! LangChain, LiteLLM, etc.) can talk to Fuse without modification.  Real
//! inference is wired in a later milestone; for now every endpoint returns a
//! structurally-correct placeholder response so the API contract and tests are
//! established first (TDD RED → GREEN).

use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;

use crate::api::server::ApiState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A tool call returned by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

/// The function portion of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// A tool definition in the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// The function portion of a tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Response format specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

/// `POST /v1/chat/completions` — request body.
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// A single choice in a chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// `POST /v1/chat/completions` — response body (non-streaming).
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
}

/// Delta message in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// A single choice in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunkChoice {
    pub index: u32,
    pub delta: ChatCompletionDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// `POST /v1/chat/completions` — streaming chunk.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChunkChoice>,
}

/// `POST /v1/embeddings` — request body.
#[derive(Debug, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
}

/// A single embedding in the response.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: u32,
}

/// Embedding usage statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

/// `POST /v1/embeddings` — response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

/// A single model entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelData {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

/// `GET /v1/models` — response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelData>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn gen_completion_id() -> String {
    format!("chatcmpl-{}", uuid::Uuid::new_v4())
}

fn epoch_secs() -> i64 {
    Utc::now().timestamp()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /v1/chat/completions`
///
/// If `stream` is false, returns a single `ChatCompletionResponse`.
/// If `stream` is true, returns SSE events with `ChatCompletionChunk` followed
/// by `[DONE]`.
pub async fn chat_completions(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    tracing::info!(model = %req.model, messages = req.messages.len(), stream = req.stream, "chat completions request");

    let id = gen_completion_id();
    let created = epoch_secs();
    let model = req.model.clone();

    if req.stream {
        let stream = async_stream::stream! {
            // First chunk: role
            let chunk = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: model.clone(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: Some("assistant".to_string()),
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
            };
            let data = serde_json::to_string(&chunk).unwrap_or_default();
            yield Ok::<_, Infallible>(Event::default().data(data));

            // Second chunk: content
            let chunk = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: model.clone(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: Some("[placeholder — inference not yet wired]".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
            };
            let data = serde_json::to_string(&chunk).unwrap_or_default();
            yield Ok::<_, Infallible>(Event::default().data(data));

            // Final chunk: finish_reason
            let chunk = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: model.clone(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: Some("stop".to_string()),
                }],
            };
            let data = serde_json::to_string(&chunk).unwrap_or_default();
            yield Ok::<_, Infallible>(Event::default().data(data));

            // Done sentinel
            yield Ok::<_, Infallible>(Event::default().data("[DONE]"));
        };

        Sse::new(stream).into_response()
    } else {
        let response = ChatCompletionResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: Some("[placeholder — inference not yet wired]".to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        };
        Json(response).into_response()
    }
}

/// `POST /v1/embeddings`
pub async fn embeddings(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<EmbeddingRequest>,
) -> Json<EmbeddingResponse> {
    tracing::info!(model = %req.model, "embeddings request");

    // Determine how many inputs we have to generate embeddings for.
    let count = match &req.input {
        serde_json::Value::String(_) => 1,
        serde_json::Value::Array(arr) => arr.len(),
        _ => 1,
    };

    let data: Vec<EmbeddingData> = (0..count)
        .map(|i| EmbeddingData {
            object: "embedding".to_string(),
            embedding: vec![0.0_f32; 1536],
            index: i as u32,
        })
        .collect();

    Json(EmbeddingResponse {
        object: "list".to_string(),
        data,
        model: req.model,
        usage: EmbeddingUsage {
            prompt_tokens: 0,
            total_tokens: 0,
        },
    })
}

/// `GET /v1/models`
pub async fn models(State(state): State<Arc<ApiState>>) -> Json<ModelsResponse> {
    tracing::info!("models request");

    let mut data: Vec<ModelData> = Vec::new();
    let created = epoch_secs();

    if let Ok(entries) = std::fs::read_dir(&state.models_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                data.push(ModelData {
                    id: name,
                    object: "model".to_string(),
                    created,
                    owned_by: "fuse".to_string(),
                });
            }
        }
    }

    Json(ModelsResponse {
        object: "list".to_string(),
        data,
    })
}

// ---------------------------------------------------------------------------
// Route builder
// ---------------------------------------------------------------------------

/// Return an [`axum::Router`] containing all OpenAI-compatible routes.
///
/// Merge this into the main application router via `Router::merge`.
pub fn router() -> axum::Router<Arc<ApiState>> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(models))
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
    async fn test_chat_completions_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/chat/completions", addr))
            .json(&serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
        assert!(body["id"].as_str().unwrap().starts_with("chatcmpl-"));
        assert!(body["created"].as_i64().is_some());
        assert_eq!(body["model"], "gpt-4");
        assert!(body["choices"].is_array());
        assert_eq!(body["choices"][0]["index"], 0);
        assert_eq!(body["choices"][0]["message"]["role"], "assistant");
        assert!(body["choices"][0]["message"]["content"].as_str().is_some());
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert!(body["usage"]["prompt_tokens"].is_number());
        assert!(body["usage"]["completion_tokens"].is_number());
        assert!(body["usage"]["total_tokens"].is_number());
    }

    #[tokio::test]
    async fn test_chat_completions_streaming() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/chat/completions", addr))
            .json(&serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let content_type = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            content_type.contains("text/event-stream"),
            "Expected SSE content type, got: {}",
            content_type
        );

        let text = resp.text().await.unwrap();
        // Should contain data lines and [DONE]
        assert!(
            text.contains("data:"),
            "SSE response should contain data lines"
        );
        assert!(
            text.contains("[DONE]"),
            "SSE response should end with [DONE]"
        );
        assert!(
            text.contains("chat.completion.chunk"),
            "SSE response should contain chunk objects"
        );
    }

    #[tokio::test]
    async fn test_embeddings_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/embeddings", addr))
            .json(&serde_json::json!({
                "model": "text-embedding-ada-002",
                "input": "Hello world"
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "list");
        assert!(body["data"].is_array());
        assert_eq!(body["data"][0]["object"], "embedding");
        assert!(body["data"][0]["embedding"].is_array());
        assert_eq!(body["data"][0]["embedding"].as_array().unwrap().len(), 1536);
        assert_eq!(body["data"][0]["index"], 0);
        assert_eq!(body["model"], "text-embedding-ada-002");
        assert!(body["usage"]["prompt_tokens"].is_number());
        assert!(body["usage"]["total_tokens"].is_number());
    }

    #[tokio::test]
    async fn test_models_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/v1/models", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "list");
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_chat_completions_with_tools() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/chat/completions", addr))
            .json(&serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "What is the weather?"}],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get the current weather",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "location": {"type": "string"}
                            },
                            "required": ["location"]
                        }
                    }
                }],
                "tool_choice": "auto",
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
        assert!(body["choices"].is_array());
        assert_eq!(body["choices"][0]["message"]["role"], "assistant");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
    }

    #[tokio::test]
    async fn test_chat_completions_json_mode() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/v1/chat/completions", addr))
            .json(&serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Return JSON"}],
                "response_format": {"type": "json_object"},
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
        assert!(body["choices"][0]["message"]["content"].as_str().is_some());
    }
}
