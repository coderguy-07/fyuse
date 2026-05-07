//! Shared test utilities for the Fuse project.
//!
//! Provides `test_config()`, `spawn_test_server()`, and mock factories
//! used across unit and integration tests.

#![cfg(test)]

use crate::config::FuseConfig;
use crate::error::Result;
use std::net::SocketAddr;

/// Create a minimal FuseConfig for testing.
pub fn test_config() -> FuseConfig {
    FuseConfig::default()
}

/// Create a temporary directory for test data.
pub fn test_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("Failed to create temp dir")
}

/// Spawn a test axum server and return its address.
/// The server binds to a random available port on localhost.
pub async fn spawn_test_server() -> Result<SocketAddr> {
    use axum::{routing::get, Router};

    let app = Router::new().route("/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(addr)
}

/// Create a test HTTP client for the given server address.
pub fn test_client(addr: SocketAddr) -> reqwest::Client {
    let _ = addr; // Used by callers to build URLs
    reqwest::Client::new()
}

/// Build a URL for the test server.
pub fn test_url(addr: SocketAddr, path: &str) -> String {
    format!("http://{}{}", addr, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_returns_default() {
        let config = test_config();
        assert!(!config.log_level.is_empty());
    }

    #[test]
    fn test_temp_dir_created() {
        let dir = test_temp_dir();
        assert!(dir.path().exists());
    }

    #[tokio::test]
    async fn test_spawn_test_server_health() {
        let addr = spawn_test_server().await.unwrap();
        let client = test_client(addr);
        let url = test_url(addr, "/health");

        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "ok");
    }
}
