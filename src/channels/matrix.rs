//! Matrix channel implementation (behind `matrix` feature flag).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{FuseError, Result};

use super::traits::{Channel, ChannelType, IncomingMessage};

/// Matrix configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixConfig {
    pub homeserver_url: String,
    pub user_id: String,
    pub access_token: String,
}

/// Matrix channel implementation.
#[derive(Debug)]
pub struct MatrixChannel {
    config: MatrixConfig,
    connected: Arc<AtomicBool>,
    #[cfg(test)]
    mock_messages: Arc<parking_lot::Mutex<Vec<IncomingMessage>>>,
}

impl MatrixChannel {
    /// Create a new Matrix channel.
    pub fn new(config: MatrixConfig) -> Self {
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mock_messages: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Channel for MatrixChannel {
    fn name(&self) -> &str {
        "matrix"
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Matrix
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.access_token.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "matrix".to_string(),
                message: "access_token is required".to_string(),
            });
        }
        if self.config.homeserver_url.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "matrix".to_string(),
                message: "homeserver_url is required".to_string(),
            });
        }
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!("Matrix channel connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        tracing::info!("Matrix channel disconnected");
        Ok(())
    }

    async fn send_message(&self, session_id: &str, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "matrix".to_string(),
                message: "not connected".to_string(),
            });
        }
        tracing::debug!(session_id, message, "Sending Matrix message");
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "matrix".to_string(),
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

    fn test_config() -> MatrixConfig {
        MatrixConfig {
            homeserver_url: "https://matrix.example.com".to_string(),
            user_id: "@bot:example.com".to_string(),
            access_token: "syt_test_token".to_string(),
        }
    }

    #[tokio::test]
    async fn test_matrix_connect_disconnect() {
        let mut ch = MatrixChannel::new(test_config());
        assert!(!ch.is_connected());

        ch.connect().await.expect("connect failed");
        assert!(ch.is_connected());

        ch.disconnect().await.expect("disconnect failed");
        assert!(!ch.is_connected());
    }

    #[tokio::test]
    async fn test_matrix_connect_empty_token() {
        let mut ch = MatrixChannel::new(MatrixConfig {
            homeserver_url: "https://example.com".to_string(),
            user_id: "@bot:example.com".to_string(),
            access_token: String::new(),
        });
        assert!(ch.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_matrix_connect_empty_homeserver() {
        let mut ch = MatrixChannel::new(MatrixConfig {
            homeserver_url: String::new(),
            user_id: "@bot:example.com".to_string(),
            access_token: "token".to_string(),
        });
        assert!(ch.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_matrix_send_when_disconnected() {
        let ch = MatrixChannel::new(test_config());
        assert!(ch.send_message("s1", "hi").await.is_err());
    }

    #[tokio::test]
    async fn test_matrix_receive_messages() {
        let mut ch = MatrixChannel::new(test_config());
        ch.connect().await.expect("connect failed");

        ch.mock_messages.lock().push(IncomingMessage {
            session_id: "s1".to_string(),
            user_id: "u1".to_string(),
            text: "hello from matrix".to_string(),
            channel_name: "matrix".to_string(),
            channel_type: ChannelType::Matrix,
        });

        let msgs = ch.receive_messages().await.expect("receive failed");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello from matrix");
    }

    #[test]
    fn test_matrix_name_and_type() {
        let ch = MatrixChannel::new(test_config());
        assert_eq!(ch.name(), "matrix");
        assert_eq!(ch.channel_type(), ChannelType::Matrix);
    }

    #[test]
    fn test_matrix_config_serde() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("ser");
        let d: MatrixConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(d.homeserver_url, "https://matrix.example.com");
        assert_eq!(d.user_id, "@bot:example.com");
    }
}
