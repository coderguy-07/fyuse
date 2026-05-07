//! AI-powered data correlation engine for sensor readings.

use super::SensorReading;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An insight derived from correlating sensor data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub description: String,
    pub confidence: f64,
    pub data_points: usize,
    pub suggestion: String,
}

/// Correlates sensor readings to produce actionable insights.
pub struct DataCorrelator;

impl DataCorrelator {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a set of sensor readings and produce insights.
    pub fn correlate(&self, readings: &[SensorReading]) -> Vec<Insight> {
        let mut insights = Vec::new();

        if readings.is_empty() {
            return insights;
        }

        // Group readings by metric
        let mut by_metric: HashMap<&str, Vec<f64>> = HashMap::new();
        for r in readings {
            by_metric
                .entry(r.metric.as_str())
                .or_default()
                .push(r.value);
        }

        // Compute averages and detect anomalies per metric
        for (metric, values) in &by_metric {
            if values.is_empty() {
                continue;
            }

            let avg = values.iter().sum::<f64>() / values.len() as f64;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = max - min;

            // High variance detection
            if values.len() >= 2 && range > avg * 0.5 && avg.abs() > f64::EPSILON {
                insights.push(Insight {
                    description: format!(
                        "High variance in {metric}: range {range:.1} (min {min:.1}, max {max:.1})"
                    ),
                    confidence: 0.7,
                    data_points: values.len(),
                    suggestion: format!("Investigate fluctuations in {metric} readings"),
                });
            }

            // Trend detection (simple linear: compare first half avg vs second half avg)
            if values.len() >= 4 {
                let mid = values.len() / 2;
                let first_avg = values[..mid].iter().sum::<f64>() / mid as f64;
                let second_avg = values[mid..].iter().sum::<f64>() / (values.len() - mid) as f64;
                let diff = second_avg - first_avg;

                if diff.abs() > avg.abs() * 0.1 && avg.abs() > f64::EPSILON {
                    let direction = if diff > 0.0 {
                        "increasing"
                    } else {
                        "decreasing"
                    };
                    insights.push(Insight {
                        description: format!("{metric} is {direction} (avg shifted by {diff:.1})"),
                        confidence: 0.6,
                        data_points: values.len(),
                        suggestion: format!(
                            "Monitor {metric} trend; consider adjusting thresholds"
                        ),
                    });
                }
            }
        }

        // Cross-metric correlation (simple Pearson between pairs)
        let metrics: Vec<&str> = by_metric.keys().copied().collect();
        for i in 0..metrics.len() {
            for j in (i + 1)..metrics.len() {
                let a = &by_metric[metrics[i]];
                let b = &by_metric[metrics[j]];
                let n = a.len().min(b.len());
                if n < 3 {
                    continue;
                }
                if let Some(corr) = pearson_correlation(&a[..n], &b[..n]) {
                    if corr.abs() > 0.7 {
                        let rel = if corr > 0.0 {
                            "positively correlated"
                        } else {
                            "negatively correlated"
                        };
                        insights.push(Insight {
                            description: format!(
                                "{} and {} are {} (r={corr:.2})",
                                metrics[i], metrics[j], rel
                            ),
                            confidence: corr.abs(),
                            data_points: n,
                            suggestion: format!(
                                "Changes in {} may affect {}",
                                metrics[i], metrics[j]
                            ),
                        });
                    }
                }
            }
        }

        insights
    }
}

impl Default for DataCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

fn pearson_correlation(a: &[f64], b: &[f64]) -> Option<f64> {
    let n = a.len();
    if n < 2 {
        return None;
    }
    let n_f = n as f64;
    let mean_a = a.iter().sum::<f64>() / n_f;
    let mean_b = b.iter().sum::<f64>() / n_f;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < f64::EPSILON {
        return None;
    }
    Some(cov / denom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_reading(metric: &str, value: f64) -> SensorReading {
        SensorReading {
            device_name: "test".to_string(),
            metric: metric.to_string(),
            value,
            unit: "unit".to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_empty_readings_no_insights() {
        let c = DataCorrelator::new();
        assert!(c.correlate(&[]).is_empty());
    }

    #[test]
    fn test_high_variance_detected() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 10.0),
            make_reading("temp", 30.0),
            make_reading("temp", 12.0),
            make_reading("temp", 28.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            insights.iter().any(|i| i.description.contains("variance")),
            "Expected variance insight, got: {insights:?}"
        );
    }

    #[test]
    fn test_trend_detection_increasing() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 10.0),
            make_reading("temp", 11.0),
            make_reading("temp", 20.0),
            make_reading("temp", 21.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            insights
                .iter()
                .any(|i| i.description.contains("increasing")),
            "Expected increasing trend, got: {insights:?}"
        );
    }

    #[test]
    fn test_trend_detection_decreasing() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 30.0),
            make_reading("temp", 28.0),
            make_reading("temp", 15.0),
            make_reading("temp", 14.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            insights
                .iter()
                .any(|i| i.description.contains("decreasing")),
            "Expected decreasing trend, got: {insights:?}"
        );
    }

    #[test]
    fn test_positive_correlation() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 10.0),
            make_reading("humidity", 40.0),
            make_reading("temp", 20.0),
            make_reading("humidity", 60.0),
            make_reading("temp", 30.0),
            make_reading("humidity", 80.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            insights
                .iter()
                .any(|i| i.description.contains("positively correlated")),
            "Expected positive correlation, got: {insights:?}"
        );
    }

    #[test]
    fn test_negative_correlation() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 10.0),
            make_reading("pressure", 80.0),
            make_reading("temp", 20.0),
            make_reading("pressure", 60.0),
            make_reading("temp", 30.0),
            make_reading("pressure", 40.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            insights
                .iter()
                .any(|i| i.description.contains("negatively correlated")),
            "Expected negative correlation, got: {insights:?}"
        );
    }

    #[test]
    fn test_pearson_perfect_correlation() {
        let r = pearson_correlation(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]);
        assert!((r.expect("should compute") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pearson_no_variance() {
        let r = pearson_correlation(&[5.0, 5.0, 5.0], &[1.0, 2.0, 3.0]);
        assert!(r.is_none());
    }

    #[test]
    fn test_stable_readings_no_variance_insight() {
        let c = DataCorrelator::new();
        let readings: Vec<SensorReading> = vec![
            make_reading("temp", 20.0),
            make_reading("temp", 20.1),
            make_reading("temp", 19.9),
            make_reading("temp", 20.0),
        ];
        let insights = c.correlate(&readings);
        assert!(
            !insights.iter().any(|i| i.description.contains("variance")),
            "Stable data should not trigger variance insight"
        );
    }
}
