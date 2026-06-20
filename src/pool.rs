//! Connection pooling and resource management for efficient utilization
//! of system resources in the Fuse AI model server.

use crate::error::{FuseError, Result};
use crate::model::inference::{InferenceEngine, ModelHandle};
#[cfg(test)]
use crate::model::inference::{InferenceInput, InferenceOutput};
use parking_lot::{Mutex as PLMutex, RwLock};
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

/// Shorthand for the shared available-connections queue type
type AvailableQueue<T> = Arc<PLMutex<VecDeque<Arc<Mutex<PooledConnectionInner<T>>>>>>;

/// Pooled connection wrapper
#[allow(dead_code)]
pub struct PooledConnection<T: Send + Sync + 'static> {
    /// The actual connection/resource — Option so it can be safely taken in Drop
    connection: Option<Arc<T>>,
    /// Connection state
    state: ConnectionState,
    /// When the connection was created
    created_at: Instant,
    /// When the connection was last used
    last_used_at: Instant,
    /// Shared reference to the originating pool's available queue for returning on drop
    available: AvailableQueue<T>,
}

impl<T: Send + Sync + 'static> PooledConnection<T> {
    /// Get reference to the underlying connection
    pub fn get(&self) -> &T {
        self.connection
            .as_deref()
            .expect("Connection already released")
    }

    /// Get mutable reference to the underlying connection
    pub fn get_mut(&mut self) -> &mut T {
        self.connection
            .as_mut()
            .and_then(Arc::get_mut)
            .expect("Connection should not have multiple references")
    }
}

impl<T: Send + Sync + 'static> Drop for PooledConnection<T> {
    fn drop(&mut self) {
        // Take the Arc — now this is the only owner (no cloning), so try_unwrap succeeds
        let Some(connection_arc) = self.connection.take() else {
            return;
        };
        let conn = match Arc::try_unwrap(connection_arc) {
            Ok(conn) => conn,
            Err(_) => return, // Connection still has other references
        };
        let conn_inner = PooledConnectionInner {
            connection: Some(conn),
            state: ConnectionState::Available,
            created_at: self.created_at,
            last_used_at: Instant::now(),
        };
        let conn_mutex = Arc::new(Mutex::new(conn_inner));
        // Push synchronously to the SAME queue as the originating pool — no spawn needed
        self.available.lock().push_back(conn_mutex);
    }
}

/// Connection pool for managing reusable resources
pub struct ConnectionPool<T: Send + Sync + 'static> {
    /// Pool configuration
    config: PoolConfig,
    /// Available connections queue — shared via Arc so PooledConnection can return here on drop
    available: AvailableQueue<T>,
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
    /// The connection value — Option so it can be taken without unsafe mem::zeroed
    connection: Option<T>,
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
            available: Arc::new(PLMutex::new(VecDeque::new())),
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
            // Pop under a short sync lock, then release before any await
            let maybe_conn = self.available.lock().pop_front();
            if let Some(conn_mutex) = maybe_conn {
                let mut conn = conn_mutex.lock().await;

                // Check if connection is still valid
                if self.is_connection_valid(&*conn).await {
                    conn.state = ConnectionState::InUse;
                    conn.last_used_at = Instant::now();

                    // Safe extraction — connection is Option<T>
                    let connection = conn.connection.take().ok_or_else(|| {
                        FuseError::InternalError("Connection already taken".to_string())
                    })?;
                    let created_at = conn.created_at;
                    let last_used_at = conn.last_used_at;

                    return Ok(PooledConnection {
                        connection: Some(Arc::new(connection)),
                        state: ConnectionState::InUse,
                        created_at,
                        last_used_at,
                        available: Arc::clone(&self.available),
                    });
                } else {
                    // Connection is invalid — extract and clean up without unsafe
                    let connection = conn.connection.take();
                    drop(conn);
                    if let Some(c) = connection {
                        if let Err(e) = (self.cleanup)(c) {
                            tracing::warn!("Failed to cleanup connection: {}", e);
                        }
                    }
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
        let conn = match connection.connection.take() {
            Some(arc) => match Arc::try_unwrap(arc) {
                Ok(c) => c,
                Err(_) => return,
            },
            None => return,
        };
        let conn_inner = PooledConnectionInner {
            connection: Some(conn),
            state: ConnectionState::Available,
            created_at: connection.created_at,
            last_used_at: Instant::now(),
        };
        let conn_mutex = Arc::new(Mutex::new(conn_inner));
        self.available.lock().push_back(conn_mutex);
    }

    /// Create a new connection
    async fn create_connection(&self) -> Result<PooledConnection<T>> {
        let connection = (self.factory)()?;
        let now = Instant::now();

        Ok(PooledConnection {
            connection: Some(Arc::new(connection)),
            state: ConnectionState::InUse,
            created_at: now,
            last_used_at: now,
            available: Arc::clone(&self.available),
        })
    }

    /// Check if a connection is still valid
    async fn is_connection_valid(&self, conn: &PooledConnectionInner<T>) -> bool {
        let Some(ref connection) = conn.connection else {
            return false;
        };

        // Check age
        if conn.created_at.elapsed() > Duration::from_secs(self.config.max_connection_age_secs) {
            return false;
        }

        // Check idle timeout
        if conn.last_used_at.elapsed() > Duration::from_secs(self.config.idle_timeout_secs) {
            return false;
        }

        // Health check
        (self.health_check)(connection).unwrap_or_default()
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let available_count = self.available.lock().len();
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
        loop {
            // Release sync lock before awaiting cleanup
            let maybe_conn = self.available.lock().pop_front();
            match maybe_conn {
                None => break,
                Some(conn_mutex) => {
                    let mut conn = conn_mutex.lock().await;
                    let connection = conn.connection.take();
                    drop(conn);
                    if let Some(c) = connection {
                        if let Err(e) = (self.cleanup)(c) {
                            tracing::warn!("Failed to cleanup connection: {}", e);
                        }
                    }
                }
            }
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
            available: Arc::new(PLMutex::new(VecDeque::new())),
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
    /// Available (not currently checked-out) model handles
    handles: RwLock<HashMap<String, Vec<ModelHandle>>>,
    /// Total loaded instances per model (available + in-use)
    total_instances: RwLock<HashMap<String, usize>>,
    /// Model loading semaphore to prevent concurrent loads
    load_semaphore: Semaphore,
}

impl<E: InferenceEngine> ModelPool<E> {
    /// Create a new model pool
    pub fn new(engine: Arc<E>, max_concurrent_loads: usize) -> Self {
        Self {
            engine,
            handles: RwLock::new(HashMap::new()),
            total_instances: RwLock::new(HashMap::new()),
            load_semaphore: Semaphore::new(max_concurrent_loads),
        }
    }

    /// Get or load a model handle. Checks out an available handle from the pool,
    /// or loads a fresh instance if none are available.
    pub async fn get_model(&self, model_name: &str) -> Result<ModelHandle> {
        // Check if we have an available (not checked-out) handle
        {
            let mut handles = self.handles.write();
            if let Some(model_handles) = handles.get_mut(model_name) {
                if !model_handles.is_empty() {
                    return Ok(model_handles.remove(0));
                }
            }
        }

        // No available handle — load a new model instance
        let _permit = self.load_semaphore.acquire().await.unwrap();
        let handle = self.engine.load_model(model_name).await?;

        // Track total instance count
        *self
            .total_instances
            .write()
            .entry(model_name.to_string())
            .or_insert(0) += 1;

        Ok(handle)
    }

    /// Return a model handle to the pool for reuse
    pub async fn return_model(&self, model_name: &str, handle: ModelHandle) {
        self.handles
            .write()
            .entry(model_name.to_string())
            .or_default()
            .push(handle);
    }

    /// Get pool statistics
    pub async fn stats(&self) -> ModelPoolStats {
        let total = self.total_instances.read();
        let total_instances: usize = total.values().sum();

        ModelPoolStats {
            total_model_instances: total_instances,
            models: total.clone(),
        }
    }

    /// Unload available (not checked-out) model handles older than `idle_secs`.
    /// Returns the number of handles unloaded.
    pub async fn cleanup_unused(&self, idle_secs: u64) -> Result<usize> {
        let cutoff =
            chrono::Utc::now() - chrono::Duration::seconds(idle_secs as i64);

        let mut to_unload: Vec<(String, ModelHandle)> = Vec::new();
        {
            let mut handles = self.handles.write();
            for (model_name, model_handles) in handles.iter_mut() {
                let mut keep = Vec::new();
                for h in model_handles.drain(..) {
                    if h.loaded_at < cutoff {
                        to_unload.push((model_name.clone(), h));
                    } else {
                        keep.push(h);
                    }
                }
                *model_handles = keep;
            }
        }

        let count = to_unload.len();
        for (model_name, handle) in to_unload {
            if let Err(e) = self.engine.unload_model(handle).await {
                tracing::warn!("Failed to unload idle model {}: {}", model_name, e);
            }
            let mut totals = self.total_instances.write();
            if let Some(n) = totals.get_mut(&model_name) {
                *n = n.saturating_sub(1);
            }
        }
        Ok(count)
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
                    .connect_timeout(Duration::from_secs(10))
                    .tcp_keepalive(Duration::from_secs(60))
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

        // Connection is returned synchronously when dropped — no yield needed
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

    #[tokio::test]
    async fn test_model_pool_cleanup_unused() {
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
            async fn infer(&self, _: &ModelHandle, _: InferenceInput) -> Result<InferenceOutput> {
                unimplemented!()
            }
            async fn infer_stream(
                &self, _: &ModelHandle, _: InferenceInput,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::model::inference::Token>>> {
                unimplemented!()
            }
            async fn is_loaded(&self, _: &str) -> bool { true }
            async fn get_model_info(&self, _: &str) -> Result<crate::model::inference::ModelInfo> {
                unimplemented!()
            }
        }

        let pool = ModelPool::new(Arc::new(MockEngine), 2);

        // Load 2 handles then return both to the available pool
        let h1 = pool.get_model("test-model").await.unwrap();
        let h2 = pool.get_model("test-model").await.unwrap();
        pool.return_model("test-model", h1).await;
        pool.return_model("test-model", h2).await;

        // idle_secs=0 → cutoff=Utc::now() → both handles loaded before cutoff → all stale
        let unloaded = pool.cleanup_unused(0).await.unwrap();
        assert_eq!(unloaded, 2);

        // total_instances decremented to 0
        let stats = pool.stats().await;
        assert_eq!(stats.total_model_instances, 0);

        // available handles queue empty
        assert!(pool
            .handles
            .read()
            .get("test-model")
            .map_or(true, |v| v.is_empty()));
    }
}
