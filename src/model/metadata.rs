use crate::model::source::ModelSource;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enhanced model metadata with all required fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMetadata {
    /// Unique identifier for the model
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model source information
    pub source: ModelSource,
    /// Model version
    pub version: String,
    /// Timestamp when model was downloaded
    pub downloaded_at: DateTime<Utc>,
    /// Timestamp when model was last updated
    pub updated_at: Option<DateTime<Utc>>,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Model architecture (e.g., "transformer", "gpt", "llama")
    pub architecture: Option<String>,
    /// Number of parameters
    pub parameter_count: Option<usize>,
    /// Quantization method if quantized
    pub quantization: Option<String>,
    /// Format downloaded (e.g., gguf, pytorch)
    #[serde(default)]
    pub format: Option<String>,
    /// Model tags for categorization
    pub tags: Vec<String>,
    /// Custom metadata fields
    pub custom_metadata: HashMap<String, serde_json::Value>,
    /// Model file paths (relative to models directory)
    pub file_paths: Vec<String>,
    /// Model configuration file path
    pub config_path: Option<String>,
    /// Tokenizer file path
    pub tokenizer_path: Option<String>,
}

impl ModelMetadata {
    /// Create a new model metadata instance
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        source: ModelSource,
        version: impl Into<String>,
        size_bytes: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source,
            version: version.into(),
            downloaded_at: Utc::now(),
            updated_at: None,
            size_bytes,
            architecture: None,
            parameter_count: None,
            quantization: None,
            format: None,
            tags: Vec::new(),
            custom_metadata: HashMap::new(),
            file_paths: Vec::new(),
            config_path: None,
            tokenizer_path: None,
        }
    }

    /// Set the architecture
    pub fn with_architecture(mut self, architecture: impl Into<String>) -> Self {
        self.architecture = Some(architecture.into());
        self
    }

    /// Set the parameter count
    pub fn with_parameter_count(mut self, count: usize) -> Self {
        self.parameter_count = Some(count);
        self
    }

    /// Set the quantization method
    pub fn with_quantization(mut self, quantization: impl Into<String>) -> Self {
        self.quantization = Some(quantization.into());
        self
    }

    /// Set the format
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Add custom metadata
    pub fn with_custom_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.custom_metadata.insert(key.into(), value);
        self
    }

    /// Add a file path
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_paths.push(path.into());
        self
    }

    /// Set the config path
    pub fn with_config_path(mut self, path: impl Into<String>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set the tokenizer path
    pub fn with_tokenizer_path(mut self, path: impl Into<String>) -> Self {
        self.tokenizer_path = Some(path.into());
        self
    }

    /// Mark the model as updated
    pub fn mark_updated(&mut self) {
        self.updated_at = Some(Utc::now());
    }

    /// Get human-readable size
    pub fn size_human_readable(&self) -> String {
        let size = self.size_bytes as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else if size < 1024.0 * 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        } else {
            format!("{:.2} TB", size / (1024.0 * 1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Get parameter count in human-readable format
    pub fn parameter_count_human_readable(&self) -> Option<String> {
        self.parameter_count.map(|count| {
            let count = count as f64;
            if count < 1_000.0 {
                format!("{}", count)
            } else if count < 1_000_000.0 {
                format!("{:.1}K", count / 1_000.0)
            } else if count < 1_000_000_000.0 {
                format!("{:.1}M", count / 1_000_000.0)
            } else {
                format!("{:.1}B", count / 1_000_000_000.0)
            }
        })
    }

    /// Check if model has been updated since download
    pub fn has_been_updated(&self) -> bool {
        self.updated_at.is_some()
    }

    /// Get age of the model (time since download)
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.downloaded_at
    }

    /// Get time since last update
    pub fn time_since_update(&self) -> Option<chrono::Duration> {
        self.updated_at.map(|updated| Utc::now() - updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::source::ModelSource;

    fn create_test_metadata() -> ModelMetadata {
        ModelMetadata::new(
            "test-model",
            "Test Model",
            ModelSource::huggingface("test/model"),
            "1.0.0",
            1024 * 1024 * 1024, // 1 GB
        )
    }

    #[test]
    fn test_metadata_creation() {
        let metadata = create_test_metadata();

        assert_eq!(metadata.id, "test-model");
        assert_eq!(metadata.name, "Test Model");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.size_bytes, 1024 * 1024 * 1024);
        assert!(metadata.architecture.is_none());
        assert!(metadata.parameter_count.is_none());
        assert!(metadata.quantization.is_none());
        assert!(metadata.format.is_none());
        assert!(metadata.tags.is_empty());
        assert!(metadata.custom_metadata.is_empty());
    }

    #[test]
    fn test_metadata_with_architecture() {
        let metadata = create_test_metadata().with_architecture("transformer");
        assert_eq!(metadata.architecture, Some("transformer".to_string()));
    }

    #[test]
    fn test_metadata_with_parameter_count() {
        let metadata = create_test_metadata().with_parameter_count(7_000_000_000);
        assert_eq!(metadata.parameter_count, Some(7_000_000_000));
    }

    #[test]
    fn test_metadata_with_quantization() {
        let metadata = create_test_metadata().with_quantization("Q4_0");
        assert_eq!(metadata.quantization, Some("Q4_0".to_string()));
    }

    #[test]
    fn test_metadata_with_tags() {
        let metadata = create_test_metadata()
            .with_tag("nlp")
            .with_tag("transformer");
        assert_eq!(metadata.tags, vec!["nlp", "transformer"]);

        let metadata =
            create_test_metadata().with_tags(vec!["nlp".to_string(), "transformer".to_string()]);
        assert_eq!(metadata.tags, vec!["nlp", "transformer"]);
    }

    #[test]
    fn test_metadata_with_custom_metadata() {
        let metadata = create_test_metadata()
            .with_custom_metadata("key1", serde_json::json!("value1"))
            .with_custom_metadata("key2", serde_json::json!(42));

        assert_eq!(metadata.custom_metadata.len(), 2);
        assert_eq!(
            metadata.custom_metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(
            metadata.custom_metadata.get("key2"),
            Some(&serde_json::json!(42))
        );
    }

    #[test]
    fn test_metadata_with_file_paths() {
        let metadata = create_test_metadata()
            .with_file_path("model.bin")
            .with_file_path("config.json");

        assert_eq!(metadata.file_paths, vec!["model.bin", "config.json"]);
    }

    #[test]
    fn test_metadata_with_config_and_tokenizer() {
        let metadata = create_test_metadata()
            .with_config_path("config.json")
            .with_tokenizer_path("tokenizer.json");

        assert_eq!(metadata.config_path, Some("config.json".to_string()));
        assert_eq!(metadata.tokenizer_path, Some("tokenizer.json".to_string()));
    }

    #[test]
    fn test_metadata_mark_updated() {
        let mut metadata = create_test_metadata();
        assert!(metadata.updated_at.is_none());
        assert!(!metadata.has_been_updated());

        metadata.mark_updated();
        assert!(metadata.updated_at.is_some());
        assert!(metadata.has_been_updated());
    }

    #[test]
    fn test_size_human_readable() {
        let metadata = ModelMetadata::new("test", "Test", ModelSource::local("test"), "1.0", 512);
        assert_eq!(metadata.size_human_readable(), "512 B");

        let metadata = ModelMetadata::new("test", "Test", ModelSource::local("test"), "1.0", 1024);
        assert_eq!(metadata.size_human_readable(), "1.00 KB");

        let metadata = ModelMetadata::new(
            "test",
            "Test",
            ModelSource::local("test"),
            "1.0",
            1024 * 1024,
        );
        assert_eq!(metadata.size_human_readable(), "1.00 MB");

        let metadata = ModelMetadata::new(
            "test",
            "Test",
            ModelSource::local("test"),
            "1.0",
            1024 * 1024 * 1024,
        );
        assert_eq!(metadata.size_human_readable(), "1.00 GB");

        let metadata = ModelMetadata::new(
            "test",
            "Test",
            ModelSource::local("test"),
            "1.0",
            1024u64 * 1024 * 1024 * 1024,
        );
        assert_eq!(metadata.size_human_readable(), "1.00 TB");
    }

    #[test]
    fn test_parameter_count_human_readable() {
        let metadata = create_test_metadata().with_parameter_count(500);
        assert_eq!(
            metadata.parameter_count_human_readable(),
            Some("500".to_string())
        );

        let metadata = create_test_metadata().with_parameter_count(7_000);
        assert_eq!(
            metadata.parameter_count_human_readable(),
            Some("7.0K".to_string())
        );

        let metadata = create_test_metadata().with_parameter_count(7_000_000);
        assert_eq!(
            metadata.parameter_count_human_readable(),
            Some("7.0M".to_string())
        );

        let metadata = create_test_metadata().with_parameter_count(7_000_000_000);
        assert_eq!(
            metadata.parameter_count_human_readable(),
            Some("7.0B".to_string())
        );

        let metadata = create_test_metadata();
        assert_eq!(metadata.parameter_count_human_readable(), None);
    }

    #[test]
    fn test_age() {
        let metadata = create_test_metadata();
        let age = metadata.age();
        assert!(age.num_seconds() >= 0);
        assert!(age.num_seconds() < 1); // Should be very recent
    }

    #[test]
    fn test_time_since_update() {
        let mut metadata = create_test_metadata();
        assert_eq!(metadata.time_since_update(), None);

        metadata.mark_updated();
        let time_since = metadata.time_since_update();
        assert!(time_since.is_some());
        assert!(time_since.unwrap().num_seconds() >= 0);
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = create_test_metadata()
            .with_architecture("transformer")
            .with_parameter_count(7_000_000_000)
            .with_tag("nlp");

        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: ModelMetadata = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, metadata.id);
        assert_eq!(deserialized.name, metadata.name);
        assert_eq!(deserialized.version, metadata.version);
        assert_eq!(deserialized.size_bytes, metadata.size_bytes);
        assert_eq!(deserialized.architecture, metadata.architecture);
        assert_eq!(deserialized.parameter_count, metadata.parameter_count);
        assert_eq!(deserialized.format, metadata.format);
        assert_eq!(deserialized.tags, metadata.tags);
    }
}
