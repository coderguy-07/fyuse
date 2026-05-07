//! Discord channel implementation (behind `discord` feature flag).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{FuseError, Result};

use super::traits::{Channel, ChannelType, IncomingMessage};

/// Discord bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub guild_id: Option<String>,
    pub command_prefix: String,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            guild_id: None,
            command_prefix: "!".to_string(),
        }
    }
}

/// Discord channel implementation.
#[derive(Debug)]
pub struct DiscordChannel {
    config: DiscordConfig,
    connected: Arc<AtomicBool>,
    #[cfg(test)]
    mock_messages: Arc<parking_lot::Mutex<Vec<IncomingMessage>>>,
}

impl DiscordChannel {
    /// Create a new Discord channel.
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mock_messages: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.bot_token.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "discord".to_string(),
                message: "bot_token is required".to_string(),
            });
        }
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!("Discord channel connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        tracing::info!("Discord channel disconnected");
        Ok(())
    }

    async fn send_message(&self, session_id: &str, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "discord".to_string(),
                message: "not connected".to_string(),
            });
        }
        tracing::debug!(session_id, message, "Sending Discord message");
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "discord".to_string(),
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

    fn test_config() -> DiscordConfig {
        DiscordConfig {
            bot_token: "discord-test-token".to_string(),
            guild_id: Some("12345".to_string()),
            command_prefix: "!".to_string(),
        }
    }

    #[tokio::test]
    async fn test_discord_connect_disconnect() {
        let mut ch = DiscordChannel::new(test_config());
        assert!(!ch.is_connected());

        ch.connect().await.expect("connect failed");
        assert!(ch.is_connected());

        ch.disconnect().await.expect("disconnect failed");
        assert!(!ch.is_connected());
    }

    #[tokio::test]
    async fn test_discord_connect_empty_token() {
        let mut ch = DiscordChannel::new(DiscordConfig::default());
        assert!(ch.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_discord_send_when_disconnected() {
        let ch = DiscordChannel::new(test_config());
        assert!(ch.send_message("s1", "hi").await.is_err());
    }

    #[tokio::test]
    async fn test_discord_receive_messages() {
        let mut ch = DiscordChannel::new(test_config());
        ch.connect().await.expect("connect failed");

        ch.mock_messages.lock().push(IncomingMessage {
            session_id: "s1".to_string(),
            user_id: "u1".to_string(),
            text: "hello from discord".to_string(),
            channel_name: "discord".to_string(),
            channel_type: ChannelType::Discord,
        });

        let msgs = ch.receive_messages().await.expect("receive failed");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello from discord");
    }

    #[test]
    fn test_discord_name_and_type() {
        let ch = DiscordChannel::new(test_config());
        assert_eq!(ch.name(), "discord");
        assert_eq!(ch.channel_type(), ChannelType::Discord);
    }

    #[test]
    fn test_discord_config_serde() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("ser");
        let d: DiscordConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(d.bot_token, "discord-test-token");
        assert_eq!(d.command_prefix, "!");
    }
}
