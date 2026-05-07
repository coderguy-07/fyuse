use crate::error::{FuseError, Result};
use crate::model::{Auth, RemoteEndpoint};
use reqwest::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for retry logic
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        }
    }
}

/// Request to be forwarded to remote endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Path to append to endpoint URL
    pub path: String,
    /// Request headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// Request body (optional)
    pub body: Option<serde_json::Value>,
}

/// Response from remote endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: std::collections::HashMap<String, String>,
    /// Response body
    pub body: serde_json::Value,
}

/// Remote model proxy for forwarding requests to remote endpoints
pub struct RemoteModelProxy {
    client: Client,
    retry_config: RetryConfig,
}

impl RemoteModelProxy {
    /// Create a new remote model proxy
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| FuseError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            retry_config: RetryConfig::default(),
        })
    }

    /// Create a new remote model proxy with custom retry configuration
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Forward a request to a remote endpoint
    pub async fn forward_request(
        &self,
        endpoint: &RemoteEndpoint,
        request: ProxyRequest,
    ) -> Result<ProxyResponse> {
        // Validate endpoint is enabled
        if !endpoint.enabled {
            return Err(FuseError::ValidationError(format!(
                "Remote endpoint '{}' is disabled",
                endpoint.name
            )));
        }

        // Build full URL
        let url = format!("{}{}", endpoint.url.trim_end_matches('/'), request.path);

        tracing::debug!(
            endpoint_name = %endpoint.name,
            url = %url,
            method = %request.method,
            "Forwarding request to remote endpoint"
        );

        // Execute request with retry logic
        self.execute_with_retry(endpoint, &url, &request).await
    }

    /// Execute request with retry logic
    async fn execute_with_retry(
        &self,
        endpoint: &RemoteEndpoint,
        url: &str,
        request: &ProxyRequest,
    ) -> Result<ProxyResponse> {
        let mut attempt = 0;
        let mut backoff = self.retry_config.initial_backoff;

        loop {
            attempt += 1;

            match self.execute_request(endpoint, url, request).await {
                Ok(response) => {
                    tracing::debug!(
                        endpoint_name = %endpoint.name,
                        attempt = %attempt,
                        status = %response.status,
                        "Request succeeded"
                    );
                    return Ok(response);
                }
                Err(e) => {
                    // Check if error is retryable
                    if !e.is_retryable() || attempt >= self.retry_config.max_retries {
                        tracing::error!(
                            endpoint_name = %endpoint.name,
                            attempt = %attempt,
                            error = %e,
                            "Request failed after retries"
                        );
                        return Err(e);
                    }

                    tracing::warn!(
                        endpoint_name = %endpoint.name,
                        attempt = %attempt,
                        backoff_ms = %backoff.as_millis(),
                        error = %e,
                        "Request failed, retrying"
                    );

                    // Wait before retrying
                    tokio::time::sleep(backoff).await;

                    // Increase backoff for next attempt
                    backoff = Duration::from_millis(
                        (backoff.as_millis() as f64 * self.retry_config.backoff_multiplier) as u64,
                    )
                    .min(self.retry_config.max_backoff);
                }
            }
        }
    }

    /// Execute a single request attempt
    async fn execute_request(
        &self,
        endpoint: &RemoteEndpoint,
        url: &str,
        request: &ProxyRequest,
    ) -> Result<ProxyResponse> {
        // Build request
        let mut req_builder = match request.method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            "HEAD" => self.client.head(url),
            method => {
                return Err(FuseError::ValidationError(format!(
                    "Unsupported HTTP method: {}",
                    method
                )));
            }
        };

        // Add authentication if configured
        if let Some(auth) = &endpoint.auth {
            req_builder = self.add_auth(req_builder, auth)?;
        }

        // Add custom headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add body if present
        if let Some(body) = &request.body {
            req_builder = req_builder.json(body);
        }

        // Execute request
        let response = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                FuseError::Timeout(format!("Request to {} timed out", url))
            } else if e.is_connect() {
                FuseError::NetworkError(format!("Failed to connect to {}: {}", url, e))
            } else {
                FuseError::NetworkError(format!("Request failed: {}", e))
            }
        })?;

        // Convert response
        self.convert_response(response).await
    }

    /// Add authentication to request
    fn add_auth(&self, req_builder: RequestBuilder, auth: &Auth) -> Result<RequestBuilder> {
        match auth.to_header_value() {
            Some(header_value) => Ok(req_builder.header("Authorization", header_value)),
            None => Ok(req_builder),
        }
    }

    /// Convert reqwest Response to ProxyResponse
    async fn convert_response(&self, response: Response) -> Result<ProxyResponse> {
        let status = response.status().as_u16();

        // Extract headers
        let mut headers = std::collections::HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(key.to_string(), value_str.to_string());
            }
        }

        // Check for error status codes
        if !response.status().is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(match status {
                401 => FuseError::AuthError(format!("Authentication failed: {}", error_body)),
                403 => FuseError::PermissionDenied(format!("Access denied: {}", error_body)),
                404 => {
                    FuseError::ResourceUnavailable(format!("Resource not found: {}", error_body))
                }
                429 => FuseError::RateLimitExceeded(format!("Rate limit exceeded: {}", error_body)),
                500..=599 => {
                    FuseError::ResourceUnavailable(format!("Server error: {}", error_body))
                }
                _ => FuseError::NetworkError(format!("HTTP error {}: {}", status, error_body)),
            });
        }

        // Parse response body
        let body = response.json::<serde_json::Value>().await.map_err(|e| {
            FuseError::SerializationError(format!("Failed to parse response: {}", e))
        })?;

        Ok(ProxyResponse {
            status,
            headers,
            body,
        })
    }

    /// Test connectivity to a remote endpoint
    pub async fn test_connection(&self, endpoint: &RemoteEndpoint) -> Result<()> {
        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        self.forward_request(endpoint, request).await?;
        Ok(())
    }
}

impl Default for RemoteModelProxy {
    fn default() -> Self {
        Self::new().expect("Failed to create default RemoteModelProxy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
        assert_eq!(config.max_backoff, Duration::from_secs(10));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_proxy_request_creation() {
        let request = ProxyRequest {
            method: "POST".to_string(),
            path: "/api/v1/infer".to_string(),
            headers: std::collections::HashMap::new(),
            body: Some(serde_json::json!({"prompt": "Hello"})),
        };

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/v1/infer");
        assert!(request.body.is_some());
    }

    #[test]
    fn test_proxy_request_serialization() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let request = ProxyRequest {
            method: "POST".to_string(),
            path: "/api/v1/infer".to_string(),
            headers,
            body: Some(serde_json::json!({"prompt": "Hello"})),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: ProxyRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.method, request.method);
        assert_eq!(deserialized.path, request.path);
        assert_eq!(deserialized.headers.len(), request.headers.len());
    }

    #[test]
    fn test_proxy_response_creation() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = ProxyResponse {
            status: 200,
            headers,
            body: serde_json::json!({"result": "success"}),
        };

        assert_eq!(response.status, 200);
        assert_eq!(response.headers.len(), 1);
        assert!(response.body.is_object());
    }

    #[test]
    fn test_remote_model_proxy_creation() {
        let proxy = RemoteModelProxy::new();
        assert!(proxy.is_ok());
    }

    #[test]
    fn test_remote_model_proxy_with_retry_config() {
        let retry_config = RetryConfig {
            max_retries: 5,
            initial_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(20),
            backoff_multiplier: 3.0,
        };

        let proxy = RemoteModelProxy::new()
            .unwrap()
            .with_retry_config(retry_config.clone());
        assert_eq!(proxy.retry_config.max_retries, 5);
        assert_eq!(
            proxy.retry_config.initial_backoff,
            Duration::from_millis(200)
        );
        assert_eq!(proxy.retry_config.max_backoff, Duration::from_secs(20));
        assert_eq!(proxy.retry_config.backoff_multiplier, 3.0);
    }

    #[tokio::test]
    async fn test_forward_request_disabled_endpoint() {
        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", "https://example.com").with_enabled(false);

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disabled"));
    }

    #[test]
    fn test_proxy_default() {
        let proxy = RemoteModelProxy::default();
        assert_eq!(proxy.retry_config.max_retries, 3);
    }

    // Integration tests with mock HTTP server

    #[tokio::test]
    async fn test_forward_request_get_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ok",
                "message": "success"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/test".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["status"], "ok");
        assert_eq!(response.body["message"], "success");
    }

    #[tokio::test]
    async fn test_forward_request_post_with_body() {
        let mock_server = MockServer::start().await;

        let expected_body = serde_json::json!({
            "prompt": "Hello, world!",
            "max_tokens": 100
        });

        Mock::given(method("POST"))
            .and(path("/api/infer"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": "Generated text",
                "tokens": 50
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "POST".to_string(),
            path: "/api/infer".to_string(),
            headers: std::collections::HashMap::new(),
            body: Some(expected_body),
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["result"], "Generated text");
        assert_eq!(response.body["tokens"], 50);
    }

    #[tokio::test]
    async fn test_forward_request_with_api_key_auth() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/protected"))
            .and(header("Authorization", "Bearer test-api-key-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authenticated": true
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri())
            .with_auth(Auth::ApiKey("test-api-key-123".to_string()));

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/protected".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["authenticated"], true);
    }

    #[tokio::test]
    async fn test_forward_request_with_bearer_token_auth() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/protected"))
            .and(header("Authorization", "Bearer my-bearer-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authenticated": true
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri())
            .with_auth(Auth::BearerToken("my-bearer-token".to_string()));

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/protected".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["authenticated"], true);
    }

    #[tokio::test]
    async fn test_forward_request_with_basic_auth() {
        let mock_server = MockServer::start().await;

        // Basic auth for "user:pass" is "dXNlcjpwYXNz"
        Mock::given(method("GET"))
            .and(path("/api/protected"))
            .and(header("Authorization", "Basic dXNlcjpwYXNz"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authenticated": true
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri()).with_auth(Auth::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        });

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/protected".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["authenticated"], true);
    }

    #[tokio::test]
    async fn test_forward_request_with_custom_headers() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/custom"))
            .and(header("X-Custom-Header", "custom-value"))
            .and(header("X-Request-ID", "req-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "received": true
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
        headers.insert("X-Request-ID".to_string(), "req-123".to_string());

        let request = ProxyRequest {
            method: "POST".to_string(),
            path: "/api/custom".to_string(),
            headers,
            body: Some(serde_json::json!({"data": "test"})),
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["received"], true);
    }

    #[tokio::test]
    async fn test_forward_request_401_unauthorized() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/protected"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/protected".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, FuseError::AuthError(_)));
        assert!(error.to_string().contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_forward_request_403_forbidden() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/forbidden"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": "Forbidden"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/forbidden".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, FuseError::PermissionDenied(_)));
        assert!(error.to_string().contains("Access denied"));
    }

    #[tokio::test]
    async fn test_forward_request_404_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/notfound"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": "Not found"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/notfound".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, FuseError::ResourceUnavailable(_)));
        assert!(error.to_string().contains("Resource not found"));
    }

    #[tokio::test]
    async fn test_forward_request_429_rate_limit() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/ratelimit"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "error": "Too many requests"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/ratelimit".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, FuseError::RateLimitExceeded(_)));
        assert!(error.to_string().contains("Rate limit exceeded"));
    }

    #[tokio::test]
    async fn test_forward_request_500_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/error"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "error": "Internal server error"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/error".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, FuseError::ResourceUnavailable(_)));
        assert!(error.to_string().contains("Server error"));
    }

    #[tokio::test]
    async fn test_forward_request_retry_on_failure() {
        let mock_server = MockServer::start().await;

        // First two requests fail, third succeeds
        Mock::given(method("GET"))
            .and(path("/api/retry"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/retry"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ok"
            })))
            .mount(&mock_server)
            .await;

        let retry_config = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        };

        let proxy = RemoteModelProxy::new()
            .unwrap()
            .with_retry_config(retry_config);
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/retry".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["status"], "ok");
    }

    #[tokio::test]
    async fn test_forward_request_retry_exhausted() {
        let mock_server = MockServer::start().await;

        // All requests fail
        Mock::given(method("GET"))
            .and(path("/api/fail"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        };

        let proxy = RemoteModelProxy::new()
            .unwrap()
            .with_retry_config(retry_config);
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let request = ProxyRequest {
            method: "GET".to_string(),
            path: "/api/fail".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let result = proxy.forward_request(&endpoint, request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_forward_request_different_http_methods() {
        let mock_server = MockServer::start().await;

        // Test PUT
        Mock::given(method("PUT"))
            .and(path("/api/update"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "updated": true
            })))
            .mount(&mock_server)
            .await;

        // Test DELETE
        Mock::given(method("DELETE"))
            .and(path("/api/delete"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "deleted": true
            })))
            .mount(&mock_server)
            .await;

        // Test PATCH
        Mock::given(method("PATCH"))
            .and(path("/api/patch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "patched": true
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        // Test PUT
        let request = ProxyRequest {
            method: "PUT".to_string(),
            path: "/api/update".to_string(),
            headers: std::collections::HashMap::new(),
            body: Some(serde_json::json!({"data": "test"})),
        };
        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.body["updated"], true);

        // Test DELETE
        let request = ProxyRequest {
            method: "DELETE".to_string(),
            path: "/api/delete".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };
        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.body["deleted"], true);

        // Test PATCH
        let request = ProxyRequest {
            method: "PATCH".to_string(),
            path: "/api/patch".to_string(),
            headers: std::collections::HashMap::new(),
            body: Some(serde_json::json!({"field": "value"})),
        };
        let response = proxy.forward_request(&endpoint, request).await.unwrap();
        assert_eq!(response.body["patched"], true);
    }

    #[tokio::test]
    async fn test_test_connection_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ok"
            })))
            .mount(&mock_server)
            .await;

        let proxy = RemoteModelProxy::new().unwrap();
        let endpoint = RemoteEndpoint::new("test", &mock_server.uri());

        let result = proxy.test_connection(&endpoint).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_test_connection_failure() {
        let proxy = RemoteModelProxy::new().unwrap();
        // Use an invalid URL that will fail to connect
        let endpoint = RemoteEndpoint::new("test", "http://localhost:1");

        let result = proxy.test_connection(&endpoint).await;
        assert!(result.is_err());
    }
}
