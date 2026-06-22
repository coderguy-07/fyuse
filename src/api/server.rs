//! API server — axum-based HTTP server with middleware stack.

use crate::error::Result;
use axum::{extract::State, middleware, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

/// Shared application state.
#[derive(Clone)]
pub struct ApiState {
    pub models_dir: std::path::PathBuf,
    pub config: ApiConfig,
}

/// API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 11434,
            cors_origins: vec!["*".to_string()],
        }
    }
}

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

/// API server.
pub struct ApiServer {
    state: Arc<ApiState>,
}

impl ApiServer {
    pub fn new(state: ApiState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }

    /// Build the router with all routes and middleware.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health))
            .route("/api/version", get(version))
            .merge(crate::api::routes::ollama::router())
            .merge(crate::api::routes::openai::router())
            .merge(crate::api::routes::anthropic::router())
            .layer(middleware::from_fn(
                crate::server::middleware::auth_middleware,
            ))
            .layer(build_cors_layer(&self.state.config.cors_origins))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone())
    }

    /// Start the server with graceful shutdown.
    pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
        let router = self.router();

        tracing::info!("Fuse API server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            crate::error::FuseError::InternalError(format!("Failed to bind: {}", e))
        })?;

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| crate::error::FuseError::InternalError(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Start on a random port and return the address (for testing).
    pub async fn serve_test(&self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let router = self.router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| {
                crate::error::FuseError::InternalError(format!("Failed to bind: {}", e))
            })?;
        let addr = listener.local_addr().map_err(|e| {
            crate::error::FuseError::InternalError(format!("Failed to get addr: {}", e))
        })?;

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok((addr, handle))
    }
}

/// Build a CORS layer from the configured origins list.
/// If `cors_origins` contains `"*"` the layer is fully permissive (dev mode).
/// Otherwise only the listed origins are allowed.
fn build_cors_layer(cors_origins: &[String]) -> CorsLayer {
    if cors_origins.iter().any(|o| o == "*") {
        return CorsLayer::permissive();
    }
    let allowed: Vec<axum::http::HeaderValue> = cors_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    if allowed.is_empty() {
        // No valid origins configured — deny cross-origin by default
        CorsLayer::new()
    } else {
        CorsLayer::new().allow_origin(AllowOrigin::list(allowed))
    }
}

async fn health(State(_state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "fuse",
    }))
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");
    tracing::info!("Shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_state() -> ApiState {
        ApiState {
            models_dir: PathBuf::from("/tmp/test-models"),
            config: ApiConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/health", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body["version"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_version_endpoint() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/api/version", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["name"], "fuse");
    }

    #[tokio::test]
    async fn test_404_unknown_route() {
        let server = ApiServer::new(test_state());
        let (addr, _handle) = server.serve_test().await.unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{}/unknown", addr))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 404);
    }

    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 11434);
    }
}
