//! Observability stack — metrics, tracing, logging.
//!
//! Provides Prometheus metrics, OpenTelemetry tracing, and structured logging.

pub mod metrics;

pub use metrics::{InferenceMetrics, MetricsCollector};
