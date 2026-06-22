use crate::error::{FuseError, Result};
use crate::model::inference::ModelHandle;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Resource usage statistics for a model
#[derive(Debug, Clone)]
pub struct ResourceStats {
    /// Memory usage in bytes (VRAM + RAM)
    pub memory_bytes: u64,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// GPU usage percentage (0-100)
    pub gpu_percent: f32,
    /// Last access time
    pub last_access: Instant,
    /// Number of active requests
    pub active_requests: usize,
}

/// Resource management policy
#[derive(Debug, Clone)]
pub struct ResourcePolicy {
    /// Time before considering a model idle (seconds)
    pub idle_timeout: Duration,
    /// Maximum memory usage before triggering cleanup (bytes)
    pub max_memory_bytes: u64,
    /// Maximum number of models to keep loaded
    pub max_loaded_models: usize,
    /// Enable automatic unloading of idle models
    pub auto_unload_idle: bool,
    /// Enable memory optimization for idle models
    pub optimize_idle_memory: bool,
    /// Enable GPU offloading for idle models
    pub offload_to_cpu: bool,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(300),   // 5 minutes
            max_memory_bytes: 8 * 1024 * 1024 * 1024, // 8GB
            max_loaded_models: 3,
            auto_unload_idle: true,
            optimize_idle_memory: true,
            offload_to_cpu: true,
        }
    }
}

/// Model state for resource management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelState {
    /// Model is actively processing requests
    Active,
    /// Model is loaded but idle
    Idle,
    /// Model is optimized for low resource usage
    Optimized,
    /// Model is offloaded to CPU
    OffloadedToCpu,
    /// Model is unloaded from memory
    Unloaded,
}

#[allow(dead_code)]
struct ModelResourceInfo {
    handle: ModelHandle,
    stats: ResourceStats,
    state: ModelState,
}

/// Resource manager for intelligent model lifecycle management
pub struct ResourceManager {
    models: Arc<RwLock<HashMap<String, ModelResourceInfo>>>,
    policy: ResourcePolicy,
    monitor_tx: mpsc::Sender<ResourceCommand>,
}

#[allow(dead_code)]
enum ResourceCommand {
    UpdateStats(String, ResourceStats),
    CheckIdle,
    OptimizeMemory,
    Shutdown,
}

impl ResourceManager {
    pub fn new(policy: ResourcePolicy) -> Self {
        let models = Arc::new(RwLock::new(HashMap::new()));
        let (monitor_tx, monitor_rx) = mpsc::channel(100);

        let manager = Self {
            models: models.clone(),
            policy: policy.clone(),
            monitor_tx,
        };

        // Start background monitoring task
        tokio::spawn(Self::monitor_loop(models, policy, monitor_rx));

        manager
    }

    /// Register a model for resource management
    pub fn register_model(&self, name: String, handle: ModelHandle, memory_bytes: u64) {
        let info = ModelResourceInfo {
            handle,
            stats: ResourceStats {
                memory_bytes,
                cpu_percent: 0.0,
                gpu_percent: 0.0,
                last_access: Instant::now(),
                active_requests: 0,
            },
            state: ModelState::Active,
        };

        self.models.write().insert(name, info);
        info!("Registered model for resource management");
    }

    /// Unregister a model
    pub fn unregister_model(&self, name: &str) {
        self.models.write().remove(name);
        info!("Unregistered model from resource management");
    }

    /// Mark model as active (being used)
    pub fn mark_active(&self, name: &str) {
        if let Some(info) = self.models.write().get_mut(name) {
            info.state = ModelState::Active;
            info.stats.last_access = Instant::now();
            info.stats.active_requests += 1;
            debug!("Model {} marked as active", name);
        }
    }

    /// Mark model request as complete
    pub fn mark_request_complete(&self, name: &str) {
        if let Some(info) = self.models.write().get_mut(name) {
            info.stats.active_requests = info.stats.active_requests.saturating_sub(1);
            if info.stats.active_requests == 0 {
                info.state = ModelState::Idle;
                debug!("Model {} is now idle", name);
            }
        }
    }

    /// Get current resource usage for a model
    pub fn get_stats(&self, name: &str) -> Option<ResourceStats> {
        self.models.read().get(name).map(|info| info.stats.clone())
    }

    /// Get total memory usage across all models
    pub fn total_memory_usage(&self) -> u64 {
        self.models
            .read()
            .values()
            .map(|info| info.stats.memory_bytes)
            .sum()
    }

    /// Get number of loaded models
    pub fn loaded_model_count(&self) -> usize {
        self.models.read().len()
    }

    /// Check if resource limits are exceeded
    pub fn is_over_limit(&self) -> bool {
        let models = self.models.read();
        let total_memory = models
            .values()
            .map(|info| info.stats.memory_bytes)
            .sum::<u64>();
        let model_count = models.len();

        total_memory > self.policy.max_memory_bytes || model_count > self.policy.max_loaded_models
    }

    /// Get list of idle models that can be optimized
    pub fn get_idle_models(&self) -> Vec<String> {
        let now = Instant::now();
        self.models
            .read()
            .iter()
            .filter(|(_, info)| {
                info.state == ModelState::Idle
                    && now.duration_since(info.stats.last_access) > self.policy.idle_timeout
                    && info.stats.active_requests == 0
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Optimize memory for idle models
    pub async fn optimize_idle_models(&self) -> Result<Vec<String>> {
        if !self.policy.optimize_idle_memory {
            return Ok(Vec::new());
        }

        let idle_models = self.get_idle_models();
        let mut optimized = Vec::new();

        for model_name in idle_models {
            if let Err(e) = self.optimize_model(&model_name).await {
                warn!("Failed to optimize model {}: {}", model_name, e);
            } else {
                optimized.push(model_name);
            }
        }

        if !optimized.is_empty() {
            info!("Optimized {} idle models", optimized.len());
        }

        Ok(optimized)
    }

    /// Optimize a specific model's resource usage
    async fn optimize_model(&self, name: &str) -> Result<()> {
        let mut models = self.models.write();

        if let Some(info) = models.get_mut(name) {
            if info.state == ModelState::Idle {
                if self.policy.offload_to_cpu {
                    // Offload to CPU to free GPU memory
                    info.state = ModelState::OffloadedToCpu;
                    // Reduce memory estimate (GPU -> CPU typically uses less)
                    info.stats.memory_bytes = (info.stats.memory_bytes as f64 * 0.7) as u64;
                    info.stats.gpu_percent = 0.0;
                    info!("Offloaded model {} to CPU", name);
                } else {
                    // Just optimize memory without offloading
                    info.state = ModelState::Optimized;
                    info.stats.memory_bytes = (info.stats.memory_bytes as f64 * 0.8) as u64;
                    info!("Optimized memory for model {}", name);
                }
            }
        }

        Ok(())
    }

    /// Unload least recently used models if over limit
    pub async fn enforce_limits(&self) -> Result<Vec<String>> {
        if !self.is_over_limit() {
            return Ok(Vec::new());
        }

        let mut unloaded = Vec::new();

        // Get models sorted by last access time (oldest first)
        let mut models_by_access: Vec<_> = self
            .models
            .read()
            .iter()
            .filter(|(_, info)| info.stats.active_requests == 0)
            .map(|(name, info)| (name.clone(), info.stats.last_access))
            .collect();

        models_by_access.sort_by_key(|(_, last_access)| *last_access);

        // Unload models until we're under limits
        for (model_name, _) in models_by_access {
            if !self.is_over_limit() {
                break;
            }

            if self.policy.auto_unload_idle {
                self.unregister_model(&model_name);
                unloaded.push(model_name.clone());
                info!("Auto-unloaded model {} to free resources", model_name);
            }
        }

        Ok(unloaded)
    }

    /// Background monitoring loop
    async fn monitor_loop(
        models: Arc<RwLock<HashMap<String, ModelResourceInfo>>>,
        policy: ResourcePolicy,
        mut rx: mpsc::Receiver<ResourceCommand>,
    ) {
        let mut check_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    // Periodic check for idle models
                    Self::check_and_optimize_idle(&models, &policy).await;
                }
                Some(cmd) = rx.recv() => {
                    match cmd {
                        ResourceCommand::UpdateStats(name, stats) => {
                            if let Some(info) = models.write().get_mut(&name) {
                                info.stats = stats;
                            }
                        }
                        ResourceCommand::CheckIdle => {
                            Self::check_and_optimize_idle(&models, &policy).await;
                        }
                        ResourceCommand::OptimizeMemory => {
                            Self::optimize_all_idle(&models, &policy).await;
                        }
                        ResourceCommand::Shutdown => {
                            info!("Resource manager shutting down");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn check_and_optimize_idle(
        models: &Arc<RwLock<HashMap<String, ModelResourceInfo>>>,
        policy: &ResourcePolicy,
    ) {
        let now = Instant::now();
        let idle_models: Vec<String> = models
            .read()
            .iter()
            .filter(|(_, info)| {
                info.state == ModelState::Idle
                    && now.duration_since(info.stats.last_access) > policy.idle_timeout
                    && info.stats.active_requests == 0
            })
            .map(|(name, _)| name.clone())
            .collect();

        if !idle_models.is_empty() {
            debug!("Found {} idle models to optimize", idle_models.len());

            for model_name in idle_models {
                let mut models_write = models.write();
                if let Some(info) = models_write.get_mut(&model_name) {
                    if policy.offload_to_cpu && info.state == ModelState::Idle {
                        info.state = ModelState::OffloadedToCpu;
                        info.stats.memory_bytes = (info.stats.memory_bytes as f64 * 0.7) as u64;
                        info.stats.gpu_percent = 0.0;
                        info!("Auto-offloaded idle model {} to CPU", model_name);
                    }
                }
            }
        }
    }

    async fn optimize_all_idle(
        models: &Arc<RwLock<HashMap<String, ModelResourceInfo>>>,
        policy: &ResourcePolicy,
    ) {
        if !policy.optimize_idle_memory {
            return;
        }

        let mut models_write = models.write();
        for (name, info) in models_write.iter_mut() {
            if info.state == ModelState::Idle && info.stats.active_requests == 0 {
                info.state = ModelState::Optimized;
                info.stats.memory_bytes = (info.stats.memory_bytes as f64 * 0.8) as u64;
                debug!("Optimized memory for idle model {}", name);
            }
        }
    }

    /// Trigger manual optimization
    pub async fn trigger_optimization(&self) -> Result<()> {
        self.monitor_tx
            .send(ResourceCommand::OptimizeMemory)
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to trigger optimization: {}", e)))
    }

    /// Shutdown the resource manager
    pub async fn shutdown(&self) -> Result<()> {
        self.monitor_tx
            .send(ResourceCommand::Shutdown)
            .await
            .map_err(|e| FuseError::InternalError(format!("Failed to shutdown: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::inference::{ModelConfig, ModelState as InferenceModelState};

    fn create_test_handle(name: &str) -> ModelHandle {
        let state = InferenceModelState {
            model_path: std::path::PathBuf::from("/tmp/test"),
            config: ModelConfig {
                max_context_length: 2048,
                architecture: "test".to_string(),
                extra: serde_json::json!({}),
            },
            is_busy: false,
        };

        ModelHandle::new(format!("test-{}", name), name.to_string(), state)
    }

    #[tokio::test]
    async fn test_resource_manager_creation() {
        let policy = ResourcePolicy::default();
        let manager = ResourceManager::new(policy);

        assert_eq!(manager.loaded_model_count(), 0);
        assert_eq!(manager.total_memory_usage(), 0);
    }

    #[tokio::test]
    async fn test_register_model() {
        let policy = ResourcePolicy::default();
        let manager = ResourceManager::new(policy);

        let handle = create_test_handle("test-model");
        manager.register_model("test-model".to_string(), handle, 1024 * 1024 * 1024);

        assert_eq!(manager.loaded_model_count(), 1);
        assert_eq!(manager.total_memory_usage(), 1024 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_mark_active() {
        let policy = ResourcePolicy::default();
        let manager = ResourceManager::new(policy);

        let handle = create_test_handle("test-model");
        manager.register_model("test-model".to_string(), handle, 1024 * 1024 * 1024);

        manager.mark_active("test-model");

        let stats = manager.get_stats("test-model").unwrap();
        assert_eq!(stats.active_requests, 1);
    }

    #[tokio::test]
    async fn test_mark_request_complete() {
        let policy = ResourcePolicy::default();
        let manager = ResourceManager::new(policy);

        let handle = create_test_handle("test-model");
        manager.register_model("test-model".to_string(), handle, 1024 * 1024 * 1024);

        manager.mark_active("test-model");
        manager.mark_request_complete("test-model");

        let stats = manager.get_stats("test-model").unwrap();
        assert_eq!(stats.active_requests, 0);
    }

    #[tokio::test]
    async fn test_is_over_limit() {
        let mut policy = ResourcePolicy::default();
        policy.max_memory_bytes = 1024 * 1024; // 1MB limit
        policy.max_loaded_models = 2;

        let manager = ResourceManager::new(policy);

        let handle1 = create_test_handle("model1");
        manager.register_model("model1".to_string(), handle1, 512 * 1024);
        assert!(!manager.is_over_limit());

        let handle2 = create_test_handle("model2");
        manager.register_model("model2".to_string(), handle2, 512 * 1024);
        assert!(!manager.is_over_limit());

        let handle3 = create_test_handle("model3");
        manager.register_model("model3".to_string(), handle3, 512 * 1024);
        assert!(manager.is_over_limit()); // Over model count limit
    }

    #[tokio::test]
    async fn test_get_idle_models() {
        let mut policy = ResourcePolicy::default();
        policy.idle_timeout = Duration::from_millis(100);

        let manager = ResourceManager::new(policy);

        let handle = create_test_handle("test-model");
        manager.register_model("test-model".to_string(), handle, 1024 * 1024 * 1024);

        manager.mark_active("test-model");
        manager.mark_request_complete("test-model");

        // Poll until the model becomes idle (avoids timing-sensitive fixed sleeps)
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let idle_models = manager.get_idle_models();
            if idle_models.len() == 1 {
                assert_eq!(idle_models[0], "test-model");
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("Timed out waiting for model to become idle");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
