//! Tokenizer abstraction — wraps HuggingFace tokenizers for encoding/decoding.
//!
//! Supports BPE, SentencePiece, and Tiktoken tokenizers via the `tokenizers` crate.
//! Feature-gated behind `cpu-inference`.

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tokenizer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenizerType {
    Bpe,
    SentencePiece,
    Tiktoken,
    Unknown,
}

/// Wrapper around HuggingFace tokenizers.
pub struct FuseTokenizer {
    #[cfg(feature = "cpu-inference")]
    inner: tokenizers::Tokenizer,
    #[cfg(not(feature = "cpu-inference"))]
    _phantom: std::marker::PhantomData<()>,
    tokenizer_type: TokenizerType,
}

impl FuseTokenizer {
    /// Load a tokenizer from a JSON file (tokenizer.json format).
    #[cfg(feature = "cpu-inference")]
    pub fn from_file(path: &Path) -> Result<Self> {
        let inner = tokenizers::Tokenizer::from_file(path).map_err(|e| {
            FuseError::InternalError(format!(
                "Failed to load tokenizer from {}: {}",
                path.display(),
                e
            ))
        })?;

        let tokenizer_type = detect_tokenizer_type(&inner);

        Ok(Self {
            inner,
            tokenizer_type,
        })
    }

    /// Load a tokenizer from a pretrained model identifier (downloads from HF).
    #[cfg(feature = "cpu-inference")]
    pub fn from_pretrained(identifier: &str) -> Result<Self> {
        let inner = tokenizers::Tokenizer::from_pretrained(identifier, None).map_err(|e| {
            FuseError::InternalError(format!("Failed to load tokenizer '{}': {}", identifier, e))
        })?;

        let tokenizer_type = detect_tokenizer_type(&inner);

        Ok(Self {
            inner,
            tokenizer_type,
        })
    }

    /// Stub when cpu-inference is not enabled.
    #[cfg(not(feature = "cpu-inference"))]
    pub fn from_file(_path: &Path) -> Result<Self> {
        Err(FuseError::InternalError(
            "Tokenizer requires cpu-inference feature".to_string(),
        ))
    }

    /// Encode text to token IDs.
    #[cfg(feature = "cpu-inference")]
    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Vec<u32>> {
        let encoding = self
            .inner
            .encode(text, add_special_tokens)
            .map_err(|e| FuseError::InternalError(format!("Tokenization failed: {}", e)))?;
        Ok(encoding.get_ids().to_vec())
    }

    #[cfg(not(feature = "cpu-inference"))]
    pub fn encode(&self, _text: &str, _add_special_tokens: bool) -> Result<Vec<u32>> {
        Err(FuseError::InternalError(
            "Tokenizer requires cpu-inference feature".to_string(),
        ))
    }

    /// Decode token IDs back to text.
    #[cfg(feature = "cpu-inference")]
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String> {
        self.inner
            .decode(ids, skip_special_tokens)
            .map_err(|e| FuseError::InternalError(format!("Decoding failed: {}", e)))
    }

    #[cfg(not(feature = "cpu-inference"))]
    pub fn decode(&self, _ids: &[u32], _skip_special_tokens: bool) -> Result<String> {
        Err(FuseError::InternalError(
            "Tokenizer requires cpu-inference feature".to_string(),
        ))
    }

    /// Get vocabulary size.
    #[cfg(feature = "cpu-inference")]
    pub fn vocab_size(&self) -> usize {
        self.inner.get_vocab_size(true)
    }

    #[cfg(not(feature = "cpu-inference"))]
    pub fn vocab_size(&self) -> usize {
        0
    }

    /// Get the tokenizer type.
    pub fn tokenizer_type(&self) -> TokenizerType {
        self.tokenizer_type
    }

    /// Encode and return the token strings along with IDs.
    #[cfg(feature = "cpu-inference")]
    pub fn encode_with_tokens(&self, text: &str) -> Result<Vec<(u32, String)>> {
        let encoding = self
            .inner
            .encode(text, false)
            .map_err(|e| FuseError::InternalError(format!("Tokenization failed: {}", e)))?;

        Ok(encoding
            .get_ids()
            .iter()
            .zip(encoding.get_tokens().iter())
            .map(|(&id, token)| (id, token.clone()))
            .collect())
    }

    #[cfg(not(feature = "cpu-inference"))]
    pub fn encode_with_tokens(&self, _text: &str) -> Result<Vec<(u32, String)>> {
        Err(FuseError::InternalError(
            "Tokenizer requires cpu-inference feature".to_string(),
        ))
    }
}

/// Detect the tokenizer type from the inner tokenizer.
#[cfg(feature = "cpu-inference")]
fn detect_tokenizer_type(tokenizer: &tokenizers::Tokenizer) -> TokenizerType {
    // Heuristic: check the model type via JSON serialization
    let json = serde_json::to_string(tokenizer.get_model()).unwrap_or_default();
    if json.contains("\"type\":\"BPE\"") || json.contains("\"type\": \"BPE\"") {
        TokenizerType::Bpe
    } else if json.contains("Unigram") {
        TokenizerType::SentencePiece
    } else {
        TokenizerType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_type_enum() {
        assert_ne!(TokenizerType::Bpe, TokenizerType::SentencePiece);
        assert_ne!(TokenizerType::Tiktoken, TokenizerType::Unknown);
    }

    #[cfg(feature = "cpu-inference")]
    #[test]
    fn test_from_pretrained_gpt2() {
        // GPT-2 tokenizer is a common BPE tokenizer
        let tok = FuseTokenizer::from_pretrained("gpt2");
        if let Ok(tok) = tok {
            assert!(tok.vocab_size() > 0);
            assert_eq!(tok.tokenizer_type(), TokenizerType::Bpe);

            // Encode and decode roundtrip
            let text = "Hello, world!";
            let ids = tok.encode(text, false).unwrap();
            assert!(!ids.is_empty());

            let decoded = tok.decode(&ids, false).unwrap();
            assert_eq!(decoded, text);
        }
        // If download fails (no network), skip gracefully
    }

    #[cfg(feature = "cpu-inference")]
    #[test]
    fn test_encode_decode_basic() {
        // Try loading gpt2 tokenizer
        let tok = FuseTokenizer::from_pretrained("gpt2");
        if let Ok(tok) = tok {
            let ids = tok.encode("test", false).unwrap();
            assert!(!ids.is_empty());

            let decoded = tok.decode(&ids, true).unwrap();
            assert_eq!(decoded, "test");
        }
    }

    #[test]
    fn test_from_file_nonexistent() {
        let result = FuseTokenizer::from_file(Path::new("/nonexistent/tokenizer.json"));
        assert!(result.is_err());
    }
}
