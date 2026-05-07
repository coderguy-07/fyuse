//! Automation engine for rule-based device control.

use super::traits::{DeviceCommand, SensorReading};
use serde::{Deserialize, Serialize};

/// Trigger conditions for automation rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trigger {
    /// Fire when a sensor metric crosses a threshold.
    SensorThreshold {
        metric: String,
        above: Option<f64>,
        below: Option<f64>,
    },
    /// Fire on a schedule (cron-like, evaluated externally).
    Schedule { cron: String },
    /// Fire when a device enters a specific state.
    DeviceState { device: String, state: String },
}

/// Conditions that must be true for the rule to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    And(Vec<Condition>),
    Or(Vec<Condition>),
    ValueAbove { metric: String, threshold: f64 },
    ValueBelow { metric: String, threshold: f64 },
    Always,
}

/// Actions to take when a rule fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    SendCommand(DeviceCommand),
    Notify { message: String },
    RunInference { model: String, prompt: String },
}

/// A single automation rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRule {
    pub name: String,
    pub trigger: Trigger,
    pub condition: Condition,
    pub action: Action,
}

/// Engine for evaluating automation rules against sensor data.
pub struct AutomationEngine;

impl AutomationEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a rule against current readings. Returns the action if the rule fires.
    pub fn evaluate(&self, rule: &AutomationRule, readings: &[SensorReading]) -> Option<Action> {
        if !self.trigger_matches(&rule.trigger, readings) {
            return None;
        }
        if !self.condition_met(&rule.condition, readings) {
            return None;
        }
        Some(rule.action.clone())
    }

    fn trigger_matches(&self, trigger: &Trigger, readings: &[SensorReading]) -> bool {
        match trigger {
            Trigger::SensorThreshold {
                metric,
                above,
                below,
            } => readings.iter().any(|r| {
                if r.metric != *metric {
                    return false;
                }
                let above_ok = above.map_or(true, |t| r.value > t);
                let below_ok = below.map_or(true, |t| r.value < t);
                above_ok && below_ok
            }),
            Trigger::Schedule { .. } => {
                // Schedule triggers are evaluated externally; always false here.
                false
            }
            Trigger::DeviceState { .. } => {
                // Device state triggers require external state tracking.
                false
            }
        }
    }

    fn condition_met(&self, condition: &Condition, readings: &[SensorReading]) -> bool {
        Self::evaluate_condition(condition, readings)
    }

    fn evaluate_condition(condition: &Condition, readings: &[SensorReading]) -> bool {
        match condition {
            Condition::Always => true,
            Condition::ValueAbove { metric, threshold } => readings
                .iter()
                .any(|r| r.metric == *metric && r.value > *threshold),
            Condition::ValueBelow { metric, threshold } => readings
                .iter()
                .any(|r| r.metric == *metric && r.value < *threshold),
            Condition::And(conditions) => conditions
                .iter()
                .all(|c| Self::evaluate_condition(c, readings)),
            Condition::Or(conditions) => conditions
                .iter()
                .any(|c| Self::evaluate_condition(c, readings)),
        }
    }
}

impl Default for AutomationEngine {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_sensor_threshold_trigger_fires() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "high-temp-alert".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(30.0),
                below: None,
            },
            condition: Condition::Always,
            action: Action::Notify {
                message: "Temperature is high!".to_string(),
            },
        };
        let readings = vec![make_reading("temperature", 35.0)];
        let result = engine.evaluate(&rule, &readings);
        assert!(result.is_some(), "Rule should fire when temp > 30");
    }

    #[test]
    fn test_sensor_threshold_trigger_no_fire() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "high-temp-alert".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(30.0),
                below: None,
            },
            condition: Condition::Always,
            action: Action::Notify {
                message: "Temperature is high!".to_string(),
            },
        };
        let readings = vec![make_reading("temperature", 25.0)];
        assert!(engine.evaluate(&rule, &readings).is_none());
    }

    #[test]
    fn test_condition_value_above() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "humid-and-hot".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(25.0),
                below: None,
            },
            condition: Condition::ValueAbove {
                metric: "humidity".to_string(),
                threshold: 70.0,
            },
            action: Action::Notify {
                message: "Hot and humid".to_string(),
            },
        };
        let readings = vec![
            make_reading("temperature", 30.0),
            make_reading("humidity", 80.0),
        ];
        assert!(engine.evaluate(&rule, &readings).is_some());

        // humidity too low
        let readings = vec![
            make_reading("temperature", 30.0),
            make_reading("humidity", 50.0),
        ];
        assert!(engine.evaluate(&rule, &readings).is_none());
    }

    #[test]
    fn test_condition_and() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "combined".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(20.0),
                below: None,
            },
            condition: Condition::And(vec![
                Condition::ValueAbove {
                    metric: "temperature".to_string(),
                    threshold: 20.0,
                },
                Condition::ValueBelow {
                    metric: "humidity".to_string(),
                    threshold: 60.0,
                },
            ]),
            action: Action::Notify {
                message: "dry heat".to_string(),
            },
        };
        let readings = vec![
            make_reading("temperature", 35.0),
            make_reading("humidity", 40.0),
        ];
        assert!(engine.evaluate(&rule, &readings).is_some());

        // humidity too high -> And fails
        let readings = vec![
            make_reading("temperature", 35.0),
            make_reading("humidity", 70.0),
        ];
        assert!(engine.evaluate(&rule, &readings).is_none());
    }

    #[test]
    fn test_condition_or() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "either".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(20.0),
                below: None,
            },
            condition: Condition::Or(vec![
                Condition::ValueAbove {
                    metric: "humidity".to_string(),
                    threshold: 90.0,
                },
                Condition::ValueBelow {
                    metric: "pressure".to_string(),
                    threshold: 1000.0,
                },
            ]),
            action: Action::Notify {
                message: "alert".to_string(),
            },
        };
        // Only pressure satisfies
        let readings = vec![
            make_reading("temperature", 25.0),
            make_reading("humidity", 50.0),
            make_reading("pressure", 990.0),
        ];
        assert!(engine.evaluate(&rule, &readings).is_some());
    }

    #[test]
    fn test_schedule_trigger_never_fires_from_evaluate() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "scheduled".to_string(),
            trigger: Trigger::Schedule {
                cron: "0 0 * * *".to_string(),
            },
            condition: Condition::Always,
            action: Action::Notify {
                message: "scheduled".to_string(),
            },
        };
        assert!(engine
            .evaluate(&rule, &[make_reading("temp", 25.0)])
            .is_none());
    }

    #[test]
    fn test_send_command_action() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "auto-ac".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(28.0),
                below: None,
            },
            condition: Condition::Always,
            action: Action::SendCommand(DeviceCommand::Toggle {
                entity_id: "switch.ac".to_string(),
                state: true,
            }),
        };
        let readings = vec![make_reading("temperature", 32.0)];
        let action = engine.evaluate(&rule, &readings);
        assert!(matches!(
            action,
            Some(Action::SendCommand(DeviceCommand::Toggle { .. }))
        ));
    }

    #[test]
    fn test_range_trigger() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "comfort-zone".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temperature".to_string(),
                above: Some(18.0),
                below: Some(26.0),
            },
            condition: Condition::Always,
            action: Action::Notify {
                message: "in comfort zone".to_string(),
            },
        };
        // In range
        assert!(engine
            .evaluate(&rule, &[make_reading("temperature", 22.0)])
            .is_some());
        // Too cold
        assert!(engine
            .evaluate(&rule, &[make_reading("temperature", 15.0)])
            .is_none());
        // Too hot
        assert!(engine
            .evaluate(&rule, &[make_reading("temperature", 30.0)])
            .is_none());
    }

    #[test]
    fn test_no_readings_no_fire() {
        let engine = AutomationEngine::new();
        let rule = AutomationRule {
            name: "test".to_string(),
            trigger: Trigger::SensorThreshold {
                metric: "temp".to_string(),
                above: Some(0.0),
                below: None,
            },
            condition: Condition::Always,
            action: Action::Notify {
                message: "test".to_string(),
            },
        };
        assert!(engine.evaluate(&rule, &[]).is_none());
    }
}
