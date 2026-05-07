//! Per-user chat session management across channels.

use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use super::ChannelType;

/// A single chat message in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Who sent the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A chat session for a user on a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub channel_type: ChannelType,
    pub user_id: String,
    pub messages: VecDeque<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/// Configuration for session management.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub max_history: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { max_history: 50 }
    }
}

/// Manages per-user chat sessions across channels.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Session>>,
    config: SessionConfig,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Create a new session for a user on a channel.
    pub fn create_session(&self, channel_type: ChannelType, user_id: &str) -> Session {
        let now = Utc::now();
        let id = uuid::Uuid::new_v4().to_string();
        let session = Session {
            id: id.clone(),
            channel_type,
            user_id: user_id.to_string(),
            messages: VecDeque::new(),
            created_at: now,
            last_active: now,
        };
        self.sessions.insert(id, session.clone());
        session
    }

    /// Get a session by ID (returns a clone).
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    /// Add a message to a session.
    pub fn add_message(&self, session_id: &str, role: MessageRole, content: &str) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| FuseError::SessionNotFound(session_id.to_string()))?;

        let session = entry.value_mut();
        session.messages.push_back(ChatMessage {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        session.last_active = Utc::now();

        // Trim to max history
        while session.messages.len() > self.config.max_history {
            session.messages.pop_front();
        }

        Ok(())
    }

    /// Remove sessions that have been idle longer than `max_idle`.
    pub fn cleanup_expired(&self, max_idle: Duration) -> usize {
        let cutoff = Utc::now() - chrono::Duration::from_std(max_idle).unwrap_or_default();
        let mut removed = 0;
        self.sessions.retain(|_, session| {
            if session.last_active < cutoff {
                removed += 1;
                false
            } else {
                true
            }
        });
        removed
    }

    /// Number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> SessionManager {
        SessionManager::new(SessionConfig { max_history: 3 })
    }

    #[test]
    fn test_create_session() {
        let mgr = make_manager();
        let session = mgr.create_session(ChannelType::Telegram, "user1");
        assert_eq!(session.user_id, "user1");
        assert_eq!(session.channel_type, ChannelType::Telegram);
        assert!(session.messages.is_empty());
        assert_eq!(mgr.session_count(), 1);
    }

    #[test]
    fn test_get_session() {
        let mgr = make_manager();
        let session = mgr.create_session(ChannelType::Discord, "user2");
        let retrieved = mgr.get_session(&session.id);
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.as_ref().map(|s| &s.user_id),
            Some(&"user2".to_string())
        );

        assert!(mgr.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_add_message() {
        let mgr = make_manager();
        let session = mgr.create_session(ChannelType::Slack, "user3");

        mgr.add_message(&session.id, MessageRole::User, "hello")
            .expect("add_message failed");
        mgr.add_message(&session.id, MessageRole::Assistant, "hi there")
            .expect("add_message failed");

        let s = mgr.get_session(&session.id).expect("session not found");
        assert_eq!(s.messages.len(), 2);
        assert_eq!(s.messages[0].content, "hello");
        assert_eq!(s.messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_add_message_nonexistent_session() {
        let mgr = make_manager();
        let result = mgr.add_message("no-such-id", MessageRole::User, "hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_history_trimming() {
        let mgr = make_manager(); // max_history = 3
        let session = mgr.create_session(ChannelType::Matrix, "user4");

        for i in 0..5 {
            mgr.add_message(&session.id, MessageRole::User, &format!("msg{i}"))
                .expect("add failed");
        }

        let s = mgr.get_session(&session.id).expect("session not found");
        assert_eq!(s.messages.len(), 3);
        assert_eq!(s.messages[0].content, "msg2");
        assert_eq!(s.messages[2].content, "msg4");
    }

    #[test]
    fn test_cleanup_expired() {
        let mgr = make_manager();
        let session = mgr.create_session(ChannelType::Telegram, "user5");

        // Manually set last_active to the past
        if let Some(mut entry) = mgr.sessions.get_mut(&session.id) {
            entry.last_active = Utc::now() - chrono::Duration::hours(2);
        }

        // Create a fresh session that should survive
        let _fresh = mgr.create_session(ChannelType::Discord, "user6");

        assert_eq!(mgr.session_count(), 2);
        let removed = mgr.cleanup_expired(Duration::from_secs(3600)); // 1 hour
        assert_eq!(removed, 1);
        assert_eq!(mgr.session_count(), 1);
    }

    #[test]
    fn test_concurrent_access() {
        let mgr = make_manager();
        let mgr_clone = mgr.clone();

        // Simulate concurrent session creation
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let m = mgr_clone.clone();
                std::thread::spawn(move || {
                    m.create_session(ChannelType::Telegram, &format!("user{i}"))
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        assert_eq!(mgr.session_count(), 10);
    }
}
