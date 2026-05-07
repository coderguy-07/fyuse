use crate::error::{FuseError, Result};
use redb::{Database as RedbDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Table definitions
const MODELS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("models");
const CONFIG_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("config");
const HISTORY_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("history");
const FEEDBACK_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("feedback");
const DOWNLOAD_STATE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("download_state");

/// Database wrapper for Redb
pub struct Database {
    db: RedbDatabase,
    path: PathBuf,
}

impl Database {
    /// Create a new database instance
    pub fn new(path: PathBuf) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = RedbDatabase::create(&path)
            .map_err(|e| FuseError::DatabaseError(format!("Failed to create database: {}", e)))?;

        // Initialize tables
        let write_txn = db.begin_write().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to begin write transaction: {}", e))
        })?;

        {
            let _ = write_txn.open_table(MODELS_TABLE).map_err(|e| {
                FuseError::DatabaseError(format!("Failed to open models table: {}", e))
            })?;
            let _ = write_txn.open_table(CONFIG_TABLE).map_err(|e| {
                FuseError::DatabaseError(format!("Failed to open config table: {}", e))
            })?;
            let _ = write_txn.open_table(HISTORY_TABLE).map_err(|e| {
                FuseError::DatabaseError(format!("Failed to open history table: {}", e))
            })?;
            let _ = write_txn.open_table(FEEDBACK_TABLE).map_err(|e| {
                FuseError::DatabaseError(format!("Failed to open feedback table: {}", e))
            })?;
            let _ = write_txn.open_table(DOWNLOAD_STATE_TABLE).map_err(|e| {
                FuseError::DatabaseError(format!("Failed to open download_state table: {}", e))
            })?;
        }

        write_txn.commit().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to commit transaction: {}", e))
        })?;

        Ok(Self { db, path })
    }

    /// Get database path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Store a value in a table
    pub fn put<T: Serialize>(&self, table: &str, key: &str, value: &T) -> Result<()> {
        let serialized = serde_json::to_vec(value).map_err(|e| {
            FuseError::SerializationError(format!("Failed to serialize value: {}", e))
        })?;

        let write_txn = self.db.begin_write().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to begin write transaction: {}", e))
        })?;

        {
            let table_def = Self::get_table_definition(table)?;
            let mut table_handle = write_txn
                .open_table(table_def)
                .map_err(|e| FuseError::DatabaseError(format!("Failed to open table: {}", e)))?;

            table_handle
                .insert(key, serialized.as_slice())
                .map_err(|e| FuseError::DatabaseError(format!("Failed to insert value: {}", e)))?;
        }

        write_txn.commit().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// Get a value from a table
    pub fn get<T: for<'de> Deserialize<'de>>(&self, table: &str, key: &str) -> Result<Option<T>> {
        let read_txn = self.db.begin_read().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to begin read transaction: {}", e))
        })?;

        let table_def = Self::get_table_definition(table)?;
        let table_handle = read_txn
            .open_table(table_def)
            .map_err(|e| FuseError::DatabaseError(format!("Failed to open table: {}", e)))?;

        let value = table_handle
            .get(key)
            .map_err(|e| FuseError::DatabaseError(format!("Failed to get value: {}", e)))?;

        match value {
            Some(bytes) => {
                let deserialized = serde_json::from_slice(bytes.value()).map_err(|e| {
                    FuseError::SerializationError(format!("Failed to deserialize value: {}", e))
                })?;
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }

    /// Delete a value from a table
    pub fn delete(&self, table: &str, key: &str) -> Result<()> {
        let write_txn = self.db.begin_write().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to begin write transaction: {}", e))
        })?;

        {
            let table_def = Self::get_table_definition(table)?;
            let mut table_handle = write_txn
                .open_table(table_def)
                .map_err(|e| FuseError::DatabaseError(format!("Failed to open table: {}", e)))?;

            table_handle
                .remove(key)
                .map_err(|e| FuseError::DatabaseError(format!("Failed to delete value: {}", e)))?;
        }

        write_txn.commit().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// List all keys in a table
    pub fn list_keys(&self, table: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read().map_err(|e| {
            FuseError::DatabaseError(format!("Failed to begin read transaction: {}", e))
        })?;

        let table_def = Self::get_table_definition(table)?;
        let table_handle = read_txn
            .open_table(table_def)
            .map_err(|e| FuseError::DatabaseError(format!("Failed to open table: {}", e)))?;

        let mut keys = Vec::new();
        let iter = table_handle
            .iter()
            .map_err(|e| FuseError::DatabaseError(format!("Failed to iterate table: {}", e)))?;

        for item in iter {
            let (key, _) =
                item.map_err(|e| FuseError::DatabaseError(format!("Failed to read item: {}", e)))?;
            keys.push(key.value().to_string());
        }

        Ok(keys)
    }

    /// Get table definition by name
    fn get_table_definition(
        table: &str,
    ) -> Result<TableDefinition<'static, &'static str, &'static [u8]>> {
        match table {
            "models" => Ok(MODELS_TABLE),
            "config" => Ok(CONFIG_TABLE),
            "history" => Ok(HISTORY_TABLE),
            "feedback" => Ok(FEEDBACK_TABLE),
            "download_state" => Ok(DOWNLOAD_STATE_TABLE),
            _ => Err(FuseError::DatabaseError(format!(
                "Unknown table: {}",
                table
            ))),
        }
    }

    /// Check if database is healthy
    pub fn health_check(&self) -> Result<()> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| FuseError::DatabaseError(format!("Health check failed: {}", e)))?;

        // Try to open all tables
        let _ = read_txn.open_table(MODELS_TABLE).map_err(|e| {
            FuseError::DatabaseError(format!("Models table health check failed: {}", e))
        })?;
        let _ = read_txn.open_table(CONFIG_TABLE).map_err(|e| {
            FuseError::DatabaseError(format!("Config table health check failed: {}", e))
        })?;
        let _ = read_txn.open_table(HISTORY_TABLE).map_err(|e| {
            FuseError::DatabaseError(format!("History table health check failed: {}", e))
        })?;
        let _ = read_txn.open_table(FEEDBACK_TABLE).map_err(|e| {
            FuseError::DatabaseError(format!("Feedback table health check failed: {}", e))
        })?;
        let _ = read_txn.open_table(DOWNLOAD_STATE_TABLE).map_err(|e| {
            FuseError::DatabaseError(format!("Download state table health check failed: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    fn create_test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = Database::new(db_path).unwrap();
        (db, temp_dir)
    }

    #[test]
    fn test_database_creation() {
        let (db, _temp_dir) = create_test_db();
        assert!(db.path().exists());
    }

    #[test]
    fn test_database_health_check() {
        let (db, _temp_dir) = create_test_db();
        assert!(db.health_check().is_ok());
    }

    #[test]
    fn test_put_and_get() {
        let (db, _temp_dir) = create_test_db();

        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        db.put("models", "test_key", &test_data).unwrap();

        let retrieved: Option<TestData> = db.get("models", "test_key").unwrap();
        assert_eq!(retrieved, Some(test_data));
    }

    #[test]
    fn test_get_nonexistent() {
        let (db, _temp_dir) = create_test_db();

        let retrieved: Option<TestData> = db.get("models", "nonexistent").unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_delete() {
        let (db, _temp_dir) = create_test_db();

        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        db.put("models", "test_key", &test_data).unwrap();
        db.delete("models", "test_key").unwrap();

        let retrieved: Option<TestData> = db.get("models", "test_key").unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_list_keys() {
        let (db, _temp_dir) = create_test_db();

        let test_data1 = TestData {
            name: "test1".to_string(),
            value: 1,
        };
        let test_data2 = TestData {
            name: "test2".to_string(),
            value: 2,
        };
        let test_data3 = TestData {
            name: "test3".to_string(),
            value: 3,
        };

        db.put("models", "key1", &test_data1).unwrap();
        db.put("models", "key2", &test_data2).unwrap();
        db.put("models", "key3", &test_data3).unwrap();

        let keys = db.list_keys("models").unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[test]
    fn test_multiple_tables() {
        let (db, _temp_dir) = create_test_db();

        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        db.put("models", "key1", &test_data).unwrap();
        db.put("config", "key2", &test_data).unwrap();
        db.put("history", "key3", &test_data).unwrap();

        let retrieved1: Option<TestData> = db.get("models", "key1").unwrap();
        let retrieved2: Option<TestData> = db.get("config", "key2").unwrap();
        let retrieved3: Option<TestData> = db.get("history", "key3").unwrap();

        assert_eq!(retrieved1, Some(test_data.clone()));
        assert_eq!(retrieved2, Some(test_data.clone()));
        assert_eq!(retrieved3, Some(test_data));
    }

    #[test]
    fn test_invalid_table() {
        let (db, _temp_dir) = create_test_db();

        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let result = db.put("invalid_table", "key", &test_data);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown table"));
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let (db, _temp_dir) = create_test_db();
        let db = Arc::new(db);

        let mut handles = vec![];

        for i in 0..10 {
            let db_clone = Arc::clone(&db);
            let handle = thread::spawn(move || {
                let test_data = TestData {
                    name: format!("test{}", i),
                    value: i,
                };
                db_clone
                    .put("models", &format!("key{}", i), &test_data)
                    .unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let keys = db.list_keys("models").unwrap();
        assert_eq!(keys.len(), 10);
    }
}
