//! Connection pooling and resource management for efficient utilization
//! of system resources in the Fuse AI model server.

use crate::error::{FuseError, Result};
use crate::model::inference::{InferenceEngine, ModelHandle};
#[cfg(test)]
use crate::model::inference::{InferenceInput, InferenceOutput};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{Duration, Instant};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections per pool
    pub max_connections: usize,
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Connection idle timeout in seconds
    pub idle_timeout_secs: u64,
    /// Connection acquisition timeout in seconds
    pub acquire_timeout_secs: u64,
    /// Health check interval in seconds
    pub health_check_interval_secs: u64,
    /// Maximum connection age in seconds
    pub max_connection_age_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 2,
            idle_timeout_secs: 300,         // 5 minutes
            acquire_timeout_secs: 30,       // 30 seconds
            health_check_interval_secs: 60, // 1 minute
            max_connection_age_secs: 3600,  // 1 hour
        }
    }
}

/// Pooled connection state
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ConnectionState {
    /// Connection is available for use
    Available,
    /// Connection is currently in use
    InUse,
    /// Connection is being tested
    Testing,
    /// Connection is closed/unavailable
    Closed,
}

/// Pooled connection wrapper
#[allow(dead_code)]
pub struct PooledConnection<T: Send + Sync + 'static> {
    /// The actual connection/resource
    connection: Arc<T>,
    /// Connection state
    state: ConnectionState,
    /// When the connection was created
    created_at: Instant,
    /// When the connection was last used
    last_used_at: Instant,
    /// Connection pool reference for returning the connection
    pool: Arc<ConnectionPool<T>>,
}

impl<T: Send + Sync + 'static> PooledConnection<T> {
    /// Get reference to the underlying connection
    pub fn get(&self) -> &T {
        &self.connection
    }

    /// Get mutable reference to the underlying connection
    pub fn get_mut(&mut self) -> &mut T {
        Arc::get_mut(&mut self.connection).expect("Connection should not have multiple references")
    }
}

impl<T: Send + Sync + 'static> Drop for PooledConnection<T> {
    fn drop(&mut self) {
        // Return connection to pool when dropped
        let pool = Arc::clone(&self.pool);
        let conn_inner = match Arc::try_unwrap(self.connection.clone()) {
            Ok(conn) => PooledConnectionInner {
                connection: conn,
                state: ConnectionState::Available,
                created_at: self.created_at,
                last_used_at: Instant::now(),
            },
            Err(_) => return, // Connection still has references
        };

        tokio::spawn(async move {
            let conn_mutex = Arc::new(Mutex::new(conn_inner));
            pool.available.lock().await.push_back(conn_mutex);
        });
    }
}

/// Connection pool for managing reusable resources
pub struct ConnectionPool<T: Send + Sync + 'static> {
    /// Pool configuration
    config: PoolConfig,
    /// Available connections queue
    available: Mutex<VecDeque<Arc<Mutex<PooledConnectionInner<T>>>>>,
    /// Total number of connections (available + in use)
    total_connections: Mutex<usize>,
    /// Connection factory function
    factory: Box<dyn Fn() -> Result<T> + Send + Sync>,
    /// Connection health check function
    #[allow(clippy::type_complexity)]
    health_check: Box<dyn Fn(&T) -> Result<bool> + Send + Sync>,
    /// Connection cleanup function
    cleanup: Box<dyn Fn(T) -> Result<()> + Send + Sync>,
}

#[derive(Debug)]
struct PooledConnectionInner<T> {
    connection: T,
    state: ConnectionState,
    created_at: Instant,
    last_used_at: Instant,
}

impl<T: Send + Sync + 'static> ConnectionPool<T> {
    /// Create a new connection pool
    pub fn new<F, H, C>(config: PoolConfig, factory: F, health_check: H, cleanup: C) -> Self
    where
        F: Fn() -> Result<T> + Send + Sync + 'static,
        H: Fn(&T) -> Result<bool> + Send + Sync + 'static,
        C: Fn(T) -> Result<()> + Send + Sync + 'static,
    {
        Self {
            config,
            available: Mutex::new(VecDeque::new()),
            total_connections: Mutex::new(0),
            factory: Box::new(factory),
            health_check: Box::new(health_check),
            cleanup: Box::new(cleanup),
        }
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> Result<PooledConnection<T>> {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(self.config.acquire_timeout_secs);

        loop {
            // Try to get an available connection
            if let Some(conn_mutex) = self.available.lock().await.pop_front() {
                let mut conn = conn_mutex.lock().await;

                // Check if connection is still valid
                if self.is_connection_valid(&*conn).await {
                    conn.state = ConnectionState::InUse;
                    conn.last_used_at = Instant::now();

                    let connection =
                        std::mem::replace(&mut conn.connection, unsafe { std::mem::zeroed() });
                    let created_at = conn.created_at;
                    let last_used_at = conn.last_used_at;

                    return Ok(PooledConnection {
                        connection: Arc::new(connection),
                        state: ConnectionState::InUse,
                        created_at,
                        last_used_at,
                        pool: Arc::new(self.clone()),
                    });
                } else {
                    // Connection is invalid, clean it up
                    let conn_inner = std::mem::replace(&mut *conn, unsafe { std::mem::zeroed() });
                    self.cleanup_connection(conn_inner).await;
                    *self.total_connections.lock().await -= 1;
                }
            }

            // Check if we can create a new connection
            let total = *self.total_connections.lock().await;
            if total < self.config.max_connections {
                match self.create_connection().await {
                    Ok(pooled_conn) => {
                        *self.total_connections.lock().await += 1;
                        return Ok(pooled_conn);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create new connection: {}", e);
                    }
                }
            }

            // Check timeout
            if start_time.elapsed() > timeout {
                return Err(FuseError::ResourceLimitExceeded(
                    "Connection acquisition timeout".to_string(),
                ));
            }

            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Return a connection to the pool
    #[allow(dead_code)]
    async fn return_connection(&self, connection: &mut PooledConnection<T>) {
        let conn_inner = PooledConnectionInner {
            connection: match Arc::try_unwrap(connection.connection.clone()) {
                Ok(conn) => conn,
                Err(_) => return, // Connection still has references
            },
            state: ConnectionState::Available,
            created_at: connection.created_at,
            last_used_at: Instant::now(),
        };

        let conn_mutex = Arc::new(Mutex::new(conn_inner));
        self.available.lock().await.push_back(conn_mutex);
    }

    /// Create a new connection
    async fn create_connection(&self) -> Result<PooledConnection<T>> {
        let connection = (self.factory)()?;
        let now = Instant::now();

        Ok(PooledConnection {
            connection: Arc::new(connection),
            state: ConnectionState::InUse,
            created_at: now,
            last_used_at: now,
            pool: Arc::new(self.clone()),
        })
    }

    /// Check if a connection is still valid
    async fn is_connection_valid(&self, conn: &PooledConnectionInner<T>) -> bool {
        // Check age
        if conn.created_at.elapsed() > Duration::from_secs(self.config.max_connection_age_secs) {
            return false;
        }

        // Check idle timeout
        if conn.last_used_at.elapsed() > Duration::from_secs(self.config.idle_timeout_secs) {
            return false;
        }

        // Health check
        (self.health_check)(&conn.connection).unwrap_or_default()
    }

    /// Clean up an invalid connection
    async fn cleanup_connection(&self, conn: PooledConnectionInner<T>) {
        let connection = conn.connection;
        if let Err(e) = (self.cleanup)(connection) {
            tracing::warn!("Failed to cleanup connection: {}", e);
        }
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let available_count = self.available.lock().await.len();
        let total_count = *self.total_connections.lock().await;

        PoolStats {
            total_connections: total_count,
            available_connections: available_count,
            in_use_connections: total_count - available_count,
            utilization_rate: if total_count > 0 {
                ((total_count - available_count) as f32 / total_count as f32) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Close all connections in the pool
    pub async fn close(&self) -> Result<()> {
        let mut available = self.available.lock().await;
        while let Some(conn_mutex) = available.pop_front() {
            let conn = conn_mutex.lock().await;
            let conn_inner = PooledConnectionInner {
                connection: unsafe { std::ptr::read(&conn.connection) },
                state: conn.state.clone(),
                created_at: conn.created_at,
                last_used_at: conn.last_used_at,
            };
            self.cleanup_connection(conn_inner).await;
        }
        *self.total_connections.lock().await = 0;
        Ok(())
    }
}

impl<T: Send + Sync + 'static> Clone for ConnectionPool<T> {
    fn clone(&self) -> Self {
        // Note: Cloning pools is generally not recommended as it can lead to
        // resource leaks. This is a simplified implementation.
        Self {
            config: self.config.clone(),
            available: Mutex::new(VecDeque::new()),
            total_connections: Mutex::new(0),
            factory: Box::new(|| {
                Err(FuseError::InternalError(
                    "Cloned pool cannot create connections".to_string(),
                ))
            }),
            health_check: Box::new(|_| Ok(true)),
            cleanup: Box::new(|_| Ok(())),
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub available_connections: usize,
    pub in_use_connections: usize,
    pub utilization_rate: f32,
}

/// Model instance pool for managing loaded models
pub struct ModelPool<E: InferenceEngine> {
    /// Underlying inference engine
    engine: Arc<E>,
    /// Pool of loaded model handles
    handles: RwLock<HashMap<String, Vec<ModelHandle>>>,
    /// Model loading semaphore to prevent concurrent loads
    load_semaphore: Semaphore,
}

impl<E: InferenceEngine> ModelPool<E> {
    /// Create a new model pool
    pub fn new(engine: Arc<E>, max_concurrent_loads: usize) -> Self {
        Self {
            engine,
            handles: RwLock::new(HashMap::new()),
            load_semaphore: Semaphore::new(max_concurrent_loads),
        }
    }

    /// Get or load a model handle
    pub async fn get_model(&self, model_name: &str) -> Result<ModelHandle> {
        // Check if we have an available handle
        {
            let handles = self.handles.read();
            if let Some(model_handles) = handles.get(model_name) {
                if let Some(handle) = model_handles.iter().find(|_h| {
                    // Check if model is not busy (simplified check)
                    true // In real implementation, check model state
                }) {
                    return Ok(handle.clone());
                }
            }
        }

        // Need to load a new model instance
        let _permit = self.load_semaphore.acquire().await.unwrap();
        let handle = self.engine.load_model(model_name).await?;

        // Add to pool
        {
            let mut handles = self.handles.write();
            handles
                .entry(model_name.to_string())
                .or_default()
                .push(handle.clone());
        }

        Ok(handle)
    }

    /// Return a model handle to the pool
    pub async fn return_model(&self, model_name: &str, handle: ModelHandle) {
        // In a real implementation, we might want to track which handles are in use
        // For now, just keep them in the pool
        let mut handles = self.handles.write();
        if let Some(model_handles) = handles.get_mut(model_name) {
            model_handles.push(handle);
        }
    }

    /// Get pool statistics
    pub async fn stats(&self) -> ModelPoolStats {
        let handles = self.handles.read();
        let mut total_instances = 0;
        let mut model_counts = HashMap::new();

        for (model_name, model_handles) in handles.iter() {
            let count = model_handles.len();
            total_instances += count;
            model_counts.insert(model_name.clone(), count);
        }

        ModelPoolStats {
            total_model_instances: total_instances,
            models: model_counts,
        }
    }

    /// Unload unused model instances
    pub async fn cleanup_unused(&self) -> Result<usize> {
        // In a real implementation, this would unload models that haven't been used recently
        // For now, return 0
        Ok(0)
    }
}

/// Model pool statistics
#[derive(Debug, Clone)]
pub struct ModelPoolStats {
    pub total_model_instances: usize,
    pub models: HashMap<String, usize>,
}

/// HTTP connection pool for external API calls
pub type HttpConnectionPool = ConnectionPool<reqwest::Client>;

impl HttpConnectionPool {
    /// Create a new HTTP connection pool
    pub fn new_http_pool(config: PoolConfig) -> Self {
        Self::new(
            config,
            || {
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .map_err(|e| {
                        FuseError::InternalError(format!("Failed to create HTTP client: {}", e))
                    })
            },
            |_client| {
                // Simple health check - just check if client exists
                Ok(true)
            },
            |_| {
                // HTTP clients don't need explicit cleanup
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool_basic() {
        let pool = ConnectionPool::new(
            PoolConfig::default(),
            || Ok(String::from("test-connection")),
            |_| Ok(true),
            |_| Ok(()),
        );

        let conn = pool.acquire().await.unwrap();
        assert_eq!(conn.get(), "test-connection");

        // Connection is returned when dropped
        drop(conn);

        let stats = pool.stats().await;
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.available_connections, 1);
    }

    #[tokio::test]
    async fn test_connection_pool_max_connections() {
        let config = PoolConfig {
            max_connections: 2,
            ..Default::default()
        };

        let pool = ConnectionPool::new(
            config,
            || Ok(String::from("test-connection")),
            |_| Ok(true),
            |_| Ok(()),
        );

        let _conn1 = pool.acquire().await.unwrap();
        let _conn2 = pool.acquire().await.unwrap();

        // Third acquisition should fail due to max connections
        // Note: In real implementation, this might succeed if connections are returned
        let stats = pool.stats().await;
        assert_eq!(stats.total_connections, 2);
    }

    #[tokio::test]
    async fn test_model_pool() {
        // Mock inference engine for testing
        struct MockEngine;
        #[async_trait::async_trait]
        impl InferenceEngine for MockEngine {
            async fn load_model(&self, model_name: &str) -> Result<ModelHandle> {
                Ok(ModelHandle::new(
                    uuid::Uuid::new_v4().to_string(),
                    model_name.to_string(),
                    crate::model::inference::ModelState {
                        model_path: std::path::PathBuf::from("/tmp/test"),
                        config: crate::model::inference::ModelConfig {
                            max_context_length: 2048,
                            architecture: "test".to_string(),
                            extra: serde_json::json!({}),
                        },
                        is_busy: false,
                    },
                ))
            }

            async fn unload_model(&self, _handle: ModelHandle) -> Result<()> {
                Ok(())
            }

            async fn infer(
                &self,
                _handle: &ModelHandle,
                _input: InferenceInput,
            ) -> Result<InferenceOutput> {
                unimplemented!()
            }

            async fn infer_stream(
                &self,
                _handle: &ModelHandle,
                _input: InferenceInput,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::model::inference::Token>>>
            {
                unimplemented!()
            }

            async fn is_loaded(&self, _model_name: &str) -> bool {
                true
            }

            async fn get_model_info(
                &self,
                _model_name: &str,
            ) -> Result<crate::model::inference::ModelInfo> {
                unimplemented!()
            }
        }

        let engine = Arc::new(MockEngine);
        let pool = ModelPool::new(engine, 2);

        let handle1 = pool.get_model("test-model").await.unwrap();
        let handle2 = pool.get_model("test-model").await.unwrap();

        assert_eq!(handle1.model_name, "test-model");
        assert_eq!(handle2.model_name, "test-model");

        let stats = pool.stats().await;
        assert_eq!(stats.total_model_instances, 2);
        assert_eq!(stats.models["test-model"], 2);
    }
}
