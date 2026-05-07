//! Production-grade async request queuing system with thread ID tracking
//! and intelligent resource management for Fuse AI model server.

use crate::error::{FuseError, Result};
use crate::model::inference::{InferenceInput, InferenceOutput};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use uuid::Uuid;

/// Priority levels for request queuing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Priority {
    /// Background tasks, maintenance operations
    Low = 0,
    /// Normal user requests
    #[default]
    Normal = 1,
    /// Active conversation requests, streaming responses
    High = 2,
    /// Critical system operations, health checks
    Critical = 3,
}

/// Thread ID for conversation tracking across requests
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadId(pub Uuid);

impl ThreadId {
    /// Generate a new thread ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from string representation
    pub fn from_string(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|e| FuseError::ValidationError(format!("Invalid thread ID: {}", e)))
    }

    /// Get string representation
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

/// Request metadata for tracking and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// Unique request ID
    pub request_id: Uuid,
    /// Thread ID for conversation tracking
    pub thread_id: ThreadId,
    /// Model name being requested
    pub model: String,
    /// Request priority
    pub priority: Priority,
    /// Timestamp when request was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when request was queued
    pub queued_at: Option<DateTime<Utc>>,
    /// Timestamp when processing started
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when processing completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Client information (IP, user agent, etc.)
    pub client_info: Option<String>,
    /// Request size estimate (tokens, bytes, etc.)
    pub estimated_size: Option<u64>,
}

/// Queued inference request
#[derive(Debug, Clone)]
pub struct QueuedRequest {
    /// Request metadata
    pub metadata: RequestMetadata,
    /// Inference input data
    pub input: InferenceInput,
    /// Response channel for async completion
    pub response_tx: mpsc::Sender<Result<InferenceOutput>>,
}

/// Queue statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Total requests in queue
    pub total_queued: usize,
    /// Requests by priority level
    pub by_priority: HashMap<String, usize>,
    /// Average queue time in milliseconds
    pub avg_queue_time_ms: f64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Total requests processed
    pub total_processed: usize,
    /// Total requests failed
    pub total_failed: usize,
    /// Current active requests being processed
    pub active_requests: usize,
    /// Queue capacity utilization (0.0 to 1.0)
    pub capacity_utilization: f64,
}

/// Request queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue size
    pub max_size: usize,
    /// Maximum concurrent requests per model
    pub max_concurrent_per_model: usize,
    /// Queue timeout in seconds
    pub queue_timeout_secs: u64,
    /// Processing timeout in seconds
    pub processing_timeout_secs: u64,
    /// Enable queue persistence
    pub enable_persistence: bool,
    /// Persistence file path
    pub persistence_path: Option<String>,
    /// Fair scheduling weight (higher = more fair)
    pub fair_scheduling_weight: f32,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            max_concurrent_per_model: 4,
            queue_timeout_secs: 300,      // 5 minutes
            processing_timeout_secs: 600, // 10 minutes
            enable_persistence: false,
            persistence_path: None,
            fair_scheduling_weight: 0.3,
        }
    }
}

/// Priority queue implementation with fair scheduling
#[derive(Debug)]
struct PriorityQueue {
    queues: [VecDeque<QueuedRequest>; 4],
    total_count: usize,
}

impl PriorityQueue {
    fn new() -> Self {
        Self {
            queues: [
                VecDeque::new(), // Low
                VecDeque::new(), // Normal
                VecDeque::new(), // High
                VecDeque::new(), // Critical
            ],
            total_count: 0,
        }
    }

    fn push(&mut self, request: QueuedRequest) -> Result<()> {
        let priority_idx = request.metadata.priority as usize;
        self.queues[priority_idx].push_back(request);
        self.total_count += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<QueuedRequest> {
        // Try critical first, then high, then normal, then low
        for i in (0..4).rev() {
            if let Some(request) = self.queues[i].pop_front() {
                self.total_count -= 1;
                return Some(request);
            }
        }
        None
    }

    fn pop_fair(&mut self, fair_weight: f32) -> Option<QueuedRequest> {
        // Fair scheduling: occasionally pick from lower priority queues
        let rand_val: f32 = fastrand::f32();
        if rand_val < fair_weight {
            // Fair mode: pick from any non-empty queue proportionally
            let mut available_queues = Vec::new();
            for (i, queue) in self.queues.iter().enumerate() {
                if !queue.is_empty() {
                    available_queues.push(i);
                }
            }

            if !available_queues.is_empty() {
                let selected_idx = available_queues[fastrand::usize(0..available_queues.len())];
                if let Some(request) = self.queues[selected_idx].pop_front() {
                    self.total_count -= 1;
                    return Some(request);
                }
            }
        }

        // Default priority scheduling
        self.pop()
    }

    fn len(&self) -> usize {
        self.total_count
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.total_count == 0
    }

    fn len_by_priority(&self, priority: Priority) -> usize {
        self.queues[priority as usize].len()
    }
}

/// Async request queue manager
pub struct RequestQueue {
    /// Configuration
    config: QueueConfig,
    /// Priority queue storage
    queue: Arc<RwLock<PriorityQueue>>,
    /// Active requests tracking
    active_requests: Arc<RwLock<HashMap<String, usize>>>,
    /// Model concurrency semaphores
    model_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    /// Statistics
    stats: Arc<RwLock<QueueStats>>,
    /// Processing task handles
    _processing_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl RequestQueue {
    /// Create a new request queue
    pub fn new(config: QueueConfig) -> Self {
        let stats = QueueStats {
            total_queued: 0,
            by_priority: HashMap::new(),
            avg_queue_time_ms: 0.0,
            avg_processing_time_ms: 0.0,
            total_processed: 0,
            total_failed: 0,
            active_requests: 0,
            capacity_utilization: 0.0,
        };

        Self {
            config,
            queue: Arc::new(RwLock::new(PriorityQueue::new())),
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            model_semaphores: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(stats)),
            _processing_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queue a new inference request
    pub async fn queue_request(
        &self,
        model: String,
        input: InferenceInput,
        priority: Priority,
        thread_id: Option<ThreadId>,
        client_info: Option<String>,
    ) -> Result<mpsc::Receiver<Result<InferenceOutput>>> {
        let (response_tx, response_rx) = mpsc::channel(1);

        let thread_id = thread_id.unwrap_or_default();

        let metadata = RequestMetadata {
            request_id: Uuid::new_v4(),
            thread_id: thread_id.clone(),
            model: model.clone(),
            priority,
            created_at: Utc::now(),
            queued_at: None,
            started_at: None,
            completed_at: None,
            client_info,
            estimated_size: self.estimate_request_size(&input),
        };

        let request = QueuedRequest {
            metadata: metadata.clone(),
            input,
            response_tx,
        };

        // Check queue capacity
        {
            let queue = self.queue.read().await;
            if queue.len() >= self.config.max_size {
                return Err(FuseError::ResourceLimitExceeded(
                    "Request queue is full".to_string(),
                ));
            }
        }

        // Add to queue
        {
            let mut queue = self.queue.write().await;
            queue.push(request)?;
        }

        // Update statistics
        self.update_stats().await;

        tracing::debug!(
            request_id = %metadata.request_id,
            thread_id = %thread_id.as_string(),
            model = %model,
            priority = ?priority,
            "Request queued successfully"
        );

        Ok(response_rx)
    }

    /// Get the next request to process (with fair scheduling)
    pub async fn next_request(&self) -> Option<QueuedRequest> {
        let mut queue = self.queue.write().await;
        queue.pop_fair(self.config.fair_scheduling_weight)
    }

    /// Mark request as started processing
    pub async fn mark_started(&self, request_id: &Uuid, model: &str) {
        // Acquire model semaphore
        let semaphore = self.get_model_semaphore(model).await;
        let _permit = semaphore.acquire().await.unwrap();

        // Update active requests
        {
            let mut active = self.active_requests.write().await;
            *active.entry(model.to_string()).or_insert(0) += 1;
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_requests += 1;
        }

        tracing::debug!(
            request_id = %request_id,
            model = %model,
            "Request processing started"
        );
    }

    /// Mark request as completed
    pub async fn mark_completed(&self, request_id: &Uuid, model: &str, success: bool) {
        // Update active requests
        {
            let mut active = self.active_requests.write().await;
            if let Some(count) = active.get_mut(model) {
                *count = count.saturating_sub(1);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_requests = stats.active_requests.saturating_sub(1);
            if success {
                stats.total_processed += 1;
            } else {
                stats.total_failed += 1;
            }
        }

        tracing::debug!(
            request_id = %request_id,
            model = %model,
            success = success,
            "Request processing completed"
        );
    }

    /// Get current queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        self.update_stats().await;
        self.stats.read().await.clone()
    }

    /// Get model semaphore (creates if doesn't exist)
    async fn get_model_semaphore(&self, model: &str) -> Arc<Semaphore> {
        let mut semaphores = self.model_semaphores.write().await;
        semaphores
            .entry(model.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.max_concurrent_per_model)))
            .clone()
    }

    /// Update queue statistics
    async fn update_stats(&self) {
        let queue = self.queue.read().await;
        let mut stats = self.stats.write().await;

        stats.total_queued = queue.len();
        stats.capacity_utilization = queue.len() as f64 / self.config.max_size as f64;

        // Update priority counts
        stats.by_priority.clear();
        for priority in &[
            Priority::Low,
            Priority::Normal,
            Priority::High,
            Priority::Critical,
        ] {
            let priority_name = match priority {
                Priority::Low => "low",
                Priority::Normal => "normal",
                Priority::High => "high",
                Priority::Critical => "critical",
            };
            stats
                .by_priority
                .insert(priority_name.to_string(), queue.len_by_priority(*priority));
        }
    }

    /// Estimate request size for prioritization
    fn estimate_request_size(&self, input: &InferenceInput) -> Option<u64> {
        // Rough estimation based on prompt length and images
        let prompt_size = input.prompt.len() as u64;
        let image_size = input
            .images
            .iter()
            .filter_map(|img| img.size_bytes().ok())
            .sum::<usize>() as u64;

        Some(prompt_size + image_size)
    }

    /// Get queue configuration
    pub fn config(&self) -> &QueueConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::inference::InferenceParameters;

    #[tokio::test]
    async fn test_queue_request() {
        let queue = RequestQueue::new(QueueConfig::default());

        let input = InferenceInput {
            prompt: "Test prompt".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let rx = queue
            .queue_request(
                "test-model".to_string(),
                input,
                Priority::Normal,
                None,
                None,
            )
            .await
            .unwrap();

        let stats = queue.get_stats().await;
        assert_eq!(stats.total_queued, 1);
        assert_eq!(stats.by_priority["normal"], 1);

        // Cleanup
        drop(rx);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = RequestQueue::new(QueueConfig::default());

        // Add requests with different priorities
        for priority in &[
            Priority::Low,
            Priority::Normal,
            Priority::High,
            Priority::Critical,
        ] {
            let input = InferenceInput {
                prompt: "Test".to_string(),
                images: vec![],
                context: None,
                parameters: InferenceParameters::default(),
            };

            let _rx = queue
                .queue_request("test-model".to_string(), input, *priority, None, None)
                .await
                .unwrap();
        }

        // Should get critical first
        let request = queue.next_request().await.unwrap();
        assert_eq!(request.metadata.priority, Priority::Critical);

        // Then high
        let request = queue.next_request().await.unwrap();
        assert_eq!(request.metadata.priority, Priority::High);
    }

    #[tokio::test]
    async fn test_thread_id_generation() {
        let thread_id = ThreadId::new();
        assert!(!thread_id.as_string().is_empty());

        let parsed = ThreadId::from_string(&thread_id.as_string()).unwrap();
        assert_eq!(thread_id, parsed);
    }

    #[tokio::test]
    async fn test_queue_capacity_limit() {
        let config = QueueConfig {
            max_size: 2,
            ..Default::default()
        };
        let queue = RequestQueue::new(config);

        // Fill queue
        for i in 0..2 {
            let input = InferenceInput {
                prompt: format!("Test {}", i),
                images: vec![],
                context: None,
                parameters: InferenceParameters::default(),
            };

            let _rx = queue
                .queue_request(
                    "test-model".to_string(),
                    input,
                    Priority::Normal,
                    None,
                    None,
                )
                .await
                .unwrap();
        }

        // Should fail on third request
        let input = InferenceInput {
            prompt: "Test 3".to_string(),
            images: vec![],
            context: None,
            parameters: InferenceParameters::default(),
        };

        let result = queue
            .queue_request(
                "test-model".to_string(),
                input,
                Priority::Normal,
                None,
                None,
            )
            .await;

        assert!(result.is_err());
    }
}
