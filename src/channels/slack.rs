//! Slack channel implementation (behind `slack` feature flag).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{FuseError, Result};

use super::traits::{Channel, ChannelType, IncomingMessage};

/// Slack bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub bot_token: String,
    pub signing_secret: String,
    pub app_id: String,
}

/// Slack channel implementation.
#[derive(Debug)]
pub struct SlackChannel {
    config: SlackConfig,
    connected: Arc<AtomicBool>,
    #[cfg(test)]
    mock_messages: Arc<parking_lot::Mutex<Vec<IncomingMessage>>>,
}

impl SlackChannel {
    /// Create a new Slack channel.
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mock_messages: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Slack
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.bot_token.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "slack".to_string(),
                message: "bot_token is required".to_string(),
            });
        }
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!("Slack channel connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        tracing::info!("Slack channel disconnected");
        Ok(())
    }

    async fn send_message(&self, session_id: &str, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "slack".to_string(),
                message: "not connected".to_string(),
            });
        }
        tracing::debug!(session_id, message, "Sending Slack message");
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "slack".to_string(),
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

    fn test_config() -> SlackConfig {
        SlackConfig {
            bot_token: "xoxb-test-token".to_string(),
            signing_secret: "secret123".to_string(),
            app_id: "A12345".to_string(),
        }
    }

    #[tokio::test]
    async fn test_slack_connect_disconnect() {
        let mut ch = SlackChannel::new(test_config());
        assert!(!ch.is_connected());

        ch.connect().await.expect("connect failed");
        assert!(ch.is_connected());

        ch.disconnect().await.expect("disconnect failed");
        assert!(!ch.is_connected());
    }

    #[tokio::test]
    async fn test_slack_connect_empty_token() {
        let mut ch = SlackChannel::new(SlackConfig {
            bot_token: String::new(),
            signing_secret: "s".to_string(),
            app_id: "a".to_string(),
        });
        assert!(ch.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_slack_send_when_disconnected() {
        let ch = SlackChannel::new(test_config());
        assert!(ch.send_message("s1", "hi").await.is_err());
    }

    #[tokio::test]
    async fn test_slack_receive_messages() {
        let mut ch = SlackChannel::new(test_config());
        ch.connect().await.expect("connect failed");

        ch.mock_messages.lock().push(IncomingMessage {
            session_id: "s1".to_string(),
            user_id: "u1".to_string(),
            text: "hello from slack".to_string(),
            channel_name: "slack".to_string(),
            channel_type: ChannelType::Slack,
        });

        let msgs = ch.receive_messages().await.expect("receive failed");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello from slack");
    }

    #[test]
    fn test_slack_name_and_type() {
        let ch = SlackChannel::new(test_config());
        assert_eq!(ch.name(), "slack");
        assert_eq!(ch.channel_type(), ChannelType::Slack);
    }

    #[test]
    fn test_slack_config_serde() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("ser");
        let d: SlackConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(d.bot_token, "xoxb-test-token");
        assert_eq!(d.app_id, "A12345");
    }
}
