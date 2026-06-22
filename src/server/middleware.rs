use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::FuseConfig;

/// Rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    requests_per_minute: u32,
}

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            requests_per_minute,
        }
    }

    /// Check if a request is allowed for the given identifier
    pub async fn check_rate_limit(&self, identifier: &str) -> bool {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(identifier.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.requests_per_minute as f64,
                last_refill: Instant::now(),
                capacity: self.requests_per_minute as f64,
            });

        // Refill tokens based on time elapsed
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * (self.requests_per_minute as f64 / 60.0);

        bucket.tokens = (bucket.tokens + tokens_to_add).min(bucket.capacity);
        bucket.last_refill = now;

        // Check if we have tokens available
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Clean up old buckets periodically
    pub async fn cleanup_old_buckets(&self) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();

        buckets
            .retain(|_, bucket| now.duration_since(bucket.last_refill) < Duration::from_secs(300));
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(config): State<Arc<FuseConfig>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Extract identifier (IP address or API key)
    let identifier = extract_identifier(&headers, &request);

    // Create rate limiter
    let rate_limiter = RateLimiter::new(config.server.rate_limit.requests_per_minute);

    // Check rate limit
    if !rate_limiter.check_rate_limit(&identifier).await {
        warn!(identifier = %identifier, "Rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded. Please try again later.",
        )
            .into_response();
    }

    debug!(identifier = %identifier, "Rate limit check passed");
    next.run(request).await
}

/// Extract identifier from request (IP or API key)
fn extract_identifier(headers: &HeaderMap, request: &Request) -> String {
    // Try to get API key from headers
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key) = api_key.to_str() {
            return format!("api:{}", key);
        }
    }

    // Try to get IP from X-Forwarded-For header
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(ip) = forwarded.to_str() {
            return format!("ip:{}", ip.split(',').next().unwrap_or(ip).trim());
        }
    }

    // Try to get IP from X-Real-IP header
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return format!("ip:{}", ip);
        }
    }

    // Fallback to connection info
    if let Some(connect_info) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return format!("ip:{}", connect_info.0.ip());
    }

    // Default identifier
    "unknown".to_string()
}

/// Authentication middleware
pub async fn auth_middleware(headers: HeaderMap, request: Request, next: Next) -> Response {
    // Check for API key in headers
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key) = api_key.to_str() {
            // Validate API key (in production, check against database)
            if validate_api_key(key) {
                debug!(api_key = %key, "Authentication successful");
                return next.run(request).await;
            }
        }
    }

    // Check for Bearer token
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if validate_bearer_token(token) {
                    debug!("Bearer token authentication successful");
                    return next.run(request).await;
                }
            }
        }
    }

    warn!("Authentication failed");
    (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

/// Validate API key against the configured allowlist.
///
/// Set `FUSE_API_KEYS` to a comma-separated list of valid keys to restrict access.
/// If the variable is unset, any non-empty key is accepted (open/dev mode).
fn validate_api_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    if let Ok(configured) = std::env::var("FUSE_API_KEYS") {
        return configured
            .split(',')
            .map(str::trim)
            .any(|k| k == key);
    }
    // No keys configured — open mode, accept any non-empty key
    true
}

/// Validate bearer token against the configured allowlist.
///
/// Set `FUSE_API_KEYS` to a comma-separated list of valid tokens to restrict access.
/// If the variable is unset, any non-empty token is accepted (open/dev mode).
fn validate_bearer_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    if let Ok(configured) = std::env::var("FUSE_API_KEYS") {
        return configured
            .split(',')
            .map(str::trim)
            .any(|k| k == token);
    }
    // No keys configured — open mode, accept any non-empty token
    true
}

/// Security headers middleware
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();

    // Add security headers
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Strict-Transport-Security",
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'".parse().unwrap(),
    );

    response
}

/// Input validation middleware
pub async fn input_validation_middleware(request: Request, next: Next) -> Response {
    // Validate request size
    if let Some(content_length) = request.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                // Limit request size to 10MB
                if length > 10 * 1024 * 1024 {
                    warn!(size = length, "Request too large");
                    return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large")
                        .into_response();
                }
            }
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new(60);

        // First request should be allowed
        assert!(limiter.check_rate_limit("test-user").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_excessive_requests() {
        let limiter = RateLimiter::new(2);

        // First two requests should be allowed
        assert!(limiter.check_rate_limit("test-user").await);
        assert!(limiter.check_rate_limit("test-user").await);

        // Third request should be blocked
        assert!(!limiter.check_rate_limit("test-user").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_tokens() {
        let limiter = RateLimiter::new(60); // 1 token per second

        // Use up a token
        assert!(limiter.check_rate_limit("test-user").await);

        // Wait for refill
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should have tokens again
        assert!(limiter.check_rate_limit("test-user").await);
    }

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("valid-key"));
        assert!(!validate_api_key(""));
    }

    #[test]
    fn test_validate_bearer_token() {
        assert!(validate_bearer_token("valid-token"));
        assert!(!validate_bearer_token(""));
    }
}
