use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use super::{AppState, HealthResponse};
use crate::model::{Image, ImageFormat, InferenceEngine, InferenceInput, InferenceParameters};
use crate::{FuseError, Result};

/// Health check handler
pub async fn health_check(State(_state): State<AppState>) -> Result<Json<HealthResponse>> {
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    }))
}

/// Inference request
#[derive(Debug, Deserialize)]
pub struct InferRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub images: Vec<ImageInput>,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub seed: Option<u64>,
}

/// Image input for API requests
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageInput {
    /// Base64-encoded image data
    pub data: String,
    /// Image format (png, jpg, gif, webp)
    pub format: String,
}

impl ImageInput {
    /// Convert to internal Image type
    fn to_image(&self) -> Result<Image> {
        let format = match self.format.to_lowercase().as_str() {
            "png" => ImageFormat::Png,
            "jpg" | "jpeg" => ImageFormat::Jpg,
            "gif" => ImageFormat::Gif,
            "webp" => ImageFormat::WebP,
            _ => {
                return Err(FuseError::ValidationError(format!(
                    "Unsupported image format: {}",
                    self.format
                )));
            }
        };

        Ok(Image::new(self.data.clone(), format))
    }
}

/// Inference response
#[derive(Debug, Serialize)]
pub struct InferResponse {
    pub model: String,
    pub response: String,
    pub formatted_response: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub inference_time_ms: u64,
}

/// Handle inference request
pub async fn infer(
    State(state): State<AppState>,
    Json(payload): Json<InferRequest>,
) -> Result<Json<InferResponse>> {
    info!(
        model = %payload.model,
        prompt_length = payload.prompt.len(),
        "Received inference request"
    );

    // Validate request
    if payload.prompt.is_empty() {
        return Err(FuseError::ValidationError(
            "Prompt cannot be empty".to_string(),
        ));
    }

    // Load model if not already loaded
    let handle = state.inference_engine.load_model(&payload.model).await?;

    // Convert images
    let images: Result<Vec<Image>> = payload.images.iter().map(|img| img.to_image()).collect();
    let images = images?;

    // Validate images
    for image in &images {
        image.validate()?;
    }

    // Build inference parameters
    let mut parameters = InferenceParameters::default();
    if let Some(max_tokens) = payload.max_tokens {
        parameters.max_tokens = max_tokens;
    }
    if let Some(temperature) = payload.temperature {
        parameters.temperature = temperature;
    }
    if let Some(top_p) = payload.top_p {
        parameters.top_p = top_p;
    }
    if let Some(top_k) = payload.top_k {
        parameters.top_k = Some(top_k);
    }
    if !payload.stop_sequences.is_empty() {
        parameters.stop_sequences = payload.stop_sequences;
    }
    if let Some(freq_penalty) = payload.frequency_penalty {
        parameters.frequency_penalty = Some(freq_penalty);
    }
    if let Some(pres_penalty) = payload.presence_penalty {
        parameters.presence_penalty = Some(pres_penalty);
    }
    if let Some(seed) = payload.seed {
        parameters.seed = Some(seed);
    }

    // Create inference input
    let input = InferenceInput {
        prompt: payload.prompt,
        images,
        context: None,
        parameters,
    };

    // Perform inference
    let output = state.inference_engine.infer(&handle, input).await?;

    info!(
        model = %payload.model,
        tokens = output.total_tokens,
        "Inference completed successfully"
    );

    // Build response
    let response = InferResponse {
        model: output.model,
        response: output.text,
        formatted_response: output.formatted_text,
        prompt_tokens: output.prompt_tokens,
        completion_tokens: output.completion_tokens,
        total_tokens: output.total_tokens,
        inference_time_ms: output
            .metadata
            .as_ref()
            .map(|m| m.inference_time_ms)
            .unwrap_or(0),
    };

    Ok(Json(response))
}

/// Streaming token response
#[derive(Debug, Serialize)]
struct StreamToken {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    is_final: bool,
}

/// Handle streaming inference request
pub async fn infer_stream(
    State(state): State<AppState>,
    Json(payload): Json<InferRequest>,
) -> Result<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>> {
    info!(
        model = %payload.model,
        prompt_length = payload.prompt.len(),
        "Received streaming inference request"
    );

    // Validate request
    if payload.prompt.is_empty() {
        return Err(FuseError::ValidationError(
            "Prompt cannot be empty".to_string(),
        ));
    }

    // Load model if not already loaded
    let handle = state.inference_engine.load_model(&payload.model).await?;

    // Convert images
    let images: Result<Vec<Image>> = payload.images.iter().map(|img| img.to_image()).collect();
    let images = images?;

    // Validate images
    for image in &images {
        image.validate()?;
    }

    // Build inference parameters
    let mut parameters = InferenceParameters::default();
    if let Some(max_tokens) = payload.max_tokens {
        parameters.max_tokens = max_tokens;
    }
    if let Some(temperature) = payload.temperature {
        parameters.temperature = temperature;
    }
    if let Some(top_p) = payload.top_p {
        parameters.top_p = top_p;
    }
    if let Some(top_k) = payload.top_k {
        parameters.top_k = Some(top_k);
    }
    if !payload.stop_sequences.is_empty() {
        parameters.stop_sequences = payload.stop_sequences;
    }
    if let Some(freq_penalty) = payload.frequency_penalty {
        parameters.frequency_penalty = Some(freq_penalty);
    }
    if let Some(pres_penalty) = payload.presence_penalty {
        parameters.presence_penalty = Some(pres_penalty);
    }
    if let Some(seed) = payload.seed {
        parameters.seed = Some(seed);
    }

    // Create inference input
    let input = InferenceInput {
        prompt: payload.prompt,
        images,
        context: None,
        parameters,
    };

    // Start streaming inference
    let mut rx = state.inference_engine.infer_stream(&handle, input).await?;

    // Create SSE stream
    let stream = async_stream::stream! {
        while let Some(token_result) = rx.recv().await {
            match token_result {
                Ok(token) => {
                    let stream_token = StreamToken {
                        text: token.text,
                        id: token.id,
                        is_final: token.is_final,
                    };

                    let json = serde_json::to_string(&stream_token).unwrap();
                    yield Ok(Event::default().data(json));
                }
                Err(e) => {
                    error!(error = %e, "Error during streaming inference");
                    let error_json = serde_json::json!({
                        "error": e.to_string()
                    });
                    yield Ok(Event::default().event("error").data(error_json.to_string()));
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream))
}

/// Model info response
#[derive(Debug, Serialize)]
pub struct ModelInfoResponse {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub architecture: Option<String>,
    pub parameter_count: Option<usize>,
    pub source: String,
    pub version: String,
    pub loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_usage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_busy: Option<bool>,
    pub tags: Vec<String>,
}

/// List all models
pub async fn list_models(State(state): State<AppState>) -> Result<Json<Vec<ModelInfoResponse>>> {
    info!("Listing all models");

    let models = state.model_manager.list().await?;

    let mut response = Vec::new();
    for model in models {
        let loaded = state.inference_engine.is_loaded(&model.name).await;

        let (memory_usage, is_busy) = if loaded {
            match state.inference_engine.get_model_info(&model.name).await {
                Ok(info) => (Some(info.memory_usage_bytes), Some(info.is_busy)),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        response.push(ModelInfoResponse {
            id: model.id.clone(),
            name: model.name.clone(),
            size_bytes: model.size_bytes,
            architecture: model.architecture.clone(),
            parameter_count: model.parameter_count,
            source: format!("{}", model.source),
            version: model.version.clone(),
            loaded,
            memory_usage_bytes: memory_usage,
            is_busy,
            tags: model.tags.clone(),
        });
    }

    info!(count = response.len(), "Listed models");

    Ok(Json(response))
}

/// Get model details
pub async fn get_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ModelInfoResponse>> {
    info!(model = %name, "Getting model details");

    let model = state
        .model_manager
        .get_metadata(&name)
        .await?
        .ok_or_else(|| FuseError::ModelNotFound(name.clone()))?;

    let loaded = state.inference_engine.is_loaded(&model.name).await;

    let (memory_usage, is_busy) = if loaded {
        match state.inference_engine.get_model_info(&model.name).await {
            Ok(info) => (Some(info.memory_usage_bytes), Some(info.is_busy)),
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    let response = ModelInfoResponse {
        id: model.id.clone(),
        name: model.name.clone(),
        size_bytes: model.size_bytes,
        architecture: model.architecture.clone(),
        parameter_count: model.parameter_count,
        source: format!("{}", model.source),
        version: model.version.clone(),
        loaded,
        memory_usage_bytes: memory_usage,
        is_busy,
        tags: model.tags.clone(),
    };

    Ok(Json(response))
}

/// Load model response
#[derive(Debug, Serialize)]
pub struct LoadModelResponse {
    pub model: String,
    pub handle_id: String,
    pub message: String,
}

/// Load model into memory
pub async fn load_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<LoadModelResponse>> {
    info!(model = %name, "Loading model into memory");

    // Check if model exists
    let _metadata = state
        .model_manager
        .get_metadata(&name)
        .await?
        .ok_or_else(|| FuseError::ModelNotFound(name.clone()))?;

    // Load the model
    let handle = state.inference_engine.load_model(&name).await?;

    info!(
        model = %name,
        handle_id = %handle.id,
        "Model loaded successfully"
    );

    Ok(Json(LoadModelResponse {
        model: name.clone(),
        handle_id: handle.id,
        message: format!("Model '{}' loaded successfully", name),
    }))
}

/// Unload model response
#[derive(Debug, Serialize)]
pub struct UnloadModelResponse {
    pub model: String,
    pub message: String,
}

/// Unload model from memory
pub async fn unload_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<UnloadModelResponse>> {
    info!(model = %name, "Unloading model from memory");

    // Check if model is loaded
    if !state.inference_engine.is_loaded(&name).await {
        return Err(FuseError::ValidationError(format!(
            "Model '{}' is not currently loaded",
            name
        )));
    }

    // Get model info to get the handle
    let _model_info = state.inference_engine.get_model_info(&name).await?;

    // Create a handle to unload (we need to reconstruct it)
    // In a real implementation, we'd store handles differently
    // For now, we'll just remove from cache directly
    let handle = state.inference_engine.load_model(&name).await?;
    state.inference_engine.unload_model(handle).await?;

    info!(model = %name, "Model unloaded successfully");

    Ok(Json(UnloadModelResponse {
        model: name.clone(),
        message: format!("Model '{}' unloaded successfully", name),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FuseConfig;
    use tempfile::TempDir;

    fn create_test_state() -> (AppState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = FuseConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        config.models_dir = temp_dir.path().join("models");

        std::fs::create_dir_all(&config.models_dir).unwrap();

        let state = AppState::from_config(config).unwrap();
        (state, temp_dir)
    }

    #[tokio::test]
    async fn test_health_check() {
        let (state, _temp) = create_test_state();
        let result = health_check(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.status, "healthy");
        assert_eq!(response.0.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_list_models_empty() {
        let (state, _temp) = create_test_state();
        let result = list_models(State(state)).await;

        assert!(result.is_ok());
        let models = result.unwrap();
        assert_eq!(models.0.len(), 0);
    }

    #[tokio::test]
    async fn test_infer_empty_prompt() {
        let (state, _temp) = create_test_state();

        let request = InferRequest {
            model: "test-model".to_string(),
            prompt: "".to_string(),
            images: vec![],
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: vec![],
            frequency_penalty: None,
            presence_penalty: None,
            seed: None,
        };

        let result = infer(State(state), Json(request)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[tokio::test]
    async fn test_infer_nonexistent_model() {
        let (state, _temp) = create_test_state();

        let request = InferRequest {
            model: "nonexistent-model".to_string(),
            prompt: "Test prompt".to_string(),
            images: vec![],
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: vec![],
            frequency_penalty: None,
            presence_penalty: None,
            seed: None,
        };

        let result = infer(State(state), Json(request)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ModelNotFound(_)));
    }

    #[tokio::test]
    async fn test_get_nonexistent_model() {
        let (state, _temp) = create_test_state();

        let result = get_model(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ModelNotFound(_)));
    }

    #[tokio::test]
    async fn test_load_nonexistent_model() {
        let (state, _temp) = create_test_state();

        let result = load_model(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ModelNotFound(_)));
    }

    #[tokio::test]
    async fn test_unload_not_loaded_model() {
        let (state, _temp) = create_test_state();

        let result = unload_model(State(state), Path("not-loaded".to_string())).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[tokio::test]
    async fn test_image_input_conversion() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let image_input = ImageInput {
            data: STANDARD.encode(b"test data"),
            format: "png".to_string(),
        };

        let result = image_input.to_image();
        assert!(result.is_ok());

        let image = result.unwrap();
        assert_eq!(image.format, ImageFormat::Png);
    }

    #[tokio::test]
    async fn test_image_input_invalid_format() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let image_input = ImageInput {
            data: STANDARD.encode(b"test data"),
            format: "invalid".to_string(),
        };

        let result = image_input.to_image();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }
}

/// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    #[serde(rename = "infer")]
    Infer {
        model: String,
        prompt: String,
        #[serde(default)]
        images: Vec<ImageInput>,
        #[serde(default)]
        parameters: Option<InferenceParameters>,
    },
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "ping")]
    Ping,
}

/// WebSocket response types
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum WsResponse {
    #[serde(rename = "token")]
    Token {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<u32>,
        is_final: bool,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "complete")]
    Complete {
        prompt_tokens: usize,
        completion_tokens: usize,
        total_tokens: usize,
    },
}

/// Handle WebSocket connections for streaming inference
pub async fn websocket_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: axum::extract::ws::WebSocket, state: AppState) {
    use axum::extract::ws::Message;
    use futures::sink::SinkExt;
    use futures::stream::StreamExt;

    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connection established");

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse incoming message
                let ws_msg: std::result::Result<WsMessage, _> = serde_json::from_str(&text);

                match ws_msg {
                    Ok(WsMessage::Infer {
                        model,
                        prompt,
                        images,
                        parameters,
                    }) => {
                        info!(model = %model, "WebSocket inference request");

                        // Load model
                        let handle = match state.inference_engine.load_model(&model).await {
                            Ok(h) => h,
                            Err(e) => {
                                let error_response = WsResponse::Error {
                                    message: e.to_string(),
                                };
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&error_response).unwrap(),
                                    ))
                                    .await;
                                continue;
                            }
                        };

                        // Convert images
                        let images_result: Result<Vec<Image>> =
                            images.iter().map(|img| img.to_image()).collect();

                        let images = match images_result {
                            Ok(imgs) => imgs,
                            Err(e) => {
                                let error_response = WsResponse::Error {
                                    message: e.to_string(),
                                };
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&error_response).unwrap(),
                                    ))
                                    .await;
                                continue;
                            }
                        };

                        // Build inference input
                        let input = InferenceInput {
                            prompt,
                            images,
                            context: None,
                            parameters: parameters.unwrap_or_default(),
                        };

                        // Start streaming inference
                        let mut rx = match state.inference_engine.infer_stream(&handle, input).await
                        {
                            Ok(r) => r,
                            Err(e) => {
                                let error_response = WsResponse::Error {
                                    message: e.to_string(),
                                };
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&error_response).unwrap(),
                                    ))
                                    .await;
                                continue;
                            }
                        };

                        // Stream tokens
                        let total_prompt_tokens = 0;
                        let mut total_completion_tokens = 0;

                        while let Some(token_result) = rx.recv().await {
                            match token_result {
                                Ok(token) => {
                                    total_completion_tokens += 1;

                                    let response = WsResponse::Token {
                                        text: token.text,
                                        id: token.id,
                                        is_final: token.is_final,
                                    };

                                    if sender
                                        .send(Message::Text(
                                            serde_json::to_string(&response).unwrap(),
                                        ))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let error_response = WsResponse::Error {
                                        message: e.to_string(),
                                    };
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&error_response).unwrap(),
                                        ))
                                        .await;
                                    break;
                                }
                            }
                        }

                        // Send completion message
                        let complete_response = WsResponse::Complete {
                            prompt_tokens: total_prompt_tokens,
                            completion_tokens: total_completion_tokens,
                            total_tokens: total_prompt_tokens + total_completion_tokens,
                        };
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&complete_response).unwrap(),
                            ))
                            .await;
                    }
                    Ok(WsMessage::Ping) => {
                        let pong_response = WsResponse::Pong;
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&pong_response).unwrap(),
                            ))
                            .await;
                    }
                    Ok(WsMessage::Cancel) => {
                        info!("WebSocket inference cancelled");
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to parse WebSocket message");
                        let error_response = WsResponse::Error {
                            message: format!("Invalid message format: {}", e),
                        };
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&error_response).unwrap(),
                            ))
                            .await;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed");
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }
            Ok(_) => {
                // Ignore other message types
            }
            Err(e) => {
                error!(error = %e, "WebSocket error");
                break;
            }
        }
    }

    info!("WebSocket connection terminated");
}
