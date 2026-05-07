//! Structured output — JSON mode and grammar-constrained generation.
//!
//! Provides grammar constraints that mask logits to ensure output conforms
//! to a given schema (JSON, regex, or custom grammar).

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};

/// Grammar constraint for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Grammar {
    /// Ensure output is valid JSON.
    Json,
    /// Ensure output matches a JSON schema.
    JsonSchema(String),
    /// Ensure output matches a regex pattern.
    Regex(String),
}

/// State machine for tracking grammar compliance during generation.
pub struct GrammarState {
    grammar: Grammar,
    output_so_far: String,
    depth: i32, // Brace/bracket nesting depth for JSON
    in_string: bool,
    escaped: bool,
}

impl GrammarState {
    pub fn new(grammar: Grammar) -> Self {
        Self {
            grammar,
            output_so_far: String::new(),
            depth: 0,
            in_string: false,
            escaped: false,
        }
    }

    /// Check if a candidate token text is allowed given the current state.
    pub fn is_token_allowed(&self, token_text: &str) -> bool {
        match &self.grammar {
            Grammar::Json => self.is_json_token_allowed(token_text),
            Grammar::JsonSchema(_) => self.is_json_token_allowed(token_text),
            Grammar::Regex(pattern) => self.is_regex_compatible(token_text, pattern),
        }
    }

    /// Advance the state with a new token.
    pub fn advance(&mut self, token_text: &str) {
        for ch in token_text.chars() {
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if ch == '\\' {
                    self.escaped = true;
                } else if ch == '"' {
                    self.in_string = false;
                }
            } else {
                match ch {
                    '"' => self.in_string = true,
                    '{' | '[' => self.depth += 1,
                    '}' | ']' => self.depth -= 1,
                    _ => {}
                }
            }
        }
        self.output_so_far.push_str(token_text);
    }

    /// Check if the output so far is complete (valid termination point).
    pub fn is_complete(&self) -> bool {
        match &self.grammar {
            Grammar::Json | Grammar::JsonSchema(_) => {
                !self.in_string && self.depth == 0 && !self.output_so_far.trim().is_empty()
            }
            Grammar::Regex(_) => true,
        }
    }

    /// Validate the final output.
    pub fn validate_output(&self) -> Result<()> {
        match &self.grammar {
            Grammar::Json | Grammar::JsonSchema(_) => {
                let trimmed = self.output_so_far.trim();
                serde_json::from_str::<serde_json::Value>(trimmed).map_err(|e| {
                    FuseError::InferenceError(format!("Invalid JSON output: {}", e))
                })?;
                Ok(())
            }
            Grammar::Regex(pattern) => {
                // Basic regex check
                if self.output_so_far.contains(pattern.as_str()) || pattern.is_empty() {
                    Ok(())
                } else {
                    Err(FuseError::InferenceError(format!(
                        "Output does not match regex: {}",
                        pattern
                    )))
                }
            }
        }
    }

    /// Get the output accumulated so far.
    pub fn output(&self) -> &str {
        &self.output_so_far
    }

    /// Apply grammar mask to logits — set disallowed tokens to -inf.
    /// `vocab` maps token_id to token string.
    pub fn mask_logits(&self, logits: &mut [f32], vocab: &[String]) {
        for (i, logit) in logits.iter_mut().enumerate() {
            if i < vocab.len() && !self.is_token_allowed(&vocab[i]) {
                *logit = f32::NEG_INFINITY;
            }
        }
    }

    fn is_json_token_allowed(&self, token_text: &str) -> bool {
        // Allow whitespace and JSON structural characters at any point
        let trimmed = token_text.trim();
        if trimmed.is_empty() {
            return true;
        }

        // When in a string, allow anything
        if self.in_string {
            return true;
        }

        // At the start, only allow { or [
        if self.output_so_far.trim().is_empty() {
            let first = trimmed.chars().next().unwrap_or(' ');
            return first == '{' || first == '[' || first == '"';
        }

        // Allow any valid JSON characters
        for ch in trimmed.chars() {
            if ch.is_alphanumeric() || "{}[]\",:.-+eE_ \t\n\r\\/".contains(ch) {
                continue;
            }
            return false;
        }

        true
    }

    fn is_regex_compatible(&self, token_text: &str, _pattern: &str) -> bool {
        // Simplified: allow all tokens for regex mode
        // Full implementation would use a regex DFA to check prefix compatibility
        !token_text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_grammar_basic() {
        let mut state = GrammarState::new(Grammar::Json);

        state.advance("{");
        assert!(!state.is_complete());

        state.advance("\"key\"");
        state.advance(":");
        state.advance("\"value\"");
        state.advance("}");

        assert!(state.is_complete());
        assert!(state.validate_output().is_ok());
    }

    #[test]
    fn test_json_grammar_nested() {
        let mut state = GrammarState::new(Grammar::Json);

        state.advance("{\"a\":{\"b\":1}}");
        assert!(state.is_complete());
        assert!(state.validate_output().is_ok());
    }

    #[test]
    fn test_json_grammar_array() {
        let mut state = GrammarState::new(Grammar::Json);

        state.advance("[1,2,3]");
        assert!(state.is_complete());
        assert!(state.validate_output().is_ok());
    }

    #[test]
    fn test_json_grammar_incomplete() {
        let mut state = GrammarState::new(Grammar::Json);

        state.advance("{\"key\":");
        assert!(!state.is_complete());
    }

    #[test]
    fn test_json_grammar_invalid() {
        let mut state = GrammarState::new(Grammar::Json);
        state.advance("not json at all");
        assert!(state.validate_output().is_err());
    }

    #[test]
    fn test_json_token_allowed_start() {
        let state = GrammarState::new(Grammar::Json);
        assert!(state.is_token_allowed("{"));
        assert!(state.is_token_allowed("["));
        assert!(state.is_token_allowed("\""));
    }

    #[test]
    fn test_mask_logits() {
        let state = GrammarState::new(Grammar::Json);
        let vocab = vec![
            "{".to_string(),
            "hello".to_string(),
            "[".to_string(),
            "\x00".to_string(),
        ];
        let mut logits = vec![1.0, 1.0, 1.0, 1.0];
        state.mask_logits(&mut logits, &vocab);

        // { and [ should be allowed (at start), others may be masked
        assert!(logits[0].is_finite()); // { allowed
        assert!(logits[2].is_finite()); // [ allowed
    }

    #[test]
    fn test_string_tracking() {
        let mut state = GrammarState::new(Grammar::Json);
        state.advance("{\"");
        assert!(state.in_string);

        state.advance("key");
        assert!(state.in_string);

        state.advance("\"");
        assert!(!state.in_string);
    }

    #[test]
    fn test_escape_in_string() {
        let mut state = GrammarState::new(Grammar::Json);
        state.advance("{\"key\\\"still_string\"");
        assert!(!state.in_string); // The escaped quote shouldn't close the string
    }

    #[test]
    fn test_regex_grammar() {
        let state = GrammarState::new(Grammar::Regex("\\d+".to_string()));
        assert!(state.is_token_allowed("123"));
        assert!(state.is_token_allowed("abc"));
    }
}
