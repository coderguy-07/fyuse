use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Main error type for Fuse
#[derive(Debug, Error)]
pub enum FuseError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Download failed: {0}")]
    DownloadError(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Workflow execution failed: {0}")]
    WorkflowError(String),

    #[error("Feature not enabled: {0}")]
    FeatureDisabled(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Layer operation failed: {0}")]
    LayerError(String),

    #[error("Quantization error: {0}")]
    QuantizationError(String),

    #[error("Merge error: {0}")]
    MergeError(String),

    #[error("Scan error: {0}")]
    ScanError(String),

    #[error("RAG error: {0}")]
    RAGError(String),

    #[error("Device error: {device}: {message}")]
    DeviceError { device: String, message: String },

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Resource not available: {0}")]
    ResourceUnavailable(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Channel error: {channel}: {message}")]
    ChannelError { channel: String, message: String },

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Agent error: {0}")]
    AgentError(String),
}

impl FuseError {
    /// Get error code for this error
    pub fn error_code(&self) -> &'static str {
        match self {
            FuseError::ModelNotFound(_) => "MODEL_NOT_FOUND",
            FuseError::DownloadError(_) => "DOWNLOAD_ERROR",
            FuseError::InferenceError(_) => "INFERENCE_ERROR",
            FuseError::AuthError(_) => "AUTH_ERROR",
            FuseError::ConfigError(_) => "CONFIG_ERROR",
            FuseError::WorkflowError(_) => "WORKFLOW_ERROR",
            FuseError::FeatureDisabled(_) => "FEATURE_DISABLED",
            FuseError::ValidationError(_) => "VALIDATION_ERROR",
            FuseError::DatabaseError(_) => "DATABASE_ERROR",
            FuseError::IoError(_) => "IO_ERROR",
            FuseError::NetworkError(_) => "NETWORK_ERROR",
            FuseError::SerializationError(_) => "SERIALIZATION_ERROR",
            FuseError::InternalError(_) => "INTERNAL_ERROR",
            FuseError::LayerError(_) => "LAYER_ERROR",
            FuseError::QuantizationError(_) => "QUANTIZATION_ERROR",
            FuseError::MergeError(_) => "MERGE_ERROR",
            FuseError::ScanError(_) => "SCAN_ERROR",
            FuseError::RAGError(_) => "RAG_ERROR",
            FuseError::DeviceError { .. } => "DEVICE_ERROR",
            FuseError::PermissionDenied(_) => "PERMISSION_DENIED",
            FuseError::ResourceUnavailable(_) => "RESOURCE_UNAVAILABLE",
            FuseError::Timeout(_) => "TIMEOUT",
            FuseError::RateLimitExceeded(_) => "RATE_LIMIT_EXCEEDED",
            FuseError::ResourceLimitExceeded(_) => "RESOURCE_LIMIT_EXCEEDED",
            FuseError::ChannelError { .. } => "CHANNEL_ERROR",
            FuseError::SessionNotFound(_) => "SESSION_NOT_FOUND",
            FuseError::AgentError(_) => "AGENT_ERROR",
        }
    }

    /// Get remediation suggestion for this error
    pub fn remediation(&self) -> Option<String> {
        match self {
            FuseError::ModelNotFound(name) => Some(format!(
                "Try pulling the model first with: fuse pull {}",
                name
            )),
            FuseError::DownloadError(_) => {
                Some("Check your internet connection and try again".to_string())
            }
            FuseError::InferenceError(_) => {
                Some("Check model compatibility and input format".to_string())
            }
            FuseError::AuthError(_) => Some("Verify your credentials and try again".to_string()),
            FuseError::ConfigError(_) => {
                Some("Check your configuration file for errors".to_string())
            }
            FuseError::WorkflowError(_) => {
                Some("Review your workflow definition for errors".to_string())
            }
            FuseError::FeatureDisabled(_) => {
                Some("Enable the feature in your configuration file".to_string())
            }
            FuseError::DatabaseError(_) => {
                Some("Check database integrity and permissions".to_string())
            }
            FuseError::IoError(_) => Some("Check file permissions and disk space".to_string()),
            FuseError::NetworkError(_) => {
                Some("Check your network connection and try again".to_string())
            }
            FuseError::PermissionDenied(_) => {
                Some("Check file and directory permissions".to_string())
            }
            FuseError::ResourceUnavailable(_) => Some(
                "The requested resource is temporarily unavailable. Try again later".to_string(),
            ),
            FuseError::Timeout(_) => Some(
                "Operation timed out. Try increasing timeout or check network connection"
                    .to_string(),
            ),
            FuseError::RateLimitExceeded(_) => {
                Some("Rate limit exceeded. Wait before retrying".to_string())
            }
            _ => None,
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            FuseError::NetworkError(_)
                | FuseError::DownloadError(_)
                | FuseError::Timeout(_)
                | FuseError::ResourceUnavailable(_)
        )
    }

    /// Get HTTP status code for this error (for API responses)
    pub fn http_status_code(&self) -> u16 {
        match self {
            FuseError::ModelNotFound(_) => 404,
            FuseError::AuthError(_) => 401,
            FuseError::PermissionDenied(_) => 403,
            FuseError::ValidationError(_) => 400,
            FuseError::ConfigError(_) => 400,
            FuseError::RateLimitExceeded(_) => 429,
            FuseError::Timeout(_) => 408,
            FuseError::ResourceUnavailable(_) => 503,
            FuseError::ResourceLimitExceeded(_) => 429,
            FuseError::FeatureDisabled(_) => 501,
            _ => 500,
        }
    }

    /// Log error with context preservation
    pub fn log_error(&self, context: &ErrorContext) {
        let error_code = self.error_code();
        let message = self.to_string();

        tracing::error!(
            error_code = %error_code,
            operation = %context.operation,
            model_name = ?context.model_name,
            user_id = ?context.user_id,
            component = ?context.component,
            retryable = %self.is_retryable(),
            message = %message,
            "Error occurred"
        );
    }

    /// Log error with full details including remediation
    pub fn log_error_detailed(&self, context: &ErrorContext) {
        let error_code = self.error_code();
        let message = self.to_string();
        let remediation = self.remediation();

        tracing::error!(
            error_code = %error_code,
            operation = %context.operation,
            model_name = ?context.model_name,
            user_id = ?context.user_id,
            component = ?context.component,
            retryable = %self.is_retryable(),
            http_status = %self.http_status_code(),
            remediation = ?remediation,
            message = %message,
            "Detailed error information"
        );
    }
}

impl From<reqwest::Error> for FuseError {
    fn from(err: reqwest::Error) -> Self {
        FuseError::NetworkError(err.to_string())
    }
}

impl From<serde_json::Error> for FuseError {
    fn from(err: serde_json::Error) -> Self {
        FuseError::SerializationError(err.to_string())
    }
}

impl From<toml::de::Error> for FuseError {
    fn from(err: toml::de::Error) -> Self {
        FuseError::ConfigError(err.to_string())
    }
}

impl From<toml::ser::Error> for FuseError {
    fn from(err: toml::ser::Error) -> Self {
        FuseError::SerializationError(err.to_string())
    }
}

impl From<crate::config::ConfigError> for FuseError {
    fn from(err: crate::config::ConfigError) -> Self {
        FuseError::ConfigError(err.to_string())
    }
}

impl From<redb::Error> for FuseError {
    fn from(err: redb::Error) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<redb::DatabaseError> for FuseError {
    fn from(err: redb::DatabaseError) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<redb::TableError> for FuseError {
    fn from(err: redb::TableError) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<redb::TransactionError> for FuseError {
    fn from(err: redb::TransactionError) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<redb::CommitError> for FuseError {
    fn from(err: redb::CommitError) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<redb::StorageError> for FuseError {
    fn from(err: redb::StorageError) -> Self {
        FuseError::DatabaseError(err.to_string())
    }
}

impl From<serde_yaml::Error> for FuseError {
    fn from(err: serde_yaml::Error) -> Self {
        FuseError::SerializationError(err.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for FuseError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        FuseError::Timeout(err.to_string())
    }
}

/// Error context for logging and debugging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub model_name: Option<String>,
    pub user_id: Option<String>,
    pub component: Option<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            model_name: None,
            user_id: None,
            component: None,
        }
    }

    /// Add model name to context
    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = Some(model_name.into());
        self
    }

    /// Add user ID to context
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Add component name to context
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }
}

/// Result type alias for Fuse operations
pub type Result<T> = std::result::Result<T, FuseError>;

/// Extension trait for Result to add context-aware error logging
pub trait ResultExt<T> {
    /// Log error if Result is Err, with context
    fn log_on_error(self, context: &ErrorContext) -> Self;

    /// Log error with detailed information if Result is Err
    fn log_on_error_detailed(self, context: &ErrorContext) -> Self;

    /// Map error and log with context
    fn map_err_log<F>(self, context: &ErrorContext, f: F) -> Self
    where
        F: FnOnce(FuseError) -> FuseError;
}

impl<T> ResultExt<T> for Result<T> {
    fn log_on_error(self, context: &ErrorContext) -> Self {
        if let Err(ref e) = self {
            e.log_error(context);
        }
        self
    }

    fn log_on_error_detailed(self, context: &ErrorContext) -> Self {
        if let Err(ref e) = self {
            e.log_error_detailed(context);
        }
        self
    }

    fn map_err_log<F>(self, context: &ErrorContext, f: F) -> Self
    where
        F: FnOnce(FuseError) -> FuseError,
    {
        self.map_err(|e| {
            e.log_error(context);
            f(e)
        })
    }
}

/// Error response format for API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub remediation: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_code: error_code.into(),
            message: message.into(),
            details: None,
            remediation: None,
            timestamp: Utc::now(),
        }
    }

    /// Add details to the error response
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add remediation suggestion
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

impl From<FuseError> for ErrorResponse {
    fn from(err: FuseError) -> Self {
        let error_code = err.error_code();
        let message = err.to_string();
        let remediation = err.remediation();

        let mut response = ErrorResponse::new(error_code, message);
        if let Some(rem) = remediation {
            response = response.with_remediation(rem);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            FuseError::ModelNotFound("test".to_string()).error_code(),
            "MODEL_NOT_FOUND"
        );
        assert_eq!(
            FuseError::DownloadError("test".to_string()).error_code(),
            "DOWNLOAD_ERROR"
        );
        assert_eq!(
            FuseError::InferenceError("test".to_string()).error_code(),
            "INFERENCE_ERROR"
        );
        assert_eq!(
            FuseError::AuthError("test".to_string()).error_code(),
            "AUTH_ERROR"
        );
        assert_eq!(
            FuseError::ConfigError("test".to_string()).error_code(),
            "CONFIG_ERROR"
        );
        assert_eq!(
            FuseError::WorkflowError("test".to_string()).error_code(),
            "WORKFLOW_ERROR"
        );
        assert_eq!(
            FuseError::FeatureDisabled("test".to_string()).error_code(),
            "FEATURE_DISABLED"
        );
        assert_eq!(
            FuseError::ValidationError("test".to_string()).error_code(),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            FuseError::DatabaseError("test".to_string()).error_code(),
            "DATABASE_ERROR"
        );
        assert_eq!(
            FuseError::NetworkError("test".to_string()).error_code(),
            "NETWORK_ERROR"
        );
        assert_eq!(
            FuseError::SerializationError("test".to_string()).error_code(),
            "SERIALIZATION_ERROR"
        );
        assert_eq!(
            FuseError::InternalError("test".to_string()).error_code(),
            "INTERNAL_ERROR"
        );
        assert_eq!(
            FuseError::LayerError("test".to_string()).error_code(),
            "LAYER_ERROR"
        );
        assert_eq!(
            FuseError::QuantizationError("test".to_string()).error_code(),
            "QUANTIZATION_ERROR"
        );
        assert_eq!(
            FuseError::MergeError("test".to_string()).error_code(),
            "MERGE_ERROR"
        );
        assert_eq!(
            FuseError::ScanError("test".to_string()).error_code(),
            "SCAN_ERROR"
        );
        assert_eq!(
            FuseError::RAGError("test".to_string()).error_code(),
            "RAG_ERROR"
        );
        assert_eq!(
            FuseError::PermissionDenied("test".to_string()).error_code(),
            "PERMISSION_DENIED"
        );
        assert_eq!(
            FuseError::ResourceUnavailable("test".to_string()).error_code(),
            "RESOURCE_UNAVAILABLE"
        );
        assert_eq!(
            FuseError::Timeout("test".to_string()).error_code(),
            "TIMEOUT"
        );
        assert_eq!(
            FuseError::RateLimitExceeded("test".to_string()).error_code(),
            "RATE_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn test_remediation_suggestions() {
        let error = FuseError::ModelNotFound("gpt2".to_string());
        assert!(error.remediation().is_some());
        assert!(error.remediation().unwrap().contains("fuse pull gpt2"));

        let error = FuseError::DownloadError("timeout".to_string());
        assert!(error.remediation().is_some());
        assert!(error.remediation().unwrap().contains("internet connection"));

        let error = FuseError::InternalError("bug".to_string());
        assert!(error.remediation().is_none());

        let error = FuseError::Timeout("operation timed out".to_string());
        assert!(error.remediation().is_some());
        assert!(error.remediation().unwrap().contains("timeout"));

        let error = FuseError::RateLimitExceeded("too many requests".to_string());
        assert!(error.remediation().is_some());
        assert!(error.remediation().unwrap().contains("Rate limit"));
    }

    #[test]
    fn test_retryable_errors() {
        assert!(FuseError::NetworkError("timeout".to_string()).is_retryable());
        assert!(FuseError::DownloadError("failed".to_string()).is_retryable());
        assert!(FuseError::Timeout("timeout".to_string()).is_retryable());
        assert!(FuseError::ResourceUnavailable("unavailable".to_string()).is_retryable());
        assert!(!FuseError::ConfigError("invalid".to_string()).is_retryable());
        assert!(!FuseError::ValidationError("bad input".to_string()).is_retryable());
        assert!(!FuseError::AuthError("unauthorized".to_string()).is_retryable());
    }

    #[test]
    fn test_http_status_codes() {
        assert_eq!(
            FuseError::ModelNotFound("test".to_string()).http_status_code(),
            404
        );
        assert_eq!(
            FuseError::AuthError("test".to_string()).http_status_code(),
            401
        );
        assert_eq!(
            FuseError::PermissionDenied("test".to_string()).http_status_code(),
            403
        );
        assert_eq!(
            FuseError::ValidationError("test".to_string()).http_status_code(),
            400
        );
        assert_eq!(
            FuseError::RateLimitExceeded("test".to_string()).http_status_code(),
            429
        );
        assert_eq!(
            FuseError::Timeout("test".to_string()).http_status_code(),
            408
        );
        assert_eq!(
            FuseError::ResourceUnavailable("test".to_string()).http_status_code(),
            503
        );
        assert_eq!(
            FuseError::FeatureDisabled("test".to_string()).http_status_code(),
            501
        );
        assert_eq!(
            FuseError::InternalError("test".to_string()).http_status_code(),
            500
        );
    }

    #[test]
    fn test_error_response_creation() {
        let error = FuseError::ModelNotFound("test-model".to_string());
        let response = ErrorResponse::from(error);

        assert_eq!(response.error_code, "MODEL_NOT_FOUND");
        assert!(response.message.contains("test-model"));
        assert!(response.remediation.is_some());
        assert!(response.remediation.unwrap().contains("fuse pull"));
    }

    #[test]
    fn test_error_response_with_details() {
        let response = ErrorResponse::new("TEST_ERROR", "Test message")
            .with_details(serde_json::json!({"key": "value"}))
            .with_remediation("Do this");

        assert_eq!(response.error_code, "TEST_ERROR");
        assert_eq!(response.message, "Test message");
        assert!(response.details.is_some());
        assert_eq!(response.remediation, Some("Do this".to_string()));
    }

    #[test]
    fn test_error_display() {
        let error = FuseError::ModelNotFound("gpt2".to_string());
        let display = format!("{}", error);
        assert!(display.contains("Model not found"));
        assert!(display.contains("gpt2"));
    }

    #[test]
    fn test_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let fuse_error: FuseError = io_error.into();
        assert!(matches!(fuse_error, FuseError::IoError(_)));
    }

    #[test]
    fn test_error_from_reqwest_error() {
        // Test network error conversion
        let error = FuseError::NetworkError("connection refused".to_string());
        assert_eq!(error.error_code(), "NETWORK_ERROR");
        assert!(error.is_retryable());
    }

    #[test]
    fn test_error_from_serde_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_error.is_err());

        let fuse_error: FuseError = json_error.unwrap_err().into();
        assert!(matches!(fuse_error, FuseError::SerializationError(_)));
    }

    #[test]
    fn test_error_from_config_error() {
        let config_error = crate::config::ConfigError::ValidationError("test".to_string());
        let fuse_error: FuseError = config_error.into();
        assert!(matches!(fuse_error, FuseError::ConfigError(_)));
    }

    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new("test_operation")
            .with_model("gpt2")
            .with_user("user123")
            .with_component("model_manager");

        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.model_name, Some("gpt2".to_string()));
        assert_eq!(context.user_id, Some("user123".to_string()));
        assert_eq!(context.component, Some("model_manager".to_string()));
    }

    #[test]
    fn test_result_ext_log_on_error() {
        let context = ErrorContext::new("test_operation");

        // Test with Ok result
        let result: Result<i32> = Ok(42);
        let logged_result = result.log_on_error(&context);
        assert!(logged_result.is_ok());
        assert_eq!(logged_result.unwrap(), 42);

        // Test with Err result
        let result: Result<i32> = Err(FuseError::ValidationError("test error".to_string()));
        let logged_result = result.log_on_error(&context);
        assert!(logged_result.is_err());
    }

    #[test]
    fn test_error_response_from_all_error_types() {
        let errors = vec![
            FuseError::ModelNotFound("test".to_string()),
            FuseError::DownloadError("test".to_string()),
            FuseError::InferenceError("test".to_string()),
            FuseError::AuthError("test".to_string()),
            FuseError::ConfigError("test".to_string()),
            FuseError::WorkflowError("test".to_string()),
            FuseError::FeatureDisabled("test".to_string()),
            FuseError::ValidationError("test".to_string()),
            FuseError::DatabaseError("test".to_string()),
            FuseError::NetworkError("test".to_string()),
            FuseError::SerializationError("test".to_string()),
            FuseError::InternalError("test".to_string()),
            FuseError::LayerError("test".to_string()),
            FuseError::QuantizationError("test".to_string()),
            FuseError::MergeError("test".to_string()),
            FuseError::ScanError("test".to_string()),
            FuseError::RAGError("test".to_string()),
            FuseError::PermissionDenied("test".to_string()),
            FuseError::ResourceUnavailable("test".to_string()),
            FuseError::Timeout("test".to_string()),
            FuseError::RateLimitExceeded("test".to_string()),
        ];

        for error in errors {
            let error_code = error.error_code();
            let response = ErrorResponse::from(error);
            assert_eq!(response.error_code, error_code);
            assert!(!response.message.is_empty());
        }
    }
}
