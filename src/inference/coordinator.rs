//! Continuous batching coordinator for inference requests.
//!
//! Receives concurrent inference requests and batches them together for
//! efficient processing through the inference backend.

use crate::error::{FuseError, Result};
use crate::inference::backend::{
    InferenceBackend, InferenceRequest, InferenceResponse, ModelHandle,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::{timeout, Duration};

/// Configuration for the batching coordinator.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of requests in a single batch.
    pub max_batch_size: usize,
    /// Maximum time to wait (in milliseconds) to fill a batch before processing.
    pub max_wait_ms: u64,
    /// Maximum number of concurrent requests allowed.
    pub max_concurrent: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 8,
            max_wait_ms: 50,
            max_concurrent: 32,
        }
    }
}

/// A request waiting to be batched and processed.
struct PendingRequest {
    request: InferenceRequest,
    handle: ModelHandle,
    response_tx: oneshot::Sender<Result<InferenceResponse>>,
}

/// Statistics about coordinator activity.
#[derive(Debug, Clone, Default)]
pub struct CoordinatorStats {
    pub total_requests: u64,
    pub active_requests: u64,
    pub total_batches: u64,
    pub avg_batch_size: f64,
}

/// Shared mutable stats counters.
struct StatsCounters {
    total_requests: AtomicU64,
    active_requests: AtomicU64,
    total_batches: AtomicU64,
    total_batch_items: AtomicU64,
}

impl StatsCounters {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            total_batches: AtomicU64::new(0),
            total_batch_items: AtomicU64::new(0),
        }
    }
}

/// Continuous batching coordinator for inference requests.
///
/// Collects incoming requests and processes them in batches through the
/// inference backend for improved throughput.
pub struct InferenceCoordinator {
    #[allow(dead_code)]
    backend: Arc<dyn InferenceBackend>,
    #[allow(dead_code)]
    config: BatchConfig,
    request_tx: mpsc::Sender<PendingRequest>,
    stats: Arc<StatsCounters>,
    semaphore: Arc<Semaphore>,
}

impl InferenceCoordinator {
    /// Create a new coordinator with the given backend and config.
    pub fn new(backend: Arc<dyn InferenceBackend>, config: BatchConfig) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PendingRequest>(config.max_concurrent * 2);
        let stats = Arc::new(StatsCounters::new());
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        let coordinator = Self {
            backend: backend.clone(),
            config: config.clone(),
            request_tx,
            stats: stats.clone(),
            semaphore,
        };

        // Spawn the batch processing loop.
        coordinator.start_batch_loop(request_rx, backend, config, stats);

        coordinator
    }

    /// Submit a request for inference. Blocks until the result is ready.
    ///
    /// Uses a semaphore to limit the number of concurrent in-flight requests.
    pub async fn submit(
        &self,
        handle: ModelHandle,
        request: InferenceRequest,
    ) -> Result<InferenceResponse> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| FuseError::InferenceError(format!("semaphore closed: {e}")))?;

        self.stats.active_requests.fetch_add(1, Ordering::Relaxed);

        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(PendingRequest {
                request,
                handle,
                response_tx,
            })
            .await
            .map_err(|_| FuseError::InferenceError("coordinator channel closed".to_string()))?;

        let result = response_rx
            .await
            .map_err(|_| FuseError::InferenceError("response channel dropped".to_string()))?;

        self.stats.active_requests.fetch_sub(1, Ordering::Relaxed);

        result
    }

    /// Return a snapshot of coordinator statistics.
    pub fn stats(&self) -> CoordinatorStats {
        let total_batches = self.stats.total_batches.load(Ordering::Relaxed);
        let total_batch_items = self.stats.total_batch_items.load(Ordering::Relaxed);
        let avg_batch_size = if total_batches > 0 {
            total_batch_items as f64 / total_batches as f64
        } else {
            0.0
        };

        CoordinatorStats {
            total_requests: self.stats.total_requests.load(Ordering::Relaxed),
            active_requests: self.stats.active_requests.load(Ordering::Relaxed),
            total_batches,
            avg_batch_size,
        }
    }

    /// Spawn the background batch processing loop.
    fn start_batch_loop(
        &self,
        mut request_rx: mpsc::Receiver<PendingRequest>,
        backend: Arc<dyn InferenceBackend>,
        config: BatchConfig,
        stats: Arc<StatsCounters>,
    ) {
        tokio::spawn(async move {
            loop {
                // Wait for the first request.
                let first = match request_rx.recv().await {
                    Some(req) => req,
                    None => break, // Channel closed, exit loop.
                };

                let mut batch = vec![first];

                // Try to collect more requests up to max_batch_size or max_wait_ms.
                let deadline = Duration::from_millis(config.max_wait_ms);
                let collect_more = async {
                    while batch.len() < config.max_batch_size {
                        match request_rx.try_recv() {
                            Ok(req) => batch.push(req),
                            Err(mpsc::error::TryRecvError::Empty) => {
                                // Wait a bit for more requests.
                                match timeout(deadline, request_rx.recv()).await {
                                    Ok(Some(req)) => batch.push(req),
                                    _ => break,
                                }
                            }
                            Err(mpsc::error::TryRecvError::Disconnected) => break,
                        }
                    }
                };

                collect_more.await;

                let batch_size = batch.len() as u64;
                stats.total_batches.fetch_add(1, Ordering::Relaxed);
                stats
                    .total_batch_items
                    .fetch_add(batch_size, Ordering::Relaxed);
                stats
                    .total_requests
                    .fetch_add(batch_size, Ordering::Relaxed);

                // Process each request serially (true batching comes when
                // the backend supports batched forward passes).
                for pending in batch {
                    let result = backend.infer(&pending.handle, pending.request).await;
                    // Ignore send error — receiver may have been dropped.
                    let _ = pending.response_tx.send(result);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::backend::*;
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use std::path::Path;

    /// Mock backend for coordinator tests.
    struct MockBackend;

    impl MockBackend {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl InferenceBackend for MockBackend {
        fn info(&self) -> BackendInfo {
            BackendInfo {
                name: "mock".to_string(),
                backend_type: BackendType::CpuSimd,
                supports_streaming: true,
                supports_embeddings: true,
                max_batch_size: 32,
            }
        }

        async fn load_model(&self, _path: &Path, _config: &ModelConfig) -> Result<ModelHandle> {
            Ok(ModelHandle {
                id: "mock-handle".to_string(),
                model_name: "test-model".to_string(),
            })
        }

        async fn unload_model(&self, _handle: &ModelHandle) -> Result<()> {
            Ok(())
        }

        async fn infer(
            &self,
            _handle: &ModelHandle,
            req: InferenceRequest,
        ) -> Result<InferenceResponse> {
            Ok(InferenceResponse {
                text: format!("Response to: {}", req.prompt),
                tokens_generated: 5,
                tokens_per_second: 100.0,
                stop_reason: StopReason::EndOfSequence,
            })
        }

        fn stream(
            &self,
            _handle: &ModelHandle,
            _req: InferenceRequest,
        ) -> BoxStream<'_, Result<Token>> {
            Box::pin(futures::stream::empty())
        }

        async fn embed(&self, _handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }

        fn resource_usage(&self) -> ResourceUsage {
            ResourceUsage::default()
        }
    }

    fn make_handle() -> ModelHandle {
        ModelHandle {
            id: "test".to_string(),
            model_name: "test-model".to_string(),
        }
    }

    fn make_request(prompt: &str) -> InferenceRequest {
        InferenceRequest {
            prompt: prompt.to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_single_request() {
        let backend = Arc::new(MockBackend::new());
        let coordinator = InferenceCoordinator::new(backend, BatchConfig::default());

        let resp = coordinator
            .submit(make_handle(), make_request("hello"))
            .await
            .unwrap();

        assert!(resp.text.contains("hello"));
        assert_eq!(resp.stop_reason, StopReason::EndOfSequence);
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let backend = Arc::new(MockBackend::new());
        let coordinator = Arc::new(InferenceCoordinator::new(backend, BatchConfig::default()));

        let mut handles = Vec::new();
        for i in 0..10 {
            let coord = coordinator.clone();
            handles.push(tokio::spawn(async move {
                coord
                    .submit(make_handle(), make_request(&format!("prompt-{i}")))
                    .await
            }));
        }

        for handle in handles {
            let resp = handle.await.unwrap().unwrap();
            assert!(resp.text.starts_with("Response to:"));
        }
    }

    #[tokio::test]
    async fn test_concurrency_limit() {
        let config = BatchConfig {
            max_concurrent: 2,
            ..Default::default()
        };
        let backend = Arc::new(MockBackend::new());
        let coordinator = Arc::new(InferenceCoordinator::new(backend, config));

        // Submit 5 requests — they should all complete even with limit of 2
        // concurrent, because the semaphore queues them.
        let mut handles = Vec::new();
        for i in 0..5 {
            let coord = coordinator.clone();
            handles.push(tokio::spawn(async move {
                coord
                    .submit(make_handle(), make_request(&format!("prompt-{i}")))
                    .await
            }));
        }

        for handle in handles {
            let resp = handle.await.unwrap().unwrap();
            assert!(resp.text.starts_with("Response to:"));
        }

        // Verify the semaphore is working: all requests completed.
        let stats = coordinator.stats();
        assert_eq!(stats.total_requests, 5);
    }

    #[tokio::test]
    async fn test_coordinator_stats() {
        let backend = Arc::new(MockBackend::new());
        let coordinator = Arc::new(InferenceCoordinator::new(backend, BatchConfig::default()));

        // Initially empty.
        let stats = coordinator.stats();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_batches, 0);

        // Submit a few requests.
        for i in 0..3 {
            coordinator
                .submit(make_handle(), make_request(&format!("p{i}")))
                .await
                .unwrap();
        }

        let stats = coordinator.stats();
        assert_eq!(stats.total_requests, 3);
        assert!(stats.total_batches >= 1);
        assert!(stats.avg_batch_size > 0.0);
        assert_eq!(stats.active_requests, 0);
    }

    #[test]
    fn test_batch_config_defaults() {
        let config = BatchConfig::default();
        assert_eq!(config.max_batch_size, 8);
        assert_eq!(config.max_wait_ms, 50);
        assert_eq!(config.max_concurrent, 32);
    }
}
