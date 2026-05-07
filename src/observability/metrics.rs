//! Prometheus-compatible metrics collection for inference and API performance.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Inference-specific metrics.
#[derive(Debug, Default)]
pub struct InferenceMetrics {
    pub total_requests: AtomicU64,
    pub total_tokens_generated: AtomicU64,
    pub total_errors: AtomicU64,
    pub active_requests: AtomicU64,
    latencies: Arc<RwLock<LatencyTracker>>,
    model_metrics: Arc<RwLock<HashMap<String, ModelMetrics>>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub requests: u64,
    pub tokens_generated: u64,
    pub errors: u64,
    pub avg_tokens_per_second: f64,
    pub avg_latency_ms: f64,
}

/// Tracks latency percentiles using a histogram-like approach.
#[derive(Debug, Default)]
struct LatencyTracker {
    samples: Vec<f64>,
    max_samples: usize,
}

impl LatencyTracker {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    fn record(&mut self, latency_ms: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(latency_ms);
    }

    fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

/// Snapshot of current metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub total_tokens: u64,
    pub total_errors: u64,
    pub active_requests: u64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub models: HashMap<String, ModelMetrics>,
}

impl InferenceMetrics {
    pub fn new() -> Self {
        Self {
            latencies: Arc::new(RwLock::new(LatencyTracker::new(10_000))),
            model_metrics: Arc::new(RwLock::new(HashMap::new())),
            ..Default::default()
        }
    }

    /// Record start of a request.
    pub fn request_start(&self) {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record completion of a request.
    pub fn request_complete(&self, model: &str, tokens: u64, latency: Duration, success: bool) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(tokens, Ordering::Relaxed);

        let latency_ms = latency.as_secs_f64() * 1000.0;
        self.latencies.write().record(latency_ms);

        if !success {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }

        // Update per-model metrics
        let mut models = self.model_metrics.write();
        let entry = models.entry(model.to_string()).or_default();
        entry.requests += 1;
        entry.tokens_generated += tokens;
        if !success {
            entry.errors += 1;
        }
        let tok_per_sec = if latency.as_secs_f64() > 0.0 {
            tokens as f64 / latency.as_secs_f64()
        } else {
            0.0
        };
        // Running average
        entry.avg_tokens_per_second = (entry.avg_tokens_per_second * (entry.requests - 1) as f64
            + tok_per_sec)
            / entry.requests as f64;
        entry.avg_latency_ms = (entry.avg_latency_ms * (entry.requests - 1) as f64 + latency_ms)
            / entry.requests as f64;
    }

    /// Get current metrics snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let latencies = self.latencies.read();
        let models = self.model_metrics.read();

        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_tokens: self.total_tokens_generated.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            active_requests: self.active_requests.load(Ordering::Relaxed),
            latency_p50_ms: latencies.percentile(50.0),
            latency_p95_ms: latencies.percentile(95.0),
            latency_p99_ms: latencies.percentile(99.0),
            models: models.clone(),
        }
    }
}

/// Aggregated metrics collector.
pub struct MetricsCollector {
    pub inference: Arc<InferenceMetrics>,
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inference: Arc::new(InferenceMetrics::new()),
            start_time: Instant::now(),
        }
    }

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Export metrics in Prometheus text format.
    pub fn prometheus_export(&self) -> String {
        let snap = self.inference.snapshot();
        let mut output = String::new();

        output.push_str("# HELP fuse_requests_total Total inference requests\n");
        output.push_str("# TYPE fuse_requests_total counter\n");
        output.push_str(&format!("fuse_requests_total {}\n", snap.total_requests));

        output.push_str("# HELP fuse_tokens_total Total tokens generated\n");
        output.push_str("# TYPE fuse_tokens_total counter\n");
        output.push_str(&format!("fuse_tokens_total {}\n", snap.total_tokens));

        output.push_str("# HELP fuse_errors_total Total errors\n");
        output.push_str("# TYPE fuse_errors_total counter\n");
        output.push_str(&format!("fuse_errors_total {}\n", snap.total_errors));

        output.push_str("# HELP fuse_active_requests Current active requests\n");
        output.push_str("# TYPE fuse_active_requests gauge\n");
        output.push_str(&format!("fuse_active_requests {}\n", snap.active_requests));

        output.push_str("# HELP fuse_latency_ms Request latency in milliseconds\n");
        output.push_str("# TYPE fuse_latency_ms summary\n");
        output.push_str(&format!(
            "fuse_latency_ms{{quantile=\"0.5\"}} {:.2}\n",
            snap.latency_p50_ms
        ));
        output.push_str(&format!(
            "fuse_latency_ms{{quantile=\"0.95\"}} {:.2}\n",
            snap.latency_p95_ms
        ));
        output.push_str(&format!(
            "fuse_latency_ms{{quantile=\"0.99\"}} {:.2}\n",
            snap.latency_p99_ms
        ));

        output.push_str(&format!(
            "# HELP fuse_uptime_seconds Server uptime\n# TYPE fuse_uptime_seconds gauge\nfuse_uptime_seconds {}\n",
            self.uptime_secs()
        ));

        output
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_metrics_new() {
        let metrics = InferenceMetrics::new();
        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.active_requests.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_request_lifecycle() {
        let metrics = InferenceMetrics::new();
        metrics.request_start();
        assert_eq!(metrics.active_requests.load(Ordering::Relaxed), 1);

        metrics.request_complete("llama3", 50, Duration::from_millis(500), true);
        assert_eq!(metrics.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.total_tokens_generated.load(Ordering::Relaxed), 50);
    }

    #[test]
    fn test_error_tracking() {
        let metrics = InferenceMetrics::new();
        metrics.request_start();
        metrics.request_complete("llama3", 0, Duration::from_millis(100), false);
        assert_eq!(metrics.total_errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_per_model_metrics() {
        let metrics = InferenceMetrics::new();
        metrics.request_start();
        metrics.request_complete("llama3", 50, Duration::from_millis(500), true);
        metrics.request_start();
        metrics.request_complete("llama3", 100, Duration::from_millis(1000), true);

        let snap = metrics.snapshot();
        let model = snap.models.get("llama3").unwrap();
        assert_eq!(model.requests, 2);
        assert_eq!(model.tokens_generated, 150);
    }

    #[test]
    fn test_latency_percentiles() {
        let metrics = InferenceMetrics::new();
        for i in 1..=100 {
            metrics.request_start();
            metrics.request_complete("test", 10, Duration::from_millis(i), true);
        }

        let snap = metrics.snapshot();
        assert!(snap.latency_p50_ms > 40.0 && snap.latency_p50_ms < 60.0);
        assert!(snap.latency_p95_ms > 90.0);
        assert!(snap.latency_p99_ms > 95.0);
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = InferenceMetrics::new();
        let snap = metrics.snapshot();
        assert_eq!(snap.total_requests, 0);
        assert_eq!(snap.total_tokens, 0);
        assert_eq!(snap.active_requests, 0);
        assert!(snap.models.is_empty());
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        assert!(collector.uptime_secs() < 2);
    }

    #[test]
    fn test_prometheus_export() {
        let collector = MetricsCollector::new();
        collector.inference.request_start();
        collector
            .inference
            .request_complete("test", 100, Duration::from_millis(500), true);

        let output = collector.prometheus_export();
        assert!(output.contains("fuse_requests_total 1"));
        assert!(output.contains("fuse_tokens_total 100"));
        assert!(output.contains("fuse_errors_total 0"));
        assert!(output.contains("fuse_active_requests 0"));
        assert!(output.contains("fuse_uptime_seconds"));
    }

    #[test]
    fn test_prometheus_export_format() {
        let collector = MetricsCollector::new();
        let output = collector.prometheus_export();
        // Verify Prometheus text format
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
        assert!(output.contains("counter"));
        assert!(output.contains("gauge"));
    }

    #[test]
    fn test_latency_tracker_empty() {
        let tracker = LatencyTracker::new(100);
        assert_eq!(tracker.percentile(50.0), 0.0);
    }

    #[test]
    fn test_latency_tracker_max_samples() {
        let mut tracker = LatencyTracker::new(5);
        for i in 0..10 {
            tracker.record(i as f64);
        }
        assert_eq!(tracker.samples.len(), 5);
    }

    #[test]
    fn test_model_metrics_serialization() {
        let m = ModelMetrics {
            requests: 10,
            tokens_generated: 500,
            errors: 1,
            avg_tokens_per_second: 100.0,
            avg_latency_ms: 50.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"requests\":10"));
    }
}
