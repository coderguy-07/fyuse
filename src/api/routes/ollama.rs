//! Ollama-compatible API endpoints.
//!
//! Implements the Ollama REST API so existing Ollama clients can talk to Fuse
//! without modification.  Real inference is wired in a later milestone; for now
//! every endpoint returns a structurally-correct placeholder response so the
//! API contract and tests are established first (TDD RED → GREEN).

use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::server::ApiState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// POST /api/generate — request body.
#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub context: Option<Vec<u32>>,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

/// POST /api/generate — single response object (also used as the final
/// streaming chunk when `done == true`).
#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
}

/// POST /api/chat — a single message in the conversation.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images: Option<Vec<String>>,
}

/// POST /api/chat — request body.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

/// POST /api/chat — response body.
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
}

/// A single model entry in GET /api/tags.
#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    pub details: ModelDetails,
}

/// Model detail fields.
#[derive(Debug, Serialize)]
pub struct ModelDetails {
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>,
    pub parameter_size: String,
    pub quantization_level: String,
}

/// GET /api/tags — response body.
#[derive(Debug, Serialize)]
pub struct TagsResponse {
    pub models: Vec<ModelInfo>,
}

/// POST /api/pull — request body.
#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub name: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub stream: bool,
}

/// POST /api/pull — single progress line (NDJSON).
#[derive(Debug, Serialize)]
pub struct PullProgress {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<u64>,
}

/// GET /api/show — query parameters.
#[derive(Debug, Deserialize)]
pub struct ShowQuery {
    pub name: String,
}

/// GET /api/show — response body.
#[derive(Debug, Serialize)]
pub struct ShowResponse {
    pub modelfile: String,
    pub parameters: String,
    pub template: String,
    pub details: ModelDetails,
}

/// POST /api/embeddings — request body.
#[derive(Debug, Deserialize)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

/// POST /api/embeddings — response body.
#[derive(Debug, Serialize)]
pub struct EmbeddingsResponse {
    pub embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/generate`
///
/// For now returns a single non-streaming placeholder response regardless of
/// the `stream` flag.  Real streaming inference is wired in a later milestone.
pub async fn generate(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<GenerateRequest>,
) -> Json<GenerateResponse> {
    tracing::info!(model = %req.model, "generate request");

    Json(GenerateResponse {
        model: req.model,
        created_at: now_rfc3339(),
        response: "[placeholder — inference not yet wired]".to_string(),
        done: true,
        context: Some(vec![]),
        total_duration: Some(0),
        load_duration: Some(0),
        prompt_eval_count: Some(0),
        eval_count: Some(0),
        eval_duration: Some(0),
    })
}

/// `POST /api/chat`
pub async fn chat(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    tracing::info!(model = %req.model, messages = req.messages.len(), "chat request");

    Json(ChatResponse {
        model: req.model,
        created_at: now_rfc3339(),
        message: ChatMessage {
            role: "assistant".to_string(),
            content: "[placeholder — inference not yet wired]".to_string(),
            images: None,
        },
        done: true,
        total_duration: Some(0),
        eval_count: Some(0),
        eval_duration: Some(0),
    })
}

/// `GET /api/tags`
pub async fn tags(State(state): State<Arc<ApiState>>) -> Json<TagsResponse> {
    tracing::info!("tags request");

    // Scan models_dir for directories; each directory name is a model family.
    // In production the model manager will supply this list; for now we do a
    // best-effort scan so the endpoint is useful even without the full stack.
    let mut models: Vec<ModelInfo> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&state.models_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                models.push(ModelInfo {
                    name: name.clone(),
                    modified_at: now_rfc3339(),
                    size: 0,
                    digest: format!("sha256:{}", "0".repeat(64)),
                    details: ModelDetails {
                        format: "gguf".to_string(),
                        family: name,
                        families: None,
                        parameter_size: "unknown".to_string(),
                        quantization_level: "unknown".to_string(),
                    },
                });
            }
        }
    }

    Json(TagsResponse { models })
}

/// `POST /api/pull`
///
/// Returns a minimal NDJSON body with a single "success" status line.  Real
/// download logic is wired when the model manager is available.
pub async fn pull(State(_state): State<Arc<ApiState>>, Json(req): Json<PullRequest>) -> Response {
    tracing::info!(model = %req.name, "pull request");

    let progress = PullProgress {
        status: format!("pull for '{}' queued (not yet implemented)", req.name),
        digest: None,
        total: None,
        completed: None,
    };

    let ndjson = match serde_json::to_string(&progress) {
        Ok(s) => format!("{}\n", s),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/x-ndjson")
        .body(Body::from(ndjson))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `GET /api/show`
pub async fn show(
    State(_state): State<Arc<ApiState>>,
    Query(params): Query<ShowQuery>,
) -> Json<ShowResponse> {
    tracing::info!(model = %params.name, "show request");

    Json(ShowResponse {
        modelfile: format!("FROM {}\n", params.name),
        parameters: String::new(),
        template: "{{ .Prompt }}".to_string(),
        details: ModelDetails {
            format: "gguf".to_string(),
            family: params.name,
            families: None,
            parameter_size: "unknown".to_string(),
            quantization_level: "unknown".to_string(),
        },
    })
}

/// `POST /api/embeddings`
pub async fn embeddings(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<EmbeddingsRequest>,
) -> Json<EmbeddingsResponse> {
    tracing::info!(model = %req.model, "embeddings request");

    // Return a zero-vector of dimension 1; real embedding is wired later.
    Json(EmbeddingsResponse {
        embedding: vec![0.0_f32],
    })
}

// ---------------------------------------------------------------------------
// Route builder
// ---------------------------------------------------------------------------

/// Return an [`axum::Router`] containing all Ollama-compatible routes.
///
/// Merge this into the main application router via `Router::merge`.
pub fn router() -> axum::Router<Arc<ApiState>> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/api/generate", post(generate))
        .route("/api/chat", post(chat))
        .route("/api/tags", get(tags))
        .route("/api/pull", post(pull))
        .route("/api/show", get(show))
        .route("/api/embeddings", post(embeddings))
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
    async fn test_generate_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/api/generate", addr))
            .json(&serde_json::json!({
                "model": "test-model",
                "prompt": "Hello, world!",
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["model"], "test-model");
        assert!(body["response"].as_str().is_some());
        assert_eq!(body["done"], true);
    }

    #[tokio::test]
    async fn test_chat_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/api/chat", addr))
            .json(&serde_json::json!({
                "model": "test-model",
                "messages": [{"role": "user", "content": "Hi"}],
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["message"]["role"], "assistant");
        assert!(body["message"]["content"].as_str().is_some());
        assert_eq!(body["done"], true);
    }

    #[tokio::test]
    async fn test_tags_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/api/tags", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body["models"].is_array());
    }

    #[tokio::test]
    async fn test_pull_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/api/pull", addr))
            .json(&serde_json::json!({
                "name": "llama3:8b",
                "stream": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let text = resp.text().await.unwrap();
        // Response is NDJSON — parse the first line
        let first_line = text.lines().next().unwrap_or("");
        let parsed: serde_json::Value = serde_json::from_str(first_line).unwrap();
        assert!(parsed["status"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_show_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/api/show?name=llama3:8b", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body["modelfile"].as_str().is_some());
        assert!(body["details"].is_object());
    }

    #[tokio::test]
    async fn test_embeddings_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{}/api/embeddings", addr))
            .json(&serde_json::json!({
                "model": "nomic-embed-text",
                "prompt": "Hello world"
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body["embedding"].is_array());
    }
}
