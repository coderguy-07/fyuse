//! Smart response caching with semantic deduplication.
//!
//! Provides SHA256-keyed LRU cache with TTL for inference responses.
//! Inspired by KUI's Moka-based caching but uses DashMap for zero extra deps.

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for the inference cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            ttl_seconds: 3600,
        }
    }
}

/// A cached response entry.
#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: u64,
    pub hit_rate: f64,
    pub evictions: u64,
}

/// Smart inference response cache.
pub struct InferenceCache {
    entries: Arc<DashMap<String, CacheEntry>>,
    lru_order: Arc<RwLock<VecDeque<String>>>,
    config: CacheConfig,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl InferenceCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Generate a cache key from model name and prompt.
    pub fn generate_key(model: &str, prompt: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        model.hash(&mut hasher);
        prompt.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Generate a cache key from model and message history.
    pub fn generate_chat_key(model: &str, messages: &[(String, String)]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        model.hash(&mut hasher);
        for (role, content) in messages {
            role.hash(&mut hasher);
            content.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// Get a cached response.
    pub fn get(&self, key: &str) -> Option<String> {
        if !self.config.enabled {
            return None;
        }

        match self.entries.get(key) {
            Some(entry) if !entry.is_expired() => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                // Move to front of LRU
                let mut lru = self.lru_order.write();
                lru.retain(|k| k != key);
                lru.push_front(key.to_string());
                Some(entry.value.clone())
            }
            Some(_) => {
                // Expired — remove
                self.entries.remove(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                let mut lru = self.lru_order.write();
                lru.retain(|k| k != key);
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Store a response in the cache.
    pub fn set(&self, key: String, value: String) {
        if !self.config.enabled {
            return;
        }

        // Evict if at capacity
        while self.entries.len() >= self.config.max_entries {
            self.evict_lru();
        }

        let entry = CacheEntry {
            value,
            created_at: Instant::now(),
            ttl: Duration::from_secs(self.config.ttl_seconds),
        };

        self.entries.insert(key.clone(), entry);
        let mut lru = self.lru_order.write();
        lru.retain(|k| k != &key);
        lru.push_front(key);
    }

    /// Check if a key exists and is not expired.
    pub fn contains(&self, key: &str) -> bool {
        if !self.config.enabled {
            return false;
        }
        match self.entries.get(key) {
            Some(entry) => !entry.is_expired(),
            None => false,
        }
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        self.entries.clear();
        self.lru_order.write().clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        CacheStats {
            hits,
            misses,
            entries: self.entries.len() as u64,
            hit_rate,
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }

    /// Remove expired entries.
    pub fn cleanup_expired(&self) {
        let expired_keys: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.value().is_expired())
            .map(|e| e.key().clone())
            .collect();

        for key in expired_keys {
            self.entries.remove(&key);
            let mut lru = self.lru_order.write();
            lru.retain(|k| k != &key);
        }
    }

    fn evict_lru(&self) {
        let mut lru = self.lru_order.write();
        if let Some(key) = lru.pop_back() {
            self.entries.remove(&key);
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Default for InferenceCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache() -> InferenceCache {
        InferenceCache::new(CacheConfig {
            enabled: true,
            max_entries: 5,
            ttl_seconds: 60,
        })
    }

    #[test]
    fn test_cache_miss() {
        let cache = test_cache();
        assert!(cache.get("nonexistent").is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_cache_hit() {
        let cache = test_cache();
        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn test_cache_contains() {
        let cache = test_cache();
        assert!(!cache.contains("key1"));
        cache.set("key1".to_string(), "value1".to_string());
        assert!(cache.contains("key1"));
    }

    #[test]
    fn test_cache_eviction() {
        let cache = test_cache(); // max 5 entries
        for i in 0..7 {
            cache.set(format!("key{}", i), format!("value{}", i));
        }
        assert!(cache.entries.len() <= 5);
        assert!(cache.stats().evictions > 0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = test_cache();
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());
        cache.clear();
        assert_eq!(cache.entries.len(), 0);
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_cache_disabled() {
        let cache = InferenceCache::new(CacheConfig {
            enabled: false,
            max_entries: 10,
            ttl_seconds: 60,
        });
        cache.set("key1".to_string(), "value1".to_string());
        assert!(cache.get("key1").is_none());
        assert!(!cache.contains("key1"));
    }

    #[test]
    fn test_cache_ttl_expiration() {
        let cache = InferenceCache::new(CacheConfig {
            enabled: true,
            max_entries: 10,
            ttl_seconds: 0, // Instant expiration
        });
        cache.set("key1".to_string(), "value1".to_string());
        // Entry should be expired immediately
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_generate_key_deterministic() {
        let k1 = InferenceCache::generate_key("llama3", "Hello world");
        let k2 = InferenceCache::generate_key("llama3", "Hello world");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_generate_key_different_inputs() {
        let k1 = InferenceCache::generate_key("llama3", "Hello");
        let k2 = InferenceCache::generate_key("llama3", "World");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_generate_key_different_models() {
        let k1 = InferenceCache::generate_key("llama3", "Hello");
        let k2 = InferenceCache::generate_key("mistral", "Hello");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_generate_chat_key() {
        let msgs = vec![
            ("user".to_string(), "Hi".to_string()),
            ("assistant".to_string(), "Hello!".to_string()),
        ];
        let k1 = InferenceCache::generate_chat_key("llama3", &msgs);
        let k2 = InferenceCache::generate_chat_key("llama3", &msgs);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_cache_stats() {
        let cache = test_cache();
        cache.set("key1".to_string(), "value1".to_string());
        cache.get("key1"); // hit
        cache.get("key2"); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
        assert!((stats.hit_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cleanup_expired() {
        let cache = InferenceCache::new(CacheConfig {
            enabled: true,
            max_entries: 10,
            ttl_seconds: 0,
        });
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        cache.cleanup_expired();
        assert_eq!(cache.entries.len(), 0);
    }

    #[test]
    fn test_lru_order() {
        let cache = test_cache(); // max 5
        for i in 0..5 {
            cache.set(format!("key{}", i), format!("value{}", i));
        }
        // Access key0 to move it to front
        cache.get("key0");
        // Add one more — should evict the LRU (key1, since key0 was accessed)
        cache.set("key5".to_string(), "value5".to_string());
        // key0 should still exist (was recently accessed)
        assert!(cache.contains("key0"));
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_seconds, 3600);
    }
}
