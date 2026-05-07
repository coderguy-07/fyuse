//! Dioxus-based UI for Fuse — desktop, web, and mobile from one codebase.
// Dioxus 0.6 `Props` derive macro uses features from Rust 1.76+.
#![allow(clippy::incompatible_msrv)]

pub mod app;
pub mod components;
pub mod pages;

use serde::{Deserialize, Serialize};

/// A chat message in the Dioxus UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub tokens: Option<usize>,
}

/// The role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Model information for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub loaded: bool,
    pub quantization: Option<String>,
}

/// Channel configuration info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub channel_type: String,
    pub enabled: bool,
    pub connected_users: u32,
}

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message {
            role: MessageRole::User,
            content: "Hello".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            model: Some("llama3".into()),
            tokens: Some(5),
        };
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_role_equality() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
    }

    #[test]
    fn test_model_info() {
        let m = ModelInfo {
            id: "llama3".into(),
            name: "LLaMA 3".into(),
            size_bytes: 7_000_000_000,
            loaded: true,
            quantization: Some("Q4_K_M".into()),
        };
        assert!(m.loaded);
        assert_eq!(m.quantization, Some("Q4_K_M".into()));
    }

    #[test]
    fn test_channel_info() {
        let c = ChannelInfo {
            id: "discord-1".into(),
            name: "Discord Bot".into(),
            channel_type: "discord".into(),
            enabled: true,
            connected_users: 42,
        };
        assert!(c.enabled);
        assert_eq!(c.connected_users, 42);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_500_000), "1.4 MB");
        assert_eq!(format_bytes(7_000_000_000), "6.5 GB");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: MessageRole::Assistant,
            content: "Hi".into(),
            timestamp: "now".into(),
            model: None,
            tokens: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }
}
