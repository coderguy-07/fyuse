use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handle to a loaded model in memory
#[derive(Clone, Debug)]
pub struct ModelHandle {
    /// Unique identifier for this model instance
    pub id: String,
    /// Name of the model
    pub model_name: String,
    /// Internal model state (implementation-specific)
    inner: Arc<RwLock<ModelState>>,
    /// Timestamp when the model was loaded
    pub loaded_at: DateTime<Utc>,
}

impl ModelHandle {
    /// Create a new model handle
    pub fn new(id: String, model_name: String, state: ModelState) -> Self {
        Self {
            id,
            model_name,
            inner: Arc::new(RwLock::new(state)),
            loaded_at: Utc::now(),
        }
    }

    /// Get read access to the model state
    pub async fn state(&self) -> tokio::sync::RwLockReadGuard<'_, ModelState> {
        self.inner.read().await
    }

    /// Get write access to the model state
    pub async fn state_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, ModelState> {
        self.inner.write().await
    }
}

/// Internal model state (placeholder for actual model implementation)
#[derive(Debug)]
pub struct ModelState {
    /// Path to the model files
    pub model_path: std::path::PathBuf,
    /// Model configuration
    pub config: ModelConfig,
    /// Whether the model is currently processing a request
    pub is_busy: bool,
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Maximum context window size
    pub max_context_length: usize,
    /// Model architecture type
    pub architecture: String,
    /// Additional model-specific parameters
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Input for inference operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceInput {
    /// The text prompt
    pub prompt: String,
    /// Optional images for vision models
    #[serde(default)]
    pub images: Vec<Image>,
    /// Optional conversation context
    #[serde(default)]
    pub context: Option<Vec<Message>>,
    /// Inference parameters
    pub parameters: InferenceParameters,
}

/// Image input for vision models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Image data as base64-encoded string
    pub data: String,
    /// Image format (png, jpg, gif, webp)
    pub format: ImageFormat,
    /// Optional image metadata
    #[serde(default)]
    pub metadata: Option<ImageMetadata>,
}

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpg,
    Gif,
    WebP,
}

impl ImageFormat {
    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
        }
    }

    /// Parse format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "jpg" | "jpeg" => Some(ImageFormat::Jpg),
            "gif" => Some(ImageFormat::Gif),
            "webp" => Some(ImageFormat::WebP),
            _ => None,
        }
    }

    /// Parse format from MIME type
    pub fn from_mime_type(mime: &str) -> Option<Self> {
        match mime.to_lowercase().as_str() {
            "image/png" => Some(ImageFormat::Png),
            "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpg),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            _ => None,
        }
    }
}

impl Image {
    /// Create a new image from base64-encoded data
    pub fn new(data: String, format: ImageFormat) -> Self {
        Self {
            data,
            format,
            metadata: None,
        }
    }

    /// Create an image with metadata
    pub fn with_metadata(mut self, metadata: ImageMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Validate the image data
    pub fn validate(&self) -> Result<()> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        // Check if data is valid base64
        if STANDARD.decode(&self.data).is_err() {
            return Err(FuseError::ValidationError(
                "Image data is not valid base64".to_string(),
            ));
        }

        // Check data size (limit to 10MB encoded)
        if self.data.len() > 10 * 1024 * 1024 {
            return Err(FuseError::ValidationError(
                "Image data exceeds maximum size of 10MB".to_string(),
            ));
        }

        Ok(())
    }

    /// Decode the base64 image data
    pub fn decode(&self) -> Result<Vec<u8>> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        STANDARD
            .decode(&self.data)
            .map_err(|e| FuseError::ValidationError(format!("Failed to decode image data: {}", e)))
    }

    /// Get the size of the decoded image data in bytes
    pub fn size_bytes(&self) -> Result<usize> {
        Ok(self.decode()?.len())
    }

    /// Create an image from a file path
    pub async fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();

        // Read file
        let data = tokio::fs::read(path).await?;

        // Determine format from extension
        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(ImageFormat::from_extension)
            .ok_or_else(|| {
                FuseError::ValidationError(format!(
                    "Unsupported image format for file: {}",
                    path.display()
                ))
            })?;

        // Encode as base64
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        let encoded = STANDARD.encode(&data);

        let metadata = ImageMetadata {
            width: None,
            height: None,
            size_bytes: Some(data.len() as u64),
        };

        Ok(Self {
            data: encoded,
            format,
            metadata: Some(metadata),
        })
    }

    /// Preprocess image for vision models
    /// This is a placeholder - real implementation would resize, normalize, etc.
    pub fn preprocess(&self) -> Result<Vec<u8>> {
        // In a real implementation, this would:
        // 1. Decode the image
        // 2. Resize to model's expected dimensions
        // 3. Normalize pixel values
        // 4. Convert to model's expected format (RGB, BGR, etc.)
        // 5. Return preprocessed data

        self.decode()
    }
}

/// Image metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: Option<u32>,
    /// Image height in pixels
    pub height: Option<u32>,
    /// File size in bytes
    pub size_bytes: Option<u64>,
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: Option<MessageMetadata>,
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Message metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Token count for this message
    pub token_count: Option<usize>,
    /// Model that generated this message (for assistant messages)
    pub model: Option<String>,
    /// Additional metadata
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceParameters {
    /// Maximum number of tokens to generate
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Temperature for sampling (0.0 to 2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Top-p sampling parameter
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Top-k sampling parameter
    #[serde(default)]
    pub top_k: Option<usize>,
    /// Stop sequences
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    /// Frequency penalty
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    /// Presence penalty
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    /// Random seed for reproducibility
    #[serde(default)]
    pub seed: Option<u64>,
}

fn default_max_tokens() -> usize {
    2048
}

fn default_temperature() -> f32 {
    0.7
}

fn default_top_p() -> f32 {
    0.9
}

impl Default for InferenceParameters {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            top_p: default_top_p(),
            top_k: None,
            stop_sequences: Vec::new(),
            frequency_penalty: None,
            presence_penalty: None,
            seed: None,
        }
    }
}

impl InferenceParameters {
    /// Validate parameters
    pub fn validate(&self) -> Result<()> {
        if self.max_tokens == 0 {
            return Err(FuseError::ValidationError(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(FuseError::ValidationError(
                "temperature must be between 0.0 and 2.0".to_string(),
            ));
        }

        if !(0.0..=1.0).contains(&self.top_p) {
            return Err(FuseError::ValidationError(
                "top_p must be between 0.0 and 1.0".to_string(),
            ));
        }

        if let Some(top_k) = self.top_k {
            if top_k == 0 {
                return Err(FuseError::ValidationError(
                    "top_k must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(freq_penalty) = self.frequency_penalty {
            if !(-2.0..=2.0).contains(&freq_penalty) {
                return Err(FuseError::ValidationError(
                    "frequency_penalty must be between -2.0 and 2.0".to_string(),
                ));
            }
        }

        if let Some(pres_penalty) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&pres_penalty) {
                return Err(FuseError::ValidationError(
                    "presence_penalty must be between -2.0 and 2.0".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Output from inference operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceOutput {
    /// Generated text
    pub text: String,
    /// Formatted output (markdown by default)
    pub formatted_text: String,
    /// Number of tokens in the prompt
    pub prompt_tokens: usize,
    /// Number of tokens generated
    pub completion_tokens: usize,
    /// Total tokens used
    pub total_tokens: usize,
    /// Model used for inference
    pub model: String,
    /// Timestamp when inference completed
    pub timestamp: DateTime<Utc>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: Option<InferenceMetadata>,
}

/// Metadata for inference output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceMetadata {
    /// Time taken for inference in milliseconds
    pub inference_time_ms: u64,
    /// Finish reason
    pub finish_reason: FinishReason,
    /// Additional metadata
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Reason why inference finished
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Reached natural stopping point
    Stop,
    /// Reached max token limit
    Length,
    /// Hit a stop sequence
    StopSequence,
    /// Inference was cancelled
    Cancelled,
    /// Error occurred
    Error,
}

/// Token emitted during streaming inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Token text
    pub text: String,
    /// Token ID (if available)
    pub id: Option<u32>,
    /// Log probability (if available)
    pub logprob: Option<f32>,
    /// Whether this is the final token
    pub is_final: bool,
}

/// Inference engine trait for running models
#[async_trait::async_trait]
pub trait InferenceEngine: Send + Sync {
    /// Load a model into memory
    async fn load_model(&self, model_name: &str) -> Result<ModelHandle>;

    /// Unload a model from memory
    async fn unload_model(&self, handle: ModelHandle) -> Result<()>;

    /// Run synchronous inference
    async fn infer(&self, handle: &ModelHandle, input: InferenceInput) -> Result<InferenceOutput>;

    /// Run streaming inference
    async fn infer_stream(
        &self,
        handle: &ModelHandle,
        input: InferenceInput,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<Token>>>;

    /// Check if a model is loaded
    async fn is_loaded(&self, model_name: &str) -> bool;

    /// Get information about a loaded model
    async fn get_model_info(&self, model_name: &str) -> Result<ModelInfo>;
}

/// Information about a loaded model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model name
    pub name: String,
    /// Model handle ID
    pub handle_id: String,
    /// When the model was loaded
    pub loaded_at: DateTime<Utc>,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Model configuration
    pub config: ModelConfig,
    /// Whether the model is currently busy
    pub is_busy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    fn encode_base64(data: &[u8]) -> String {
        STANDARD.encode(data)
    }

    #[test]
    fn test_image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }

    #[test]
    fn test_image_format_from_extension() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpg));
        assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpg));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::WebP));
        assert_eq!(ImageFormat::from_extension("bmp"), None);
    }

    #[test]
    fn test_image_format_from_mime_type() {
        assert_eq!(
            ImageFormat::from_mime_type("image/png"),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::from_mime_type("image/jpeg"),
            Some(ImageFormat::Jpg)
        );
        assert_eq!(
            ImageFormat::from_mime_type("image/gif"),
            Some(ImageFormat::Gif)
        );
        assert_eq!(
            ImageFormat::from_mime_type("image/webp"),
            Some(ImageFormat::WebP)
        );
        assert_eq!(ImageFormat::from_mime_type("image/bmp"), None);
    }

    #[test]
    fn test_image_new() {
        let data = encode_base64(b"test image data");
        let image = Image::new(data.clone(), ImageFormat::Png);

        assert_eq!(image.data, data);
        assert_eq!(image.format, ImageFormat::Png);
        assert!(image.metadata.is_none());
    }

    #[test]
    fn test_image_with_metadata() {
        let data = encode_base64(b"test image data");
        let metadata = ImageMetadata {
            width: Some(100),
            height: Some(100),
            size_bytes: Some(1024),
        };

        let image = Image::new(data, ImageFormat::Png).with_metadata(metadata.clone());

        assert!(image.metadata.is_some());
        let img_metadata = image.metadata.unwrap();
        assert_eq!(img_metadata.width, Some(100));
        assert_eq!(img_metadata.height, Some(100));
        assert_eq!(img_metadata.size_bytes, Some(1024));
    }

    #[test]
    fn test_image_validate_success() {
        let data = encode_base64(b"valid image data");
        let image = Image::new(data, ImageFormat::Png);

        assert!(image.validate().is_ok());
    }

    #[test]
    fn test_image_validate_invalid_base64() {
        let image = Image::new("not valid base64!!!".to_string(), ImageFormat::Png);

        let result = image.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[test]
    fn test_image_validate_too_large() {
        // Create data larger than 10MB
        let large_data = "A".repeat(11 * 1024 * 1024);
        let image = Image::new(large_data, ImageFormat::Png);

        let result = image.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[test]
    fn test_image_decode() {
        let original_data = b"test image data";
        let encoded = encode_base64(original_data);
        let image = Image::new(encoded, ImageFormat::Png);

        let decoded = image.decode().unwrap();
        assert_eq!(decoded, original_data);
    }

    #[test]
    fn test_image_size_bytes() {
        let original_data = b"test image data";
        let encoded = encode_base64(original_data);
        let image = Image::new(encoded, ImageFormat::Png);

        let size = image.size_bytes().unwrap();
        assert_eq!(size, original_data.len());
    }

    #[test]
    fn test_image_preprocess() {
        let original_data = b"test image data";
        let encoded = encode_base64(original_data);
        let image = Image::new(encoded, ImageFormat::Png);

        let preprocessed = image.preprocess().unwrap();
        assert_eq!(preprocessed, original_data);
    }

    #[tokio::test]
    async fn test_image_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary PNG file
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"fake png data";
        temp_file.write_all(test_data).unwrap();
        temp_file.flush().unwrap();

        // Rename to have .png extension
        let temp_path = temp_file.path().with_extension("png");
        std::fs::copy(temp_file.path(), &temp_path).unwrap();

        let result = Image::from_file(&temp_path).await;
        assert!(result.is_ok());

        let image = result.unwrap();
        assert_eq!(image.format, ImageFormat::Png);
        assert!(image.metadata.is_some());

        let decoded = image.decode().unwrap();
        assert_eq!(decoded, test_data);

        // Cleanup
        std::fs::remove_file(&temp_path).ok();
    }

    #[tokio::test]
    async fn test_image_from_file_unsupported_format() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().with_extension("bmp");
        std::fs::copy(temp_file.path(), &temp_path).unwrap();

        let result = Image::from_file(&temp_path).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));

        // Cleanup
        std::fs::remove_file(&temp_path).ok();
    }

    #[test]
    fn test_inference_parameters_default() {
        let params = InferenceParameters::default();

        assert_eq!(params.max_tokens, 2048);
        assert_eq!(params.temperature, 0.7);
        assert_eq!(params.top_p, 0.9);
        assert!(params.top_k.is_none());
        assert!(params.stop_sequences.is_empty());
        assert!(params.frequency_penalty.is_none());
        assert!(params.presence_penalty.is_none());
        assert!(params.seed.is_none());
    }

    #[test]
    fn test_inference_parameters_validate_success() {
        let params = InferenceParameters::default();
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_inference_parameters_validate_max_tokens() {
        let mut params = InferenceParameters::default();
        params.max_tokens = 0;

        let result = params.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FuseError::ValidationError(_)));
    }

    #[test]
    fn test_inference_parameters_validate_temperature() {
        let mut params = InferenceParameters::default();
        params.temperature = 3.0;

        let result = params.validate();
        assert!(result.is_err());

        params.temperature = -0.1;
        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_parameters_validate_top_p() {
        let mut params = InferenceParameters::default();
        params.top_p = 1.5;

        let result = params.validate();
        assert!(result.is_err());

        params.top_p = -0.1;
        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_parameters_validate_top_k() {
        let mut params = InferenceParameters::default();
        params.top_k = Some(0);

        let result = params.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_parameters_validate_penalties() {
        let mut params = InferenceParameters::default();
        params.frequency_penalty = Some(3.0);

        let result = params.validate();
        assert!(result.is_err());

        params.frequency_penalty = Some(1.0);
        params.presence_penalty = Some(-3.0);

        let result = params.validate();
        assert!(result.is_err());
    }
}
