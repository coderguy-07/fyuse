use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use crate::model::{LocalInferenceEngine, ModelManager};
use crate::storage::{Database, ModelRepository};
use crate::{ErrorResponse, FuseConfig, FuseError, Result};

pub mod handlers;
pub mod middleware;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<FuseConfig>,
    pub model_manager: Arc<ModelManager>,
    pub inference_engine: Arc<LocalInferenceEngine>,
    pub request_queue: Arc<crate::queue::RequestQueue>,
    pub system_detector: Arc<crate::system::SystemDetector>,
    pub model_pool: Arc<crate::pool::ModelPool<LocalInferenceEngine>>,
    pub http_pool: Arc<crate::pool::HttpConnectionPool>,
}

impl AppState {
    pub fn new(
        config: FuseConfig,
        model_manager: Arc<ModelManager>,
        inference_engine: Arc<LocalInferenceEngine>,
        request_queue: Arc<crate::queue::RequestQueue>,
        system_detector: Arc<crate::system::SystemDetector>,
        model_pool: Arc<crate::pool::ModelPool<LocalInferenceEngine>>,
        http_pool: Arc<crate::pool::HttpConnectionPool>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            model_manager,
            inference_engine,
            request_queue,
            system_detector,
            model_pool,
            http_pool,
        }
    }

    /// Create AppState from config with default components (config-driven)
    pub fn from_config(config: FuseConfig) -> Result<Self> {
        let db_path = config.data_dir.join("fuse.redb");
        let db = Arc::new(Database::new(db_path)?);
        let repository = Arc::new(ModelRepository::new(db));

        let model_manager = Arc::new(ModelManager::new(
            repository.clone(),
            config.models_dir.clone(),
        ));

        // Create inference engine with resource policy from config
        let resource_policy = config.resource_management.to_policy();
        let inference_engine = Arc::new(LocalInferenceEngine::with_resource_policy(
            repository,
            config.models_dir.clone(),
            resource_policy,
        ));

        // Create request queue
        let queue_config = crate::queue::QueueConfig::default();
        let request_queue = Arc::new(crate::queue::RequestQueue::new(queue_config));

        // Create system detector
        let system_detector = Arc::new(crate::system::SystemDetector::new());

        // Create model pool
        let model_pool = Arc::new(crate::pool::ModelPool::new(
            inference_engine.clone(),
            4, // max concurrent model loads
        ));

        // Create HTTP connection pool
        let pool_config = crate::pool::PoolConfig::default();
        let http_pool = Arc::new(crate::pool::HttpConnectionPool::new_http_pool(pool_config));

        Ok(Self::new(
            config,
            model_manager,
            inference_engine,
            request_queue,
            system_detector,
            model_pool,
            http_pool,
        ))
    }
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

/// Start the Axum web server
pub async fn start_server(config: FuseConfig) -> Result<()> {
    let addr = format!("{}:{}", config.server.host, config.server.port);

    info!("Starting Fuse server on {}", addr);

    let state = AppState::from_config(config)?;

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| FuseError::InternalError(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| FuseError::InternalError(format!("Server error: {}", e)))?;

    Ok(())
}

/// Create the application router with all routes and middleware
fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check endpoint (no auth required)
        .route("/health", get(handlers::health_check))
        // API v1 routes
        .nest("/api/v1", api_v1_routes())
        // Add shared state
        .with_state(state)
        // Add middleware layers
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(CompressionLayer::new())
                .layer(axum::middleware::from_fn(
                    middleware::security_headers_middleware,
                ))
                .layer(axum::middleware::from_fn(
                    middleware::input_validation_middleware,
                )),
        )
}

/// API v1 routes
fn api_v1_routes() -> Router<AppState> {
    Router::new()
        // Inference endpoints
        .route("/infer", post(handlers::infer))
        .route("/infer/stream", post(handlers::infer_stream))
        // WebSocket endpoint for streaming
        .route("/ws", axum::routing::get(handlers::websocket_handler))
        // Model management endpoints
        .route("/models", get(handlers::list_models))
        .route("/models/:name", get(handlers::get_model))
        .route("/models/:name/load", post(handlers::load_model))
        .route("/models/:name/unload", delete(handlers::unload_model))
}

/// Custom error response implementation for Axum
impl IntoResponse for FuseError {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from_u16(self.http_status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let error_response = ErrorResponse::from(self);

        (status_code, Json(error_response)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn create_test_state() -> (AppState, tempfile::TempDir) {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let mut config = FuseConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        config.models_dir = temp_dir.path().join("models");

        std::fs::create_dir_all(&config.models_dir).unwrap();

        let state = AppState::from_config(config).unwrap();
        (state, temp_dir)
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let (state, _temp) = create_test_state();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_v1_models_endpoint() {
        let (state, _temp) = create_test_state();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return OK even if no models are available
        assert_eq!(response.status(), StatusCode::OK);
    }
}
