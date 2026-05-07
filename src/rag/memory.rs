//! Conversation memory with vector search [9.2]
//!
//! Stores conversation history and retrieves relevant context using
//! embedding-based similarity search. Integrates with the RAG system
//! for context-aware responses.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A memory entry — a stored conversation turn with embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, String>,
}

impl MemoryEntry {
    pub fn new(
        session_id: impl Into<String>,
        role: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
            embedding: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }
}

/// Configuration for conversation memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum entries per session.
    pub max_entries_per_session: usize,
    /// Embedding dimension.
    pub embedding_dim: usize,
    /// Similarity threshold for retrieval (0.0 - 1.0).
    pub similarity_threshold: f64,
    /// Maximum results returned by search.
    pub max_results: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_entries_per_session: 500,
            embedding_dim: 384,
            similarity_threshold: 0.5,
            max_results: 10,
        }
    }
}

/// A search result from memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    pub similarity: f64,
}

/// Conversation memory store with vector similarity search.
pub struct ConversationMemory {
    config: MemoryConfig,
    entries: Vec<MemoryEntry>,
}

impl ConversationMemory {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// Add a memory entry.
    pub fn add(&mut self, entry: MemoryEntry) -> Result<()> {
        // Enforce per-session limit
        let session_count = self
            .entries
            .iter()
            .filter(|e| e.session_id == entry.session_id)
            .count();

        if session_count >= self.config.max_entries_per_session {
            // Evict oldest entry for this session
            if let Some(pos) = self
                .entries
                .iter()
                .position(|e| e.session_id == entry.session_id)
            {
                self.entries.remove(pos);
            }
        }

        self.entries.push(entry);
        Ok(())
    }

    /// Search memory by embedding similarity.
    pub fn search(
        &self,
        query_embedding: &[f32],
        session_id: Option<&str>,
    ) -> Vec<MemorySearchResult> {
        let mut results: Vec<MemorySearchResult> = self
            .entries
            .iter()
            .filter(|e| {
                // Filter by session if specified
                session_id.map_or(true, |sid| e.session_id == sid)
            })
            .filter_map(|e| {
                e.embedding.as_ref().map(|emb| {
                    let sim = cosine_similarity(query_embedding, emb);
                    MemorySearchResult {
                        entry: e.clone(),
                        similarity: sim,
                    }
                })
            })
            .filter(|r| r.similarity >= self.config.similarity_threshold)
            .collect();

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.max_results);
        results
    }

    /// Get all entries for a session, ordered by time.
    pub fn get_session_history(&self, session_id: &str) -> Vec<&MemoryEntry> {
        let mut entries: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect();
        entries.sort_by_key(|e| e.timestamp);
        entries
    }

    /// Get recent entries for context window.
    pub fn get_recent(&self, session_id: &str, n: usize) -> Vec<&MemoryEntry> {
        let mut history = self.get_session_history(session_id);
        let start = history.len().saturating_sub(n);
        history.split_off(start)
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the memory is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries for a session.
    pub fn clear_session(&mut self, session_id: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.session_id != session_id);
        before - self.entries.len()
    }

    /// Clear all entries.
    pub fn clear_all(&mut self) {
        self.entries.clear();
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_memory() -> ConversationMemory {
        ConversationMemory::new(MemoryConfig::default())
    }

    fn entry_with_embedding(session: &str, content: &str, emb: Vec<f32>) -> MemoryEntry {
        MemoryEntry::new(session, "user", content).with_embedding(emb)
    }

    #[test]
    fn test_memory_add_and_len() {
        let mut mem = make_memory();
        assert!(mem.is_empty());
        mem.add(MemoryEntry::new("s1", "user", "hello")).unwrap();
        assert_eq!(mem.len(), 1);
        assert!(!mem.is_empty());
    }

    #[test]
    fn test_session_history() {
        let mut mem = make_memory();
        mem.add(MemoryEntry::new("s1", "user", "msg1")).unwrap();
        mem.add(MemoryEntry::new("s1", "assistant", "msg2"))
            .unwrap();
        mem.add(MemoryEntry::new("s2", "user", "other")).unwrap();

        let history = mem.get_session_history("s1");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "msg1");
    }

    #[test]
    fn test_get_recent() {
        let mut mem = make_memory();
        for i in 0..10 {
            mem.add(MemoryEntry::new("s1", "user", format!("msg{i}")))
                .unwrap();
        }
        let recent = mem.get_recent("s1", 3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "msg7");
        assert_eq!(recent[2].content, "msg9");
    }

    #[test]
    fn test_clear_session() {
        let mut mem = make_memory();
        mem.add(MemoryEntry::new("s1", "user", "a")).unwrap();
        mem.add(MemoryEntry::new("s1", "user", "b")).unwrap();
        mem.add(MemoryEntry::new("s2", "user", "c")).unwrap();

        let removed = mem.clear_session("s1");
        assert_eq!(removed, 2);
        assert_eq!(mem.len(), 1);
    }

    #[test]
    fn test_clear_all() {
        let mut mem = make_memory();
        mem.add(MemoryEntry::new("s1", "user", "a")).unwrap();
        mem.add(MemoryEntry::new("s2", "user", "b")).unwrap();
        mem.clear_all();
        assert!(mem.is_empty());
    }

    #[test]
    fn test_max_entries_eviction() {
        let mut mem = ConversationMemory::new(MemoryConfig {
            max_entries_per_session: 3,
            ..Default::default()
        });

        for i in 0..5 {
            mem.add(MemoryEntry::new("s1", "user", format!("msg{i}")))
                .unwrap();
        }

        let history = mem.get_session_history("s1");
        assert_eq!(history.len(), 3);
        // Oldest should have been evicted
        assert_eq!(history[0].content, "msg2");
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn test_search_by_embedding() {
        let mut mem = make_memory();
        mem.add(entry_with_embedding(
            "s1",
            "about cats",
            vec![1.0, 0.0, 0.0],
        ))
        .unwrap();
        mem.add(entry_with_embedding(
            "s1",
            "about dogs",
            vec![0.9, 0.1, 0.0],
        ))
        .unwrap();
        mem.add(entry_with_embedding(
            "s1",
            "about math",
            vec![0.0, 0.0, 1.0],
        ))
        .unwrap();

        // Query similar to "cats" and "dogs"
        let results = mem.search(&[1.0, 0.0, 0.0], None);
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.content, "about cats");
        assert!(results[0].similarity > 0.9);
    }

    #[test]
    fn test_search_filtered_by_session() {
        let mut mem = make_memory();
        mem.add(entry_with_embedding("s1", "msg1", vec![1.0, 0.0]))
            .unwrap();
        mem.add(entry_with_embedding("s2", "msg2", vec![1.0, 0.0]))
            .unwrap();

        let results = mem.search(&[1.0, 0.0], Some("s1"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.session_id, "s1");
    }

    #[test]
    fn test_search_threshold() {
        let mut mem = ConversationMemory::new(MemoryConfig {
            similarity_threshold: 0.9,
            ..Default::default()
        });
        mem.add(entry_with_embedding("s1", "close", vec![1.0, 0.0]))
            .unwrap();
        mem.add(entry_with_embedding("s1", "far", vec![0.0, 1.0]))
            .unwrap();

        let results = mem.search(&[1.0, 0.0], None);
        assert_eq!(results.len(), 1); // Only "close" above 0.9 threshold
    }

    #[test]
    fn test_search_max_results() {
        let mut mem = ConversationMemory::new(MemoryConfig {
            max_results: 2,
            similarity_threshold: 0.0,
            ..Default::default()
        });
        for i in 0..10 {
            mem.add(entry_with_embedding(
                "s1",
                &format!("msg{i}"),
                vec![1.0, 0.0],
            ))
            .unwrap();
        }
        let results = mem.search(&[1.0, 0.0], None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_memory_entry_with_metadata() {
        let mut entry = MemoryEntry::new("s1", "user", "hello");
        entry.metadata.insert("model".into(), "llama3".into());
        assert_eq!(entry.metadata.get("model").unwrap(), "llama3");
    }

    #[test]
    fn test_memory_config_default() {
        let config = MemoryConfig::default();
        assert_eq!(config.max_entries_per_session, 500);
        assert_eq!(config.embedding_dim, 384);
        assert!((config.similarity_threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.max_results, 10);
    }

    #[test]
    fn test_serde_roundtrip() {
        let entry = MemoryEntry::new("s1", "user", "hello").with_embedding(vec![1.0, 2.0, 3.0]);
        let json = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "hello");
        assert_eq!(back.embedding.unwrap(), vec![1.0, 2.0, 3.0]);
    }
}
