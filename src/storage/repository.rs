use crate::error::Result;
use crate::model::ModelMetadata;
use crate::storage::database::Database;
use crate::storage::download::DownloadState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Model repository for managing model metadata
pub struct ModelRepository {
    db: Arc<Database>,
}

impl ModelRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn save(&self, metadata: &ModelMetadata) -> Result<()> {
        self.db.put("models", &metadata.id, metadata)
    }

    pub fn get(&self, id: &str) -> Result<Option<ModelMetadata>> {
        self.db.get("models", id)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.db.delete("models", id)
    }

    pub fn list(&self) -> Result<Vec<ModelMetadata>> {
        let keys = self.db.list_keys("models")?;
        let mut models = Vec::new();

        for key in keys {
            if let Some(metadata) = self.get(&key)? {
                models.push(metadata);
            }
        }

        Ok(models)
    }

    pub fn exists(&self, id: &str) -> Result<bool> {
        Ok(self.get(id)?.is_some())
    }
}

/// Configuration repository
pub struct ConfigRepository {
    db: Arc<Database>,
}

impl ConfigRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn save<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.db.put("config", key, value)
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        self.db.get("config", key)
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.db.delete("config", key)
    }
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub model_name: Option<String>,
}

/// History repository for managing chat history
pub struct HistoryRepository {
    db: Arc<Database>,
}

impl HistoryRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn save(&self, message: &ChatMessage) -> Result<()> {
        self.db.put("history", &message.id, message)
    }

    pub fn get(&self, id: &str) -> Result<Option<ChatMessage>> {
        self.db.get("history", id)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.db.delete("history", id)
    }

    pub fn list(&self) -> Result<Vec<ChatMessage>> {
        let keys = self.db.list_keys("history")?;
        let mut messages = Vec::new();

        for key in keys {
            if let Some(message) = self.get(&key)? {
                messages.push(message);
            }
        }

        // Sort by timestamp
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(messages)
    }

    pub fn clear(&self) -> Result<()> {
        let keys = self.db.list_keys("history")?;
        for key in keys {
            self.delete(&key)?;
        }
        Ok(())
    }
}

/// Download state repository
pub struct DownloadStateRepository {
    db: Arc<Database>,
}

impl DownloadStateRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn save(&self, url: &str, state: &DownloadState) -> Result<()> {
        self.db.put("download_state", url, state)
    }

    pub fn get(&self, url: &str) -> Result<Option<DownloadState>> {
        self.db.get("download_state", url)
    }

    pub fn delete(&self, url: &str) -> Result<()> {
        self.db.delete("download_state", url)
    }

    pub fn list(&self) -> Result<Vec<(String, DownloadState)>> {
        let keys = self.db.list_keys("download_state")?;
        let mut states = Vec::new();

        for key in keys {
            if let Some(state) = self.get(&key)? {
                states.push((key, state));
            }
        }

        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_db() -> (Arc<Database>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = Arc::new(Database::new(db_path).unwrap());
        (db, temp_dir)
    }

    fn create_test_model() -> ModelMetadata {
        use crate::model::ModelSource;

        ModelMetadata::new(
            "test-model",
            "Test Model",
            ModelSource::huggingface("test/model"),
            "1.0.0",
            1024 * 1024,
        )
        .with_architecture("transformer")
        .with_parameter_count(7_000_000_000)
        .with_tag("test")
        .with_tag("model")
    }

    #[test]
    fn test_model_repository_save_and_get() {
        let (db, _temp_dir) = create_test_db();
        let repo = ModelRepository::new(db);

        let model = create_test_model();
        repo.save(&model).unwrap();

        let retrieved = repo.get(&model.id).unwrap();
        assert_eq!(retrieved, Some(model));
    }

    #[test]
    fn test_model_repository_delete() {
        let (db, _temp_dir) = create_test_db();
        let repo = ModelRepository::new(db);

        let model = create_test_model();
        repo.save(&model).unwrap();
        repo.delete(&model.id).unwrap();

        let retrieved = repo.get(&model.id).unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_model_repository_list() {
        let (db, _temp_dir) = create_test_db();
        let repo = ModelRepository::new(db);

        let mut model1 = create_test_model();
        model1.id = "model1".to_string();

        let mut model2 = create_test_model();
        model2.id = "model2".to_string();

        repo.save(&model1).unwrap();
        repo.save(&model2).unwrap();

        let models = repo.list().unwrap();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_model_repository_exists() {
        let (db, _temp_dir) = create_test_db();
        let repo = ModelRepository::new(db);

        let model = create_test_model();

        assert!(!repo.exists(&model.id).unwrap());

        repo.save(&model).unwrap();

        assert!(repo.exists(&model.id).unwrap());
    }

    #[test]
    fn test_config_repository() {
        let (db, _temp_dir) = create_test_db();
        let repo = ConfigRepository::new(db);

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestConfig {
            value: String,
        }

        let config = TestConfig {
            value: "test".to_string(),
        };

        repo.save("test_key", &config).unwrap();

        let retrieved: Option<TestConfig> = repo.get("test_key").unwrap();
        assert_eq!(retrieved, Some(config));
    }

    #[test]
    fn test_history_repository_save_and_get() {
        let (db, _temp_dir) = create_test_db();
        let repo = HistoryRepository::new(db);

        let message = ChatMessage {
            id: "msg1".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
            model_name: Some("gpt2".to_string()),
        };

        repo.save(&message).unwrap();

        let retrieved = repo.get(&message.id).unwrap();
        assert_eq!(retrieved, Some(message));
    }

    #[test]
    fn test_history_repository_list() {
        let (db, _temp_dir) = create_test_db();
        let repo = HistoryRepository::new(db);

        let message1 = ChatMessage {
            id: "msg1".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
            model_name: Some("gpt2".to_string()),
        };

        let message2 = ChatMessage {
            id: "msg2".to_string(),
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
            timestamp: Utc::now(),
            model_name: Some("gpt2".to_string()),
        };

        repo.save(&message1).unwrap();
        repo.save(&message2).unwrap();

        let messages = repo.list().unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_history_repository_clear() {
        let (db, _temp_dir) = create_test_db();
        let repo = HistoryRepository::new(db);

        let message = ChatMessage {
            id: "msg1".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
            model_name: Some("gpt2".to_string()),
        };

        repo.save(&message).unwrap();
        repo.clear().unwrap();

        let messages = repo.list().unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_download_state_repository() {
        let (db, _temp_dir) = create_test_db();
        let repo = DownloadStateRepository::new(db);

        let state = DownloadState::InProgress {
            bytes_downloaded: 1024,
            total_bytes: Some(2048),
            started_at: Utc::now(),
        };

        repo.save("https://example.com/model", &state).unwrap();

        let retrieved = repo.get("https://example.com/model").unwrap();
        assert_eq!(retrieved, Some(state));
    }

    #[test]
    fn test_download_state_repository_list() {
        let (db, _temp_dir) = create_test_db();
        let repo = DownloadStateRepository::new(db);

        let state1 = DownloadState::Pending;
        let state2 = DownloadState::Completed {
            bytes_downloaded: 2048,
            completed_at: Utc::now(),
        };

        repo.save("url1", &state1).unwrap();
        repo.save("url2", &state2).unwrap();

        let states = repo.list().unwrap();
        assert_eq!(states.len(), 2);
    }
}
