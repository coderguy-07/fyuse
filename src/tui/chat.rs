//! Chat state management for the TUI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
        }
    }
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub model: Option<String>,
    pub tokens: Option<usize>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            timestamp: Utc::now(),
            model: None,
            tokens: None,
        }
    }

    pub fn assistant(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            model: Some(model.into()),
            tokens: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            timestamp: Utc::now(),
            model: None,
            tokens: None,
        }
    }
}

/// Chat session state.
#[derive(Debug)]
pub struct ChatSession {
    pub messages: VecDeque<ChatMessage>,
    pub model: String,
    pub input: String,
    pub is_streaming: bool,
    pub streaming_buffer: String,
    max_history: usize,
}

impl ChatSession {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            messages: VecDeque::new(),
            model: model.into(),
            input: String::new(),
            is_streaming: false,
            streaming_buffer: String::new(),
            max_history: 1000,
        }
    }

    /// Add a message to the session.
    pub fn add_message(&mut self, msg: ChatMessage) {
        self.messages.push_back(msg);
        while self.messages.len() > self.max_history {
            self.messages.pop_front();
        }
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Start streaming a new assistant response.
    pub fn start_streaming(&mut self) {
        self.is_streaming = true;
        self.streaming_buffer.clear();
    }

    /// Append a token to the streaming buffer.
    pub fn append_token(&mut self, token: &str) {
        self.streaming_buffer.push_str(token);
    }

    /// Finish streaming and commit the response as a message.
    pub fn finish_streaming(&mut self) {
        self.is_streaming = false;
        let content = std::mem::take(&mut self.streaming_buffer);
        if !content.is_empty() {
            self.add_message(ChatMessage::assistant(content, &self.model));
        }
    }

    /// Get message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
        assert!(msg.model.is_none());
    }

    #[test]
    fn test_chat_message_assistant() {
        let msg = ChatMessage::assistant("Hi there", "llama3");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hi there");
        assert_eq!(msg.model.as_deref(), Some("llama3"));
    }

    #[test]
    fn test_chat_message_system() {
        let msg = ChatMessage::system("You are a helpful assistant");
        assert_eq!(msg.role, Role::System);
    }

    #[test]
    fn test_role_display() {
        assert_eq!(format!("{}", Role::User), "user");
        assert_eq!(format!("{}", Role::Assistant), "assistant");
        assert_eq!(format!("{}", Role::System), "system");
    }

    #[test]
    fn test_chat_session_new() {
        let session = ChatSession::new("llama3");
        assert_eq!(session.model, "llama3");
        assert_eq!(session.message_count(), 0);
        assert!(!session.is_streaming);
        assert!(session.input.is_empty());
    }

    #[test]
    fn test_chat_session_add_message() {
        let mut session = ChatSession::new("llama3");
        session.add_message(ChatMessage::user("Hello"));
        session.add_message(ChatMessage::assistant("Hi!", "llama3"));
        assert_eq!(session.message_count(), 2);
    }

    #[test]
    fn test_chat_session_clear() {
        let mut session = ChatSession::new("llama3");
        session.add_message(ChatMessage::user("Hello"));
        session.clear();
        assert_eq!(session.message_count(), 0);
    }

    #[test]
    fn test_chat_session_streaming() {
        let mut session = ChatSession::new("llama3");
        session.add_message(ChatMessage::user("Hello"));

        session.start_streaming();
        assert!(session.is_streaming);

        session.append_token("Hi");
        session.append_token(" there");
        session.append_token("!");

        assert_eq!(session.streaming_buffer, "Hi there!");

        session.finish_streaming();
        assert!(!session.is_streaming);
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.messages.back().unwrap().content, "Hi there!");
    }

    #[test]
    fn test_chat_session_max_history() {
        let mut session = ChatSession::new("llama3");
        session.max_history = 5;

        for i in 0..10 {
            session.add_message(ChatMessage::user(format!("Message {}", i)));
        }

        assert_eq!(session.message_count(), 5);
        // First message should be "Message 5" (oldest 5 were dropped)
        assert_eq!(session.messages.front().unwrap().content, "Message 5");
    }

    #[test]
    fn test_chat_session_finish_streaming_empty() {
        let mut session = ChatSession::new("llama3");
        session.start_streaming();
        session.finish_streaming();
        // Empty streaming buffer should not create a message
        assert_eq!(session.message_count(), 0);
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage::user("Hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"User\""));
        assert!(json.contains("\"content\":\"Hello\""));

        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, Role::User);
        assert_eq!(deserialized.content, "Hello");
    }
}
