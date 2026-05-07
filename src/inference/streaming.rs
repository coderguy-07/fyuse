//! Token streaming utilities — helpers for streaming inference output.

use crate::error::Result;
use crate::inference::backend::Token;
use futures::stream::BoxStream;
use futures::StreamExt;

/// Collect all tokens from a stream into a single string.
pub async fn collect_stream(mut stream: BoxStream<'_, Result<Token>>) -> Result<CollectedOutput> {
    let mut tokens = Vec::new();
    let mut text = String::new();

    while let Some(result) = stream.next().await {
        let token = result?;
        text.push_str(&token.text);
        tokens.push(token);
    }

    Ok(CollectedOutput { text, tokens })
}

/// Result of collecting a stream.
pub struct CollectedOutput {
    pub text: String,
    pub tokens: Vec<Token>,
}

impl CollectedOutput {
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn token_ids(&self) -> Vec<u32> {
        self.tokens.iter().map(|t| t.id).collect()
    }
}

/// Adapter that maps a token stream through a callback.
pub async fn stream_with_callback<F>(
    mut stream: BoxStream<'_, Result<Token>>,
    mut callback: F,
) -> Result<Vec<Token>>
where
    F: FnMut(&Token),
{
    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        let token = result?;
        callback(&token);
        tokens.push(token);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    fn make_test_stream() -> BoxStream<'static, Result<Token>> {
        let tokens = vec![
            Ok(Token {
                text: "Hello".to_string(),
                id: 1,
                logprob: None,
            }),
            Ok(Token {
                text: " world".to_string(),
                id: 2,
                logprob: None,
            }),
            Ok(Token {
                text: "!".to_string(),
                id: 3,
                logprob: None,
            }),
        ];
        Box::pin(stream::iter(tokens))
    }

    #[tokio::test]
    async fn test_collect_stream() {
        let stream = make_test_stream();
        let output = collect_stream(stream).await.unwrap();

        assert_eq!(output.text, "Hello world!");
        assert_eq!(output.token_count(), 3);
        assert_eq!(output.token_ids(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_stream_with_callback() {
        let stream = make_test_stream();
        let mut received = Vec::new();

        let tokens = stream_with_callback(stream, |token| {
            received.push(token.text.clone());
        })
        .await
        .unwrap();

        assert_eq!(tokens.len(), 3);
        assert_eq!(received, vec!["Hello", " world", "!"]);
    }

    #[tokio::test]
    async fn test_collect_empty_stream() {
        let stream: BoxStream<'static, Result<Token>> = Box::pin(stream::empty());
        let output = collect_stream(stream).await.unwrap();
        assert_eq!(output.text, "");
        assert_eq!(output.token_count(), 0);
    }
}
