/// Input validation and sanitization utilities
use crate::error::{FuseError, Result};
use std::path::Path;

/// Validate model name format
/// Model names should be alphanumeric with hyphens, underscores, and forward slashes
pub fn validate_model_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(FuseError::ValidationError(
            "Model name cannot be empty".to_string(),
        ));
    }

    if name.len() > 255 {
        return Err(FuseError::ValidationError(
            "Model name is too long (max 255 characters)".to_string(),
        ));
    }

    // Allow alphanumeric, hyphens, underscores, forward slashes, dots, and colons
    let valid_chars = name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.' || c == ':');

    if !valid_chars {
        return Err(FuseError::ValidationError(
            "Model name contains invalid characters. Only alphanumeric, hyphens, underscores, forward slashes, dots, and colons are allowed".to_string(),
        ));
    }

    Ok(())
}

/// Validate URL format
pub fn validate_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Err(FuseError::ValidationError(
            "URL cannot be empty".to_string(),
        ));
    }

    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(FuseError::ValidationError(
            "URL must start with http:// or https://".to_string(),
        ));
    }

    Ok(())
}

/// Validate port number
pub fn validate_port(port: u16) -> Result<()> {
    if port < 1024 {
        return Err(FuseError::ValidationError(
            "Port number must be >= 1024 (privileged ports are not allowed)".to_string(),
        ));
    }

    Ok(())
}

/// Validate file path exists
pub fn validate_file_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(FuseError::ValidationError(format!(
            "File does not exist: {}",
            path.display()
        )));
    }

    if !path.is_file() {
        return Err(FuseError::ValidationError(format!(
            "Path is not a file: {}",
            path.display()
        )));
    }

    Ok(())
}

/// Validate directory path exists
pub fn validate_dir_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(FuseError::ValidationError(format!(
            "Directory does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(FuseError::ValidationError(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    Ok(())
}

/// Sanitize string input by removing control characters
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Validate quantization method
pub fn validate_quantization_method(method: &str) -> Result<()> {
    let valid_methods = ["gguf", "gptq", "awq", "ggml"];

    if !valid_methods.contains(&method.to_lowercase().as_str()) {
        return Err(FuseError::ValidationError(format!(
            "Invalid quantization method: {}. Valid methods: {}",
            method,
            valid_methods.join(", ")
        )));
    }

    Ok(())
}

/// Validate quantization format
pub fn validate_quantization_format(format: &str) -> Result<()> {
    let valid_formats = ["q4_0", "q4_1", "q5_0", "q5_1", "q8_0", "q8_1"];

    if !valid_formats.contains(&format.to_lowercase().as_str()) {
        return Err(FuseError::ValidationError(format!(
            "Invalid quantization format: {}. Valid formats: {}",
            format,
            valid_formats.join(", ")
        )));
    }

    Ok(())
}

/// Validate merge strategy
pub fn validate_merge_strategy(strategy: &str) -> Result<()> {
    let valid_strategies = ["average", "weighted", "slerp"];

    if !valid_strategies.contains(&strategy.to_lowercase().as_str()) {
        return Err(FuseError::ValidationError(format!(
            "Invalid merge strategy: {}. Valid strategies: {}",
            strategy,
            valid_strategies.join(", ")
        )));
    }

    Ok(())
}

/// Validate scan output format
pub fn validate_scan_format(format: &str) -> Result<()> {
    let valid_formats = ["html", "json", "cyclonedx"];

    if !valid_formats.contains(&format.to_lowercase().as_str()) {
        return Err(FuseError::ValidationError(format!(
            "Invalid scan format: {}. Valid formats: {}",
            format,
            valid_formats.join(", ")
        )));
    }

    Ok(())
}

/// Validate layer type
pub fn validate_layer_type(layer_type: &str) -> Result<()> {
    let valid_types = ["geo-restriction", "content-filter", "custom"];

    if !valid_types.contains(&layer_type.to_lowercase().as_str()) {
        return Err(FuseError::ValidationError(format!(
            "Invalid layer type: {}. Valid types: {}",
            layer_type,
            valid_types.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_model_name() {
        assert!(validate_model_name("gpt-2").is_ok());
        assert!(validate_model_name("llama-3.1").is_ok());
        assert!(validate_model_name("org/model-name").is_ok());
        assert!(validate_model_name("model:latest").is_ok());
        assert!(validate_model_name("").is_err());
        assert!(validate_model_name("model with spaces").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://localhost:8080").is_ok());
        assert!(validate_url("").is_err());
        assert!(validate_url("not-a-url").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(3000).is_ok());
        assert!(validate_port(80).is_err());
        assert!(validate_port(1023).is_err());
    }

    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("hello\x00world"), "helloworld");
        assert_eq!(sanitize_string("hello\nworld"), "hello\nworld");
        assert_eq!(sanitize_string("hello\tworld"), "hello\tworld");
    }

    #[test]
    fn test_validate_quantization_method() {
        assert!(validate_quantization_method("gguf").is_ok());
        assert!(validate_quantization_method("GPTQ").is_ok());
        assert!(validate_quantization_method("invalid").is_err());
    }

    #[test]
    fn test_validate_merge_strategy() {
        assert!(validate_merge_strategy("average").is_ok());
        assert!(validate_merge_strategy("SLERP").is_ok());
        assert!(validate_merge_strategy("invalid").is_err());
    }
}
