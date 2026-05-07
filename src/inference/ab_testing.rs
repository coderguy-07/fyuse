use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{FuseError, Result};

/// Defines a traffic split between two models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplit {
    pub model_a: String,
    pub model_b: String,
    /// Percentage of traffic routed to model A (0.0 to 100.0).
    pub percentage_a: f64,
}

impl TrafficSplit {
    pub fn new(
        model_a: impl Into<String>,
        model_b: impl Into<String>,
        percentage_a: f64,
    ) -> Result<Self> {
        if !(0.0..=100.0).contains(&percentage_a) {
            return Err(FuseError::ValidationError(format!(
                "percentage_a must be between 0.0 and 100.0, got {percentage_a}"
            )));
        }
        Ok(Self {
            model_a: model_a.into(),
            model_b: model_b.into(),
            percentage_a,
        })
    }

    pub fn percentage_b(&self) -> f64 {
        100.0 - self.percentage_a
    }
}

/// Tracks a quality metric for A/B testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetric {
    pub latency: Duration,
    pub tokens_per_second: f64,
    pub user_rating: Option<f64>,
}

impl QualityMetric {
    pub fn new(latency: Duration, tokens_per_second: f64) -> Self {
        Self {
            latency,
            tokens_per_second,
            user_rating: None,
        }
    }

    pub fn with_rating(mut self, rating: f64) -> Self {
        self.user_rating = Some(rating);
        self
    }
}

/// Configuration for an A/B test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    pub name: String,
    pub split: TrafficSplit,
    pub min_samples: usize,
    pub enabled: bool,
}

impl ABTestConfig {
    pub fn new(name: impl Into<String>, split: TrafficSplit, min_samples: usize) -> Self {
        Self {
            name: name.into(),
            split,
            min_samples,
            enabled: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(FuseError::ValidationError(
                "A/B test name cannot be empty".to_string(),
            ));
        }
        if self.min_samples == 0 {
            return Err(FuseError::ValidationError(
                "min_samples must be > 0".to_string(),
            ));
        }
        if self.split.model_a == self.split.model_b {
            return Err(FuseError::ValidationError(
                "model_a and model_b must be different".to_string(),
            ));
        }
        Ok(())
    }
}

/// Routes requests to model A or B based on the configured split.
#[derive(Debug)]
pub struct ABRouter {
    config: ABTestConfig,
    metrics_a: Vec<QualityMetric>,
    metrics_b: Vec<QualityMetric>,
    request_count: u64,
}

/// Which model variant was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variant {
    A,
    B,
}

impl ABRouter {
    pub fn new(config: ABTestConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            metrics_a: Vec::new(),
            metrics_b: Vec::new(),
            request_count: 0,
        })
    }

    /// Route a request, returning which model to use.
    /// Uses a deterministic approach based on request count for testability.
    pub fn route(&mut self) -> Result<(Variant, &str)> {
        if !self.config.enabled {
            return Err(FuseError::FeatureDisabled(
                "A/B test is disabled".to_string(),
            ));
        }
        self.request_count += 1;
        let threshold = self.config.split.percentage_a;
        // Use modular arithmetic for deterministic distribution
        let bucket = ((self.request_count - 1) % 100) as f64;
        if bucket < threshold {
            Ok((Variant::A, &self.config.split.model_a))
        } else {
            Ok((Variant::B, &self.config.split.model_b))
        }
    }

    /// Route using an explicit random value (0.0..100.0) for stochastic routing.
    pub fn route_with_value(&self, value: f64) -> Result<(Variant, &str)> {
        if !self.config.enabled {
            return Err(FuseError::FeatureDisabled(
                "A/B test is disabled".to_string(),
            ));
        }
        if value < self.config.split.percentage_a {
            Ok((Variant::A, &self.config.split.model_a))
        } else {
            Ok((Variant::B, &self.config.split.model_b))
        }
    }

    pub fn record_metric(&mut self, variant: &Variant, metric: QualityMetric) {
        match variant {
            Variant::A => self.metrics_a.push(metric),
            Variant::B => self.metrics_b.push(metric),
        }
    }

    pub fn metrics_a(&self) -> &[QualityMetric] {
        &self.metrics_a
    }

    pub fn metrics_b(&self) -> &[QualityMetric] {
        &self.metrics_b
    }

    /// Average latency for a variant.
    pub fn avg_latency(&self, variant: &Variant) -> Option<Duration> {
        let metrics = match variant {
            Variant::A => &self.metrics_a,
            Variant::B => &self.metrics_b,
        };
        if metrics.is_empty() {
            return None;
        }
        let total: Duration = metrics.iter().map(|m| m.latency).sum();
        Some(total / metrics.len() as u32)
    }

    /// Average tokens/s for a variant.
    pub fn avg_tokens_per_second(&self, variant: &Variant) -> Option<f64> {
        let metrics = match variant {
            Variant::A => &self.metrics_a,
            Variant::B => &self.metrics_b,
        };
        if metrics.is_empty() {
            return None;
        }
        let total: f64 = metrics.iter().map(|m| m.tokens_per_second).sum();
        Some(total / metrics.len() as f64)
    }

    /// Check if we have enough samples to draw conclusions.
    pub fn has_sufficient_samples(&self) -> bool {
        self.metrics_a.len() >= self.config.min_samples
            && self.metrics_b.len() >= self.config.min_samples
    }

    /// Rollback: disable the test and return the better-performing model.
    /// Returns None if insufficient data.
    pub fn rollback(&mut self) -> Option<String> {
        self.config.enabled = false;
        let avg_a = self.avg_tokens_per_second(&Variant::A);
        let avg_b = self.avg_tokens_per_second(&Variant::B);
        match (avg_a, avg_b) {
            (Some(a), Some(b)) => {
                if a >= b {
                    Some(self.config.split.model_a.clone())
                } else {
                    Some(self.config.split.model_b.clone())
                }
            }
            (Some(_), None) => Some(self.config.split.model_a.clone()),
            (None, Some(_)) => Some(self.config.split.model_b.clone()),
            (None, None) => None,
        }
    }

    pub fn config(&self) -> &ABTestConfig {
        &self.config
    }

    pub fn request_count(&self) -> u64 {
        self.request_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_split() -> TrafficSplit {
        TrafficSplit::new("model-a", "model-b", 70.0).unwrap()
    }

    fn make_config() -> ABTestConfig {
        ABTestConfig::new("test-experiment", make_split(), 10)
    }

    #[test]
    fn test_traffic_split_valid() {
        let split = TrafficSplit::new("a", "b", 50.0).unwrap();
        assert_eq!(split.percentage_b(), 50.0);
    }

    #[test]
    fn test_traffic_split_invalid_percentage() {
        assert!(TrafficSplit::new("a", "b", -1.0).is_err());
        assert!(TrafficSplit::new("a", "b", 101.0).is_err());
    }

    #[test]
    fn test_traffic_split_boundary() {
        let split = TrafficSplit::new("a", "b", 0.0).unwrap();
        assert_eq!(split.percentage_b(), 100.0);
        let split = TrafficSplit::new("a", "b", 100.0).unwrap();
        assert_eq!(split.percentage_b(), 0.0);
    }

    #[test]
    fn test_config_validation_empty_name() {
        let cfg = ABTestConfig::new("", make_split(), 10);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_config_validation_zero_samples() {
        let cfg = ABTestConfig::new("test", make_split(), 0);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_config_validation_same_models() {
        let split = TrafficSplit::new("same", "same", 50.0).unwrap();
        let cfg = ABTestConfig::new("test", split, 10);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_routing_distribution() {
        let mut router = ABRouter::new(make_config()).unwrap();
        let mut a_count = 0u64;
        let mut b_count = 0u64;
        for _ in 0..100 {
            let (variant, _) = router.route().unwrap();
            match variant {
                Variant::A => a_count += 1,
                Variant::B => b_count += 1,
            }
        }
        assert_eq!(a_count, 70);
        assert_eq!(b_count, 30);
    }

    #[test]
    fn test_route_with_value() {
        let config = make_config();
        let router = ABRouter::new(config).unwrap();
        let (v, _) = router.route_with_value(30.0).unwrap();
        assert_eq!(v, Variant::A);
        let (v, _) = router.route_with_value(80.0).unwrap();
        assert_eq!(v, Variant::B);
    }

    #[test]
    fn test_routing_disabled() {
        let mut config = make_config();
        config.enabled = false;
        let mut router = ABRouter::new(config).unwrap();
        assert!(router.route().is_err());
    }

    #[test]
    fn test_metric_tracking() {
        let mut router = ABRouter::new(make_config()).unwrap();
        let metric = QualityMetric::new(Duration::from_millis(100), 50.0);
        router.record_metric(&Variant::A, metric);
        assert_eq!(router.metrics_a().len(), 1);
        assert_eq!(router.metrics_b().len(), 0);
    }

    #[test]
    fn test_avg_latency() {
        let mut router = ABRouter::new(make_config()).unwrap();
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 50.0),
        );
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(200), 40.0),
        );
        let avg = router.avg_latency(&Variant::A).unwrap();
        assert_eq!(avg, Duration::from_millis(150));
        assert!(router.avg_latency(&Variant::B).is_none());
    }

    #[test]
    fn test_avg_tokens_per_second() {
        let mut router = ABRouter::new(make_config()).unwrap();
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 40.0),
        );
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 60.0),
        );
        let avg = router.avg_tokens_per_second(&Variant::A).unwrap();
        assert!((avg - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sufficient_samples() {
        let config = ABTestConfig::new("test", make_split(), 2);
        let mut router = ABRouter::new(config).unwrap();
        assert!(!router.has_sufficient_samples());
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 50.0),
        );
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 50.0),
        );
        router.record_metric(
            &Variant::B,
            QualityMetric::new(Duration::from_millis(100), 50.0),
        );
        router.record_metric(
            &Variant::B,
            QualityMetric::new(Duration::from_millis(100), 50.0),
        );
        assert!(router.has_sufficient_samples());
    }

    #[test]
    fn test_rollback_picks_better_model() {
        let config = ABTestConfig::new("test", make_split(), 1);
        let mut router = ABRouter::new(config).unwrap();
        router.record_metric(
            &Variant::A,
            QualityMetric::new(Duration::from_millis(100), 30.0),
        );
        router.record_metric(
            &Variant::B,
            QualityMetric::new(Duration::from_millis(100), 60.0),
        );
        let winner = router.rollback().unwrap();
        assert_eq!(winner, "model-b");
        assert!(!router.config().enabled);
    }

    #[test]
    fn test_rollback_no_data() {
        let mut router = ABRouter::new(make_config()).unwrap();
        assert!(router.rollback().is_none());
    }

    #[test]
    fn test_quality_metric_with_rating() {
        let m = QualityMetric::new(Duration::from_millis(50), 100.0).with_rating(4.5);
        assert_eq!(m.user_rating, Some(4.5));
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = make_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ABTestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-experiment");
        assert_eq!(deserialized.split.percentage_a, 70.0);
    }
}
