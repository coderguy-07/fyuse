//! Integration test entry point.
//! Verifies that the test infrastructure works end-to-end.

use fuse::config::FuseConfig;

#[test]
fn test_config_loads_default() {
    let config = FuseConfig::default();
    assert!(!config.log_level.is_empty());
}

#[tokio::test]
async fn test_health_endpoint() {
    use axum::{routing::get, Router};

    let app = Router::new().route("/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
