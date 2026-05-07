//! Telegram channel implementation (behind `telegram` feature flag).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{FuseError, Result};

use super::traits::{Channel, ChannelType, IncomingMessage};

/// Telegram bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub allowed_users: Vec<String>,
    pub webhook_url: Option<String>,
}

/// Telegram channel implementation.
#[derive(Debug)]
pub struct TelegramChannel {
    config: TelegramConfig,
    connected: Arc<AtomicBool>,
    #[cfg(test)]
    mock_messages: Arc<parking_lot::Mutex<Vec<IncomingMessage>>>,
}

impl TelegramChannel {
    /// Create a new Telegram channel.
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mock_messages: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.bot_token.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "telegram".to_string(),
                message: "bot_token is required".to_string(),
            });
        }
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!("Telegram channel connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        tracing::info!("Telegram channel disconnected");
        Ok(())
    }

    async fn send_message(&self, session_id: &str, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "telegram".to_string(),
                message: "not connected".to_string(),
            });
        }
        tracing::debug!(session_id, message, "Sending Telegram message");
        // Real implementation would call Telegram Bot API here
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "telegram".to_string(),
                message: "not connected".to_string(),
            });
        }
        #[cfg(test)]
        {
            let mut msgs = self.mock_messages.lock();
            return Ok(msgs.drain(..).collect());
        }
        #[cfg(not(test))]
        {
            // Real implementation would poll Telegram getUpdates
            Ok(Vec::new())
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TelegramConfig {
        TelegramConfig {
            bot_token: "test-token-123".to_string(),
            allowed_users: vec!["user1".to_string()],
            webhook_url: None,
        }
    }

    #[tokio::test]
    async fn test_telegram_connect_disconnect() {
        let mut ch = TelegramChannel::new(test_config());
        assert!(!ch.is_connected());

        ch.connect().await.expect("connect failed");
        assert!(ch.is_connected());

        ch.disconnect().await.expect("disconnect failed");
        assert!(!ch.is_connected());
    }

    #[tokio::test]
    async fn test_telegram_connect_empty_token() {
        let mut ch = TelegramChannel::new(TelegramConfig {
            bot_token: String::new(),
            allowed_users: vec![],
            webhook_url: None,
        });
        let result = ch.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_telegram_send_when_disconnected() {
        let ch = TelegramChannel::new(test_config());
        let result = ch.send_message("s1", "hello").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_telegram_send_when_connected() {
        let mut ch = TelegramChannel::new(test_config());
        ch.connect().await.expect("connect failed");
        ch.send_message("s1", "hello").await.expect("send failed");
    }

    #[tokio::test]
    async fn test_telegram_receive_messages() {
        let mut ch = TelegramChannel::new(test_config());
        ch.connect().await.expect("connect failed");

        // Inject mock message
        ch.mock_messages.lock().push(IncomingMessage {
            session_id: "s1".to_string(),
            user_id: "u1".to_string(),
            text: "hello from telegram".to_string(),
            channel_name: "telegram".to_string(),
            channel_type: ChannelType::Telegram,
        });

        let msgs = ch.receive_messages().await.expect("receive failed");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello from telegram");
    }

    #[tokio::test]
    async fn test_telegram_receive_when_disconnected() {
        let ch = TelegramChannel::new(test_config());
        let result = ch.receive_messages().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_telegram_name_and_type() {
        let ch = TelegramChannel::new(test_config());
        assert_eq!(ch.name(), "telegram");
        assert_eq!(ch.channel_type(), ChannelType::Telegram);
    }

    #[test]
    fn test_telegram_config_serde() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize failed");
        let deser: TelegramConfig = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deser.bot_token, "test-token-123");
        assert_eq!(deser.allowed_users.len(), 1);
    }
}
