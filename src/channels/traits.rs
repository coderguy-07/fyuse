//! Channel trait — abstraction for communication surfaces (Telegram, Discord, etc).

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The type of channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    Telegram,
    Discord,
    Slack,
    Matrix,
    WebWidget,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Telegram => write!(f, "telegram"),
            Self::Discord => write!(f, "discord"),
            Self::Slack => write!(f, "slack"),
            Self::Matrix => write!(f, "matrix"),
            Self::WebWidget => write!(f, "web_widget"),
        }
    }
}

/// Configuration for a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub enabled: bool,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_history: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            system_prompt: None,
            max_history: 50,
        }
    }
}

/// An outgoing message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub text: String,
    pub metadata: std::collections::HashMap<String, String>,
}

/// An incoming message from a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub session_id: String,
    pub user_id: String,
    pub text: String,
    pub channel_name: String,
    pub channel_type: ChannelType,
}

/// Core trait for communication channels.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel name (e.g., "telegram", "discord").
    fn name(&self) -> &str;

    /// The type of this channel.
    fn channel_type(&self) -> ChannelType;

    /// Connect / start the channel.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect / stop the channel.
    async fn disconnect(&mut self) -> Result<()>;

    /// Send a message to a session.
    async fn send_message(&self, session_id: &str, message: &str) -> Result<()>;

    /// Receive pending messages (poll-based).
    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>>;

    /// Whether the channel is currently connected.
    fn is_connected(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct MockChannel {
        channel_name: String,
        connected: Arc<AtomicBool>,
    }

    impl MockChannel {
        fn new() -> Self {
            Self {
                channel_name: "mock".to_string(),
                connected: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.channel_name
        }

        fn channel_type(&self) -> ChannelType {
            ChannelType::WebWidget
        }

        async fn connect(&mut self) -> Result<()> {
            self.connected.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<()> {
            self.connected.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn send_message(&self, _session_id: &str, _message: &str) -> Result<()> {
            Ok(())
        }

        async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
            Ok(vec![IncomingMessage {
                session_id: "s1".to_string(),
                user_id: "u1".to_string(),
                text: "hello".to_string(),
                channel_name: self.channel_name.clone(),
                channel_type: ChannelType::WebWidget,
            }])
        }

        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn test_channel_lifecycle() {
        let mut ch = MockChannel::new();
        assert_eq!(ch.name(), "mock");
        assert_eq!(ch.channel_type(), ChannelType::WebWidget);
        assert!(!ch.is_connected());

        ch.connect().await.expect("connect failed");
        assert!(ch.is_connected());

        ch.send_message("s1", "hi").await.expect("send failed");

        let msgs = ch.receive_messages().await.expect("receive failed");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello");

        ch.disconnect().await.expect("disconnect failed");
        assert!(!ch.is_connected());
    }

    #[test]
    fn test_channel_type_display() {
        assert_eq!(ChannelType::Telegram.to_string(), "telegram");
        assert_eq!(ChannelType::Discord.to_string(), "discord");
        assert_eq!(ChannelType::Slack.to_string(), "slack");
        assert_eq!(ChannelType::Matrix.to_string(), "matrix");
        assert_eq!(ChannelType::WebWidget.to_string(), "web_widget");
    }

    #[test]
    fn test_channel_config_default() {
        let config = ChannelConfig::default();
        assert!(!config.enabled);
        assert!(config.model.is_none());
        assert!(config.system_prompt.is_none());
        assert_eq!(config.max_history, 50);
    }

    #[test]
    fn test_incoming_message_serde() {
        let msg = IncomingMessage {
            session_id: "s1".to_string(),
            user_id: "u1".to_string(),
            text: "hello".to_string(),
            channel_name: "test".to_string(),
            channel_type: ChannelType::Telegram,
        };
        let json = serde_json::to_string(&msg).expect("serialize failed");
        let deser: IncomingMessage = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deser.text, "hello");
        assert_eq!(deser.channel_type, ChannelType::Telegram);
    }

    #[test]
    fn test_message_creation() {
        let msg = Message {
            text: "hello".to_string(),
            metadata: HashMap::new(),
        };
        assert_eq!(msg.text, "hello");
        assert!(msg.metadata.is_empty());
    }
}
