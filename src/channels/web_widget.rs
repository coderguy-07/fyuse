//! Web chat widget channel — embeddable WASM widget that connects to Fuse API.
//!
//! Provides a lightweight web chat widget that can be embedded in any webpage
//! via a single `<script>` tag. Communicates with the Fuse API server over
//! WebSocket for real-time streaming.
//!
//! ## Architecture
//!
//! The widget channel has two parts:
//! 1. **Server-side** (this module): Manages WebSocket sessions, routes messages
//!    to the inference engine, and streams responses back.
//! 2. **Client-side** (widget/ directory): A WASM/JS bundle that renders the
//!    chat UI in the browser. (Build separately with `cargo build --target wasm32-unknown-unknown`)
//!
//! ## Configuration
//!
//! ```toml
//! [channels.web_widget]
//! enabled = true
//! cors_origins = ["*"]
//! max_sessions = 100
//! session_timeout_secs = 3600
//! theme = "auto"
//! title = "Fuse Chat"
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{FuseError, Result};

use super::traits::{Channel, ChannelType, IncomingMessage};

/// Web widget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebWidgetConfig {
    /// CORS allowed origins.
    pub cors_origins: Vec<String>,
    /// Maximum concurrent sessions.
    pub max_sessions: usize,
    /// Session timeout in seconds.
    pub session_timeout_secs: u64,
    /// Widget theme: "auto", "dark", "light".
    pub theme: String,
    /// Widget title displayed in the header.
    pub title: String,
    /// API endpoint the widget connects to.
    pub api_endpoint: String,
    /// Whether to enable streaming responses.
    pub streaming: bool,
}

impl Default for WebWidgetConfig {
    fn default() -> Self {
        Self {
            cors_origins: vec!["*".to_string()],
            max_sessions: 100,
            session_timeout_secs: 3600,
            theme: "auto".to_string(),
            title: "Fuse Chat".to_string(),
            api_endpoint: "ws://localhost:11434/ws".to_string(),
            streaming: true,
        }
    }
}

impl WebWidgetConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.max_sessions == 0 {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: "max_sessions must be > 0".to_string(),
            });
        }
        if self.session_timeout_secs == 0 {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: "session_timeout_secs must be > 0".to_string(),
            });
        }
        if self.api_endpoint.is_empty() {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: "api_endpoint is required".to_string(),
            });
        }
        Ok(())
    }
}

/// A WebSocket session for a connected widget client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSession {
    pub session_id: String,
    pub user_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
    pub metadata: HashMap<String, String>,
}

impl WidgetSession {
    pub fn new(session_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            session_id: session_id.into(),
            user_id: user_id.into(),
            created_at: now,
            last_active: now,
            message_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Check if the session has expired.
    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(self.last_active)
            .num_seconds();
        elapsed > timeout_secs as i64
    }

    /// Touch the session to update last_active.
    pub fn touch(&mut self) {
        self.last_active = chrono::Utc::now();
        self.message_count += 1;
    }
}

/// WebSocket message types between widget and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WidgetMessage {
    /// Client sends a chat message.
    #[serde(rename = "chat")]
    Chat { text: String },
    /// Server sends a response token (streaming).
    #[serde(rename = "token")]
    Token { text: String },
    /// Server sends completion signal.
    #[serde(rename = "done")]
    Done { total_tokens: Option<usize> },
    /// Server sends an error.
    #[serde(rename = "error")]
    Error { message: String },
    /// Ping/pong for keepalive.
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "pong")]
    Pong,
}

/// Web widget channel — manages widget sessions and routes messages.
pub struct WebWidgetChannel {
    config: WebWidgetConfig,
    connected: Arc<AtomicBool>,
    sessions: Arc<parking_lot::Mutex<HashMap<String, WidgetSession>>>,
    pending_messages: Arc<parking_lot::Mutex<Vec<IncomingMessage>>>,
}

impl std::fmt::Debug for WebWidgetChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebWidgetChannel")
            .field("config", &self.config)
            .field("connected", &self.connected.load(Ordering::SeqCst))
            .finish()
    }
}

impl WebWidgetChannel {
    /// Create a new web widget channel.
    pub fn new(config: WebWidgetConfig) -> Self {
        Self {
            config,
            connected: Arc::new(AtomicBool::new(false)),
            sessions: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            pending_messages: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Get the number of active sessions.
    pub fn active_session_count(&self) -> usize {
        let sessions = self.sessions.lock();
        sessions
            .values()
            .filter(|s| !s.is_expired(self.config.session_timeout_secs))
            .count()
    }

    /// Register a new widget session.
    pub fn register_session(
        &self,
        session_id: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Result<()> {
        let mut sessions = self.sessions.lock();

        // Check capacity
        let active = sessions
            .values()
            .filter(|s| !s.is_expired(self.config.session_timeout_secs))
            .count();
        if active >= self.config.max_sessions {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: format!("Max sessions ({}) reached", self.config.max_sessions),
            });
        }

        let session = WidgetSession::new(session_id, user_id);
        sessions.insert(session.session_id.clone(), session);
        Ok(())
    }

    /// Remove a session.
    pub fn remove_session(&self, session_id: &str) -> bool {
        self.sessions.lock().remove(session_id).is_some()
    }

    /// Handle an incoming widget message for a session.
    pub fn handle_widget_message(
        &self,
        session_id: &str,
        user_id: &str,
        msg: &WidgetMessage,
    ) -> Result<()> {
        // Touch session
        {
            let mut sessions = self.sessions.lock();
            if let Some(session) = sessions.get_mut(session_id) {
                session.touch();
            }
        }

        match msg {
            WidgetMessage::Chat { text } => {
                let incoming = IncomingMessage {
                    session_id: session_id.to_string(),
                    user_id: user_id.to_string(),
                    text: text.clone(),
                    channel_name: "web_widget".to_string(),
                    channel_type: ChannelType::WebWidget,
                };
                self.pending_messages.lock().push(incoming);
                Ok(())
            }
            WidgetMessage::Ping => Ok(()), // Handled at transport level
            _ => Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: "Unexpected client message type".to_string(),
            }),
        }
    }

    /// Evict expired sessions.
    pub fn evict_expired(&self) -> usize {
        let mut sessions = self.sessions.lock();
        let before = sessions.len();
        sessions.retain(|_, s| !s.is_expired(self.config.session_timeout_secs));
        before - sessions.len()
    }

    /// Generate the embeddable script tag HTML.
    pub fn embed_script(&self) -> String {
        format!(
            r#"<script src="{endpoint}/widget.js" data-theme="{theme}" data-title="{title}"></script>"#,
            endpoint = self
                .config
                .api_endpoint
                .replace("ws://", "http://")
                .replace("wss://", "https://")
                .trim_end_matches("/ws"),
            theme = self.config.theme,
            title = self.config.title,
        )
    }
}

#[async_trait]
impl Channel for WebWidgetChannel {
    fn name(&self) -> &str {
        "web_widget"
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::WebWidget
    }

    async fn connect(&mut self) -> Result<()> {
        self.config.validate()?;
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!(
            max_sessions = self.config.max_sessions,
            endpoint = %self.config.api_endpoint,
            "Web widget channel connected"
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        self.sessions.lock().clear();
        tracing::info!("Web widget channel disconnected");
        Ok(())
    }

    async fn send_message(&self, session_id: &str, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: "not connected".to_string(),
            });
        }

        // Verify session exists
        let sessions = self.sessions.lock();
        if !sessions.contains_key(session_id) {
            return Err(FuseError::ChannelError {
                channel: "web_widget".to_string(),
                message: format!("Session not found: {session_id}"),
            });
        }

        tracing::debug!(session_id, message, "Sending web widget message");
        // In production: send via WebSocket to the client
        Ok(())
    }

    async fn receive_messages(&self) -> Result<Vec<IncomingMessage>> {
        let mut pending = self.pending_messages.lock();
        let messages = std::mem::take(&mut *pending);
        Ok(messages)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_channel() -> WebWidgetChannel {
        WebWidgetChannel::new(WebWidgetConfig::default())
    }

    #[test]
    fn test_config_default() {
        let config = WebWidgetConfig::default();
        assert_eq!(config.max_sessions, 100);
        assert_eq!(config.theme, "auto");
        assert_eq!(config.title, "Fuse Chat");
        assert!(config.streaming);
    }

    #[test]
    fn test_config_validate_ok() {
        let config = WebWidgetConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_zero_sessions() {
        let config = WebWidgetConfig {
            max_sessions: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_timeout() {
        let config = WebWidgetConfig {
            session_timeout_secs: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_empty_endpoint() {
        let config = WebWidgetConfig {
            api_endpoint: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_widget_session_new() {
        let s = WidgetSession::new("s1", "u1");
        assert_eq!(s.session_id, "s1");
        assert_eq!(s.user_id, "u1");
        assert_eq!(s.message_count, 0);
        assert!(!s.is_expired(3600));
    }

    #[test]
    fn test_widget_session_touch() {
        let mut s = WidgetSession::new("s1", "u1");
        s.touch();
        assert_eq!(s.message_count, 1);
        s.touch();
        assert_eq!(s.message_count, 2);
    }

    #[test]
    fn test_widget_session_expired() {
        let mut s = WidgetSession::new("s1", "u1");
        // Force last_active to the past
        s.last_active = chrono::Utc::now() - chrono::Duration::seconds(100);
        assert!(s.is_expired(50));
        assert!(!s.is_expired(200));
    }

    #[test]
    fn test_register_session() {
        let ch = default_channel();
        assert!(ch.register_session("s1", "u1").is_ok());
        assert_eq!(ch.active_session_count(), 1);
    }

    #[test]
    fn test_register_session_max_reached() {
        let ch = WebWidgetChannel::new(WebWidgetConfig {
            max_sessions: 2,
            ..Default::default()
        });
        ch.register_session("s1", "u1").unwrap();
        ch.register_session("s2", "u2").unwrap();
        assert!(ch.register_session("s3", "u3").is_err());
    }

    #[test]
    fn test_remove_session() {
        let ch = default_channel();
        ch.register_session("s1", "u1").unwrap();
        assert!(ch.remove_session("s1"));
        assert!(!ch.remove_session("s1")); // Already removed
        assert_eq!(ch.active_session_count(), 0);
    }

    #[test]
    fn test_handle_chat_message() {
        let ch = default_channel();
        ch.register_session("s1", "u1").unwrap();

        let msg = WidgetMessage::Chat {
            text: "hello".to_string(),
        };
        ch.handle_widget_message("s1", "u1", &msg).unwrap();

        let pending = ch.pending_messages.lock();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].text, "hello");
        assert_eq!(pending[0].channel_type, ChannelType::WebWidget);
    }

    #[test]
    fn test_handle_ping_message() {
        let ch = default_channel();
        ch.register_session("s1", "u1").unwrap();
        let msg = WidgetMessage::Ping;
        assert!(ch.handle_widget_message("s1", "u1", &msg).is_ok());
    }

    #[test]
    fn test_handle_unexpected_message() {
        let ch = default_channel();
        ch.register_session("s1", "u1").unwrap();
        let msg = WidgetMessage::Token {
            text: "bad".to_string(),
        };
        assert!(ch.handle_widget_message("s1", "u1", &msg).is_err());
    }

    #[test]
    fn test_evict_expired() {
        let ch = default_channel();
        ch.register_session("s1", "u1").unwrap();
        ch.register_session("s2", "u2").unwrap();

        // Force s1 to expire
        {
            let mut sessions = ch.sessions.lock();
            sessions.get_mut("s1").unwrap().last_active =
                chrono::Utc::now() - chrono::Duration::seconds(7200);
        }

        let evicted = ch.evict_expired();
        assert_eq!(evicted, 1);
        assert_eq!(ch.active_session_count(), 1);
    }

    #[tokio::test]
    async fn test_channel_lifecycle() {
        let mut ch = default_channel();
        assert!(!ch.is_connected());

        ch.connect().await.unwrap();
        assert!(ch.is_connected());

        ch.register_session("s1", "u1").unwrap();
        ch.send_message("s1", "hello").await.unwrap();

        ch.disconnect().await.unwrap();
        assert!(!ch.is_connected());
        assert_eq!(ch.active_session_count(), 0);
    }

    #[tokio::test]
    async fn test_send_message_not_connected() {
        let ch = default_channel();
        assert!(ch.send_message("s1", "hello").await.is_err());
    }

    #[tokio::test]
    async fn test_send_message_unknown_session() {
        let mut ch = default_channel();
        ch.connect().await.unwrap();
        assert!(ch.send_message("nonexistent", "hello").await.is_err());
    }

    #[tokio::test]
    async fn test_receive_messages() {
        let mut ch = default_channel();
        ch.connect().await.unwrap();
        ch.register_session("s1", "u1").unwrap();

        let msg = WidgetMessage::Chat {
            text: "hello".to_string(),
        };
        ch.handle_widget_message("s1", "u1", &msg).unwrap();

        let msgs = ch.receive_messages().await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello");

        // Second call should be empty (messages consumed)
        let msgs = ch.receive_messages().await.unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_embed_script() {
        let ch = default_channel();
        let script = ch.embed_script();
        assert!(script.contains("<script"));
        assert!(script.contains("widget.js"));
        assert!(script.contains("data-theme"));
        assert!(script.contains("Fuse Chat"));
    }

    #[test]
    fn test_widget_message_serde_chat() {
        let msg = WidgetMessage::Chat {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"chat\""));
        let back: WidgetMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, WidgetMessage::Chat { text } if text == "hello"));
    }

    #[test]
    fn test_widget_message_serde_token() {
        let msg = WidgetMessage::Token {
            text: "hi".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: WidgetMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, WidgetMessage::Token { text } if text == "hi"));
    }

    #[test]
    fn test_widget_message_serde_done() {
        let msg = WidgetMessage::Done {
            total_tokens: Some(42),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: WidgetMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            WidgetMessage::Done {
                total_tokens: Some(42)
            }
        ));
    }

    #[test]
    fn test_widget_message_serde_error() {
        let msg = WidgetMessage::Error {
            message: "fail".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: WidgetMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, WidgetMessage::Error { message } if message == "fail"));
    }

    #[test]
    fn test_widget_message_serde_ping_pong() {
        let ping_json = serde_json::to_string(&WidgetMessage::Ping).unwrap();
        let pong_json = serde_json::to_string(&WidgetMessage::Pong).unwrap();
        assert!(ping_json.contains("ping"));
        assert!(pong_json.contains("pong"));
    }

    #[test]
    fn test_widget_session_serde() {
        let s = WidgetSession::new("s1", "u1");
        let json = serde_json::to_string(&s).unwrap();
        let back: WidgetSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "s1");
        assert_eq!(back.user_id, "u1");
    }
}
