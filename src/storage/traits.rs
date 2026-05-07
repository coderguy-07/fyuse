//! StorageBackend trait — generic key-value persistence.

use crate::error::Result;
use async_trait::async_trait;

/// Core trait for data persistence backends.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLock;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct InMemoryStorage {
        data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    }

    impl InMemoryStorage {
        fn new() -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl StorageBackend for InMemoryStorage {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            Ok(self.data.read().get(key).cloned())
        }
        async fn put(&self, key: &str, value: &[u8]) -> Result<()> {
            self.data.write().insert(key.to_string(), value.to_vec());
            Ok(())
        }
        async fn delete(&self, key: &str) -> Result<()> {
            self.data.write().remove(key);
            Ok(())
        }
        async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
            Ok(self
                .data
                .read()
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }
    }

    #[tokio::test]
    async fn test_storage_crud() {
        let store = InMemoryStorage::new();

        // Put and get
        store.put("key1", b"value1").await.unwrap();
        let val = store.get("key1").await.unwrap();
        assert_eq!(val, Some(b"value1".to_vec()));

        // Missing key
        assert!(store.get("nonexistent").await.unwrap().is_none());

        // Delete
        store.delete("key1").await.unwrap();
        assert!(store.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_storage_list_prefix() {
        let store = InMemoryStorage::new();
        store.put("models/a", b"1").await.unwrap();
        store.put("models/b", b"2").await.unwrap();
        store.put("config/x", b"3").await.unwrap();

        let keys = store.list_prefix("models/").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|k| k.starts_with("models/")));
    }
}
