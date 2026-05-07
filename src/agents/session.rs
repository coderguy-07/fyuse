//! Persistent session management [10.2]
//!
//! Session storage with resumption, forking, compaction, and token tracking.

use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single message in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: u32,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent_id: Option<String>,
    pub total_tokens: u64,
    pub message_count: usize,
    pub model: Option<String>,
}

/// A persistent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub meta: SessionMeta,
    pub messages: Vec<SessionMessage>,
}

impl Session {
    /// Create a new empty session.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let now = Utc::now();
        Self {
            meta: SessionMeta {
                id,
                created_at: now,
                updated_at: now,
                parent_id: None,
                total_tokens: 0,
                message_count: 0,
                model: None,
            },
            messages: Vec::new(),
        }
    }

    /// Append a message to the session.
    pub fn append(&mut self, role: MessageRole, content: String, token_count: u32) {
        let msg = SessionMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: Utc::now(),
            token_count,
            pinned: false,
        };
        self.meta.total_tokens += token_count as u64;
        self.meta.message_count += 1;
        self.meta.updated_at = Utc::now();
        self.messages.push(msg);
    }

    /// Fork this session into a new independent session.
    pub fn fork(&self, new_id: impl Into<String>) -> Self {
        let new_id = new_id.into();
        let now = Utc::now();
        Self {
            meta: SessionMeta {
                id: new_id,
                created_at: now,
                updated_at: now,
                parent_id: Some(self.meta.id.clone()),
                total_tokens: self.meta.total_tokens,
                message_count: self.meta.message_count,
                model: self.meta.model.clone(),
            },
            messages: self.messages.clone(),
        }
    }

    /// Compact the session by keeping only pinned messages and recent messages.
    /// Returns the number of messages removed.
    pub fn compact(&mut self, keep_recent: usize) -> usize {
        let original_count = self.messages.len();
        if original_count <= keep_recent {
            return 0;
        }

        let mut kept = Vec::new();

        // Keep system messages, pinned messages, and recent messages
        for (i, msg) in self.messages.iter().enumerate() {
            if msg.pinned || msg.role == MessageRole::System || i >= original_count - keep_recent {
                kept.push(msg.clone());
            }
        }

        // Deduplicate (pinned messages in the recent window)
        let mut seen = std::collections::HashSet::new();
        kept.retain(|m| seen.insert(m.id.clone()));

        let removed = original_count - kept.len();

        // Recalculate tokens
        self.meta.total_tokens = kept.iter().map(|m| m.token_count as u64).sum();
        self.meta.message_count = kept.len();
        self.messages = kept;

        removed
    }
}

/// File-based session store.
pub struct SessionStore {
    base_dir: PathBuf,
}

impl SessionStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let base_dir = base_dir.into();
        std::fs::create_dir_all(&base_dir)
            .map_err(|e| FuseError::AgentError(format!("Failed to create session dir: {e}")))?;
        Ok(Self { base_dir })
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{id}.json"))
    }

    /// Save a session to disk.
    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.meta.id);
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| FuseError::AgentError(format!("Failed to serialize session: {e}")))?;
        std::fs::write(&path, json)
            .map_err(|e| FuseError::AgentError(format!("Failed to write session: {e}")))?;
        Ok(())
    }

    /// Load a session from disk.
    pub fn load(&self, id: &str) -> Result<Session> {
        let path = self.session_path(id);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| FuseError::AgentError(format!("Session not found: {id}: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| FuseError::AgentError(format!("Failed to parse session: {e}")))
    }

    /// List all sessions.
    pub fn list(&self) -> Result<Vec<SessionMeta>> {
        let mut metas = Vec::new();
        let entries = std::fs::read_dir(&self.base_dir)
            .map_err(|e| FuseError::AgentError(format!("Failed to read session dir: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<Session>(&content) {
                        metas.push(session.meta);
                    }
                }
            }
        }

        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    /// Delete a session.
    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        std::fs::remove_file(&path)
            .map_err(|e| FuseError::AgentError(format!("Failed to delete session {id}: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_create_and_append() {
        let mut s = Session::new("s1");
        assert_eq!(s.meta.message_count, 0);
        assert_eq!(s.meta.total_tokens, 0);

        s.append(MessageRole::User, "Hello".into(), 5);
        assert_eq!(s.meta.message_count, 1);
        assert_eq!(s.meta.total_tokens, 5);

        s.append(MessageRole::Assistant, "Hi there!".into(), 10);
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.total_tokens, 15);
    }

    #[test]
    fn test_session_fork() {
        let mut s = Session::new("s1");
        s.append(MessageRole::User, "msg1".into(), 5);
        s.append(MessageRole::Assistant, "msg2".into(), 10);

        let forked = s.fork("s2");
        assert_eq!(forked.meta.id, "s2");
        assert_eq!(forked.meta.parent_id, Some("s1".into()));
        assert_eq!(forked.messages.len(), 2);
        assert_eq!(forked.meta.total_tokens, 15);

        // Fork is independent — modify original
        s.append(MessageRole::User, "msg3".into(), 3);
        assert_eq!(s.messages.len(), 3);
        assert_eq!(forked.messages.len(), 2); // Unchanged
    }

    #[test]
    fn test_session_compact() {
        let mut s = Session::new("s1");
        for i in 0..20 {
            s.append(MessageRole::User, format!("msg{i}"), 10);
        }
        assert_eq!(s.messages.len(), 20);

        let removed = s.compact(5);
        assert_eq!(removed, 15);
        assert_eq!(s.messages.len(), 5);
        assert_eq!(s.meta.total_tokens, 50);
    }

    #[test]
    fn test_compact_preserves_pinned() {
        let mut s = Session::new("s1");
        for i in 0..10 {
            s.append(MessageRole::User, format!("msg{i}"), 10);
        }
        s.messages[0].pinned = true; // Pin first message

        let removed = s.compact(3);
        assert!(removed > 0);
        assert!(s.messages.iter().any(|m| m.content == "msg0")); // Pinned kept
    }

    #[test]
    fn test_compact_preserves_system() {
        let mut s = Session::new("s1");
        s.append(MessageRole::System, "system prompt".into(), 20);
        for i in 0..10 {
            s.append(MessageRole::User, format!("msg{i}"), 10);
        }

        s.compact(2);
        assert!(s.messages.iter().any(|m| m.role == MessageRole::System));
    }

    #[test]
    fn test_compact_noop_when_small() {
        let mut s = Session::new("s1");
        s.append(MessageRole::User, "msg".into(), 5);
        let removed = s.compact(10);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_session_store_save_load() {
        let dir = std::env::temp_dir().join("fuse-test-sessions");
        let _ = std::fs::remove_dir_all(&dir);

        let store = SessionStore::new(&dir).unwrap();
        let mut s = Session::new("test-session");
        s.append(MessageRole::User, "Hello".into(), 5);

        store.save(&s).unwrap();
        let loaded = store.load("test-session").unwrap();

        assert_eq!(loaded.meta.id, "test-session");
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.meta.total_tokens, 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_store_list() {
        let dir = std::env::temp_dir().join("fuse-test-sessions-list");
        let _ = std::fs::remove_dir_all(&dir);

        let store = SessionStore::new(&dir).unwrap();
        store.save(&Session::new("s1")).unwrap();
        store.save(&Session::new("s2")).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_store_delete() {
        let dir = std::env::temp_dir().join("fuse-test-sessions-del");
        let _ = std::fs::remove_dir_all(&dir);

        let store = SessionStore::new(&dir).unwrap();
        store.save(&Session::new("s1")).unwrap();
        store.delete("s1").unwrap();
        assert!(store.load("s1").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut s = Session::new("s1");
        s.meta.model = Some("llama3:7b".into());
        s.append(MessageRole::User, "test".into(), 5);

        let json = serde_json::to_string(&s).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(back.meta.id, "s1");
        assert_eq!(back.meta.model, Some("llama3:7b".into()));
        assert_eq!(back.messages.len(), 1);
    }
}
