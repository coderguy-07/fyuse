use serde::{Deserialize, Serialize};

/// Model provider types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    /// Hugging Face model hub
    HuggingFace,
    /// Unsloth model provider
    Unsloth,
    /// Remote endpoint (custom URL)
    Remote,
    /// Local file system
    Local,
}

impl Provider {
    /// Get the default base URL for this provider
    pub fn base_url(&self) -> Option<&'static str> {
        match self {
            Provider::HuggingFace => Some("https://huggingface.co"),
            Provider::Unsloth => Some("https://unsloth.ai"),
            Provider::Remote | Provider::Local => None,
        }
    }

    /// Check if this provider requires authentication
    pub fn requires_auth(&self) -> bool {
        matches!(
            self,
            Provider::HuggingFace | Provider::Unsloth | Provider::Remote
        )
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::HuggingFace => write!(f, "huggingface"),
            Provider::Unsloth => write!(f, "unsloth"),
            Provider::Remote => write!(f, "remote"),
            Provider::Local => write!(f, "local"),
        }
    }
}

impl std::str::FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "huggingface" | "hf" => Ok(Provider::HuggingFace),
            "unsloth" => Ok(Provider::Unsloth),
            "remote" => Ok(Provider::Remote),
            "local" => Ok(Provider::Local),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// Authentication credentials for model sources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Auth {
    /// API key authentication
    ApiKey(String),
    /// Bearer token authentication
    BearerToken(String),
    /// Basic authentication (username, password)
    Basic { username: String, password: String },
    /// No authentication
    None,
}

impl Auth {
    /// Check if authentication is provided
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, Auth::None)
    }

    /// Get authorization header value
    pub fn to_header_value(&self) -> Option<String> {
        match self {
            Auth::ApiKey(key) => Some(format!("Bearer {}", key)),
            Auth::BearerToken(token) => Some(format!("Bearer {}", token)),
            Auth::Basic { username, password } => {
                use base64::{engine::general_purpose, Engine as _};
                let credentials = format!("{}:{}", username, password);
                let encoded = general_purpose::STANDARD.encode(credentials);
                Some(format!("Basic {}", encoded))
            }
            Auth::None => None,
        }
    }
}

/// Model source information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSource {
    /// Provider type
    pub provider: Provider,
    /// Repository or model identifier
    pub repository: String,
    /// Model version or tag
    pub version: Option<String>,
    /// Custom URL for remote providers
    pub url: Option<String>,
}

impl ModelSource {
    /// Create a new model source
    pub fn new(provider: Provider, repository: impl Into<String>) -> Self {
        Self {
            provider,
            repository: repository.into(),
            version: None,
            url: None,
        }
    }

    /// Create a Hugging Face model source
    pub fn huggingface(repository: impl Into<String>) -> Self {
        Self::new(Provider::HuggingFace, repository)
    }

    /// Create an Unsloth model source
    pub fn unsloth(repository: impl Into<String>) -> Self {
        Self::new(Provider::Unsloth, repository)
    }

    /// Create a remote model source
    pub fn remote(url: impl Into<String>) -> Self {
        Self {
            provider: Provider::Remote,
            repository: String::new(),
            version: None,
            url: Some(url.into()),
        }
    }

    /// Create a local model source
    pub fn local(path: impl Into<String>) -> Self {
        Self {
            provider: Provider::Local,
            repository: path.into(),
            version: None,
            url: None,
        }
    }

    /// Set the version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the URL
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Get the full URL for downloading the model
    pub fn download_url(&self) -> Option<String> {
        match &self.provider {
            Provider::HuggingFace => {
                let version = self.version.as_deref().unwrap_or("main");
                Some(format!(
                    "{}/{}/resolve/{}",
                    self.provider.base_url()?,
                    self.repository,
                    version
                ))
            }
            Provider::Unsloth => Some(format!(
                "{}/models/{}",
                self.provider.base_url()?,
                self.repository
            )),
            Provider::Remote => self.url.clone(),
            Provider::Local => None,
        }
    }

    /// Get a unique identifier for this source
    pub fn identifier(&self) -> String {
        match &self.provider {
            Provider::HuggingFace | Provider::Unsloth => {
                if let Some(version) = &self.version {
                    format!("{}:{}", self.repository, version)
                } else {
                    self.repository.clone()
                }
            }
            Provider::Remote => self.url.clone().unwrap_or_else(|| "remote".to_string()),
            Provider::Local => self.repository.clone(),
        }
    }
}

impl std::fmt::Display for ModelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.provider {
            Provider::HuggingFace | Provider::Unsloth => {
                write!(f, "{}/{}", self.provider, self.repository)?;
                if let Some(version) = &self.version {
                    write!(f, ":{}", version)?;
                }
                Ok(())
            }
            Provider::Remote => {
                if let Some(url) = &self.url {
                    write!(f, "remote:{}", url)
                } else {
                    write!(f, "remote")
                }
            }
            Provider::Local => write!(f, "local:{}", self.repository),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_base_url() {
        assert_eq!(
            Provider::HuggingFace.base_url(),
            Some("https://huggingface.co")
        );
        assert_eq!(Provider::Unsloth.base_url(), Some("https://unsloth.ai"));
        assert_eq!(Provider::Remote.base_url(), None);
        assert_eq!(Provider::Local.base_url(), None);
    }

    #[test]
    fn test_provider_requires_auth() {
        assert!(Provider::HuggingFace.requires_auth());
        assert!(Provider::Unsloth.requires_auth());
        assert!(Provider::Remote.requires_auth());
        assert!(!Provider::Local.requires_auth());
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(Provider::HuggingFace.to_string(), "huggingface");
        assert_eq!(Provider::Unsloth.to_string(), "unsloth");
        assert_eq!(Provider::Remote.to_string(), "remote");
        assert_eq!(Provider::Local.to_string(), "local");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(
            "huggingface".parse::<Provider>().unwrap(),
            Provider::HuggingFace
        );
        assert_eq!("hf".parse::<Provider>().unwrap(), Provider::HuggingFace);
        assert_eq!("unsloth".parse::<Provider>().unwrap(), Provider::Unsloth);
        assert_eq!("remote".parse::<Provider>().unwrap(), Provider::Remote);
        assert_eq!("local".parse::<Provider>().unwrap(), Provider::Local);
        assert!("invalid".parse::<Provider>().is_err());
    }

    #[test]
    fn test_auth_is_authenticated() {
        assert!(Auth::ApiKey("key".to_string()).is_authenticated());
        assert!(Auth::BearerToken("token".to_string()).is_authenticated());
        assert!(Auth::Basic {
            username: "user".to_string(),
            password: "pass".to_string()
        }
        .is_authenticated());
        assert!(!Auth::None.is_authenticated());
    }

    #[test]
    fn test_auth_to_header_value() {
        assert_eq!(
            Auth::ApiKey("mykey".to_string()).to_header_value(),
            Some("Bearer mykey".to_string())
        );
        assert_eq!(
            Auth::BearerToken("mytoken".to_string()).to_header_value(),
            Some("Bearer mytoken".to_string())
        );
        assert!(Auth::Basic {
            username: "user".to_string(),
            password: "pass".to_string()
        }
        .to_header_value()
        .unwrap()
        .starts_with("Basic "));
        assert_eq!(Auth::None.to_header_value(), None);
    }

    #[test]
    fn test_model_source_creation() {
        let source = ModelSource::huggingface("gpt2");
        assert_eq!(source.provider, Provider::HuggingFace);
        assert_eq!(source.repository, "gpt2");
        assert_eq!(source.version, None);

        let source = ModelSource::unsloth("llama-3");
        assert_eq!(source.provider, Provider::Unsloth);
        assert_eq!(source.repository, "llama-3");

        let source = ModelSource::remote("https://example.com/model");
        assert_eq!(source.provider, Provider::Remote);
        assert_eq!(source.url, Some("https://example.com/model".to_string()));

        let source = ModelSource::local("/path/to/model");
        assert_eq!(source.provider, Provider::Local);
        assert_eq!(source.repository, "/path/to/model");
    }

    #[test]
    fn test_model_source_with_version() {
        let source = ModelSource::huggingface("gpt2").with_version("v1.0");
        assert_eq!(source.version, Some("v1.0".to_string()));
    }

    #[test]
    fn test_model_source_download_url() {
        let source = ModelSource::huggingface("gpt2");
        assert_eq!(
            source.download_url(),
            Some("https://huggingface.co/gpt2/resolve/main".to_string())
        );

        let source = ModelSource::huggingface("gpt2").with_version("v1.0");
        assert_eq!(
            source.download_url(),
            Some("https://huggingface.co/gpt2/resolve/v1.0".to_string())
        );

        let source = ModelSource::unsloth("llama-3");
        assert_eq!(
            source.download_url(),
            Some("https://unsloth.ai/models/llama-3".to_string())
        );

        let source = ModelSource::remote("https://example.com/model");
        assert_eq!(
            source.download_url(),
            Some("https://example.com/model".to_string())
        );

        let source = ModelSource::local("/path/to/model");
        assert_eq!(source.download_url(), None);
    }

    #[test]
    fn test_model_source_identifier() {
        let source = ModelSource::huggingface("gpt2");
        assert_eq!(source.identifier(), "gpt2");

        let source = ModelSource::huggingface("gpt2").with_version("v1.0");
        assert_eq!(source.identifier(), "gpt2:v1.0");

        let source = ModelSource::remote("https://example.com/model");
        assert_eq!(source.identifier(), "https://example.com/model");

        let source = ModelSource::local("/path/to/model");
        assert_eq!(source.identifier(), "/path/to/model");
    }

    #[test]
    fn test_model_source_display() {
        let source = ModelSource::huggingface("gpt2");
        assert_eq!(source.to_string(), "huggingface/gpt2");

        let source = ModelSource::huggingface("gpt2").with_version("v1.0");
        assert_eq!(source.to_string(), "huggingface/gpt2:v1.0");

        let source = ModelSource::unsloth("llama-3");
        assert_eq!(source.to_string(), "unsloth/llama-3");

        let source = ModelSource::remote("https://example.com/model");
        assert_eq!(source.to_string(), "remote:https://example.com/model");

        let source = ModelSource::local("/path/to/model");
        assert_eq!(source.to_string(), "local:/path/to/model");
    }

    #[test]
    fn test_model_source_serialization() {
        let source = ModelSource::huggingface("gpt2").with_version("v1.0");
        let serialized = serde_json::to_string(&source).unwrap();
        let deserialized: ModelSource = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.provider, source.provider);
        assert_eq!(deserialized.repository, source.repository);
        assert_eq!(deserialized.version, source.version);
    }
}
