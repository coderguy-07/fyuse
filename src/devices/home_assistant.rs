//! Home Assistant connector (feature: home-assistant).

use super::traits::{DeviceCommand, DeviceType, SensorReading};
use crate::error::{FuseError, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Configuration for the Home Assistant connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeAssistantConfig {
    pub url: String,
    pub access_token: String,
    pub entities: Vec<String>,
}

impl Default for HomeAssistantConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8123".to_string(),
            access_token: String::new(),
            entities: Vec::new(),
        }
    }
}

/// Represents a Home Assistant entity state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_id: String,
    pub state: String,
    pub attributes: serde_json::Value,
}

/// Home Assistant connector implementing DeviceConnector.
pub struct HomeAssistantConnector {
    config: HomeAssistantConfig,
    connected: bool,
    /// Cached entity states (populated by sync or mock).
    entity_states: Vec<EntityState>,
}

impl HomeAssistantConnector {
    pub fn new(config: HomeAssistantConfig) -> Self {
        Self {
            config,
            connected: false,
            entity_states: Vec::new(),
        }
    }

    /// Inject mock entity state for testing.
    #[cfg(test)]
    pub fn inject_state(&mut self, state: EntityState) {
        self.entity_states.push(state);
    }

    fn states_to_readings(&self) -> Vec<SensorReading> {
        let now = Utc::now();
        let mut readings = Vec::new();

        for es in &self.entity_states {
            // Try to parse numeric state
            if let Ok(val) = es.state.parse::<f64>() {
                let unit = es
                    .attributes
                    .get("unit_of_measurement")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();
                readings.push(SensorReading {
                    device_name: "home-assistant".to_string(),
                    metric: es.entity_id.clone(),
                    value: val,
                    unit,
                    timestamp: now,
                });
            } else {
                // Map on/off to 1.0/0.0
                let val = match es.state.as_str() {
                    "on" => 1.0,
                    "off" => 0.0,
                    _ => continue,
                };
                readings.push(SensorReading {
                    device_name: "home-assistant".to_string(),
                    metric: es.entity_id.clone(),
                    value: val,
                    unit: "state".to_string(),
                    timestamp: now,
                });
            }
        }

        readings
    }
}

#[async_trait]
impl super::traits::DeviceConnector for HomeAssistantConnector {
    fn name(&self) -> &str {
        "home-assistant"
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::SmartHome
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.access_token.is_empty() {
            return Err(FuseError::DeviceError {
                device: "home-assistant".to_string(),
                message: "access_token cannot be empty".to_string(),
            });
        }
        if self.config.url.is_empty() {
            return Err(FuseError::DeviceError {
                device: "home-assistant".to_string(),
                message: "url cannot be empty".to_string(),
            });
        }
        // In a real implementation, validate the connection to the HA API.
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        self.entity_states.clear();
        Ok(())
    }

    async fn read_data(&self) -> Result<Vec<SensorReading>> {
        if !self.connected {
            return Err(FuseError::DeviceError {
                device: "home-assistant".to_string(),
                message: "not connected".to_string(),
            });
        }
        Ok(self.states_to_readings())
    }

    async fn send_command(&self, command: DeviceCommand) -> Result<()> {
        if !self.connected {
            return Err(FuseError::DeviceError {
                device: "home-assistant".to_string(),
                message: "not connected".to_string(),
            });
        }
        // In a real implementation, call the HA REST API.
        match &command {
            DeviceCommand::Toggle { entity_id, state } => {
                tracing::info!(entity = %entity_id, state = %state, "HA toggle service");
            }
            DeviceCommand::SetValue { entity_id, value } => {
                tracing::info!(entity = %entity_id, value = %value, "HA set value");
            }
            DeviceCommand::Raw { topic, payload } => {
                tracing::info!(topic = %topic, payload = %payload, "HA raw command");
            }
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::super::traits::DeviceConnector;
    use super::*;

    fn test_config() -> HomeAssistantConfig {
        HomeAssistantConfig {
            url: "http://localhost:8123".to_string(),
            access_token: "test-token".to_string(),
            entities: vec!["sensor.temperature".to_string(), "switch.light".to_string()],
        }
    }

    #[tokio::test]
    async fn test_ha_connect_disconnect() {
        let mut conn = HomeAssistantConnector::new(test_config());
        assert!(!conn.is_connected());
        conn.connect().await.expect("connect");
        assert!(conn.is_connected());
        conn.disconnect().await.expect("disconnect");
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_ha_connect_empty_token() {
        let mut conn = HomeAssistantConnector::new(HomeAssistantConfig::default());
        assert!(conn.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_ha_read_not_connected() {
        let conn = HomeAssistantConnector::new(test_config());
        assert!(conn.read_data().await.is_err());
    }

    #[tokio::test]
    async fn test_ha_numeric_entity_state() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_state(EntityState {
            entity_id: "sensor.temperature".to_string(),
            state: "22.5".to_string(),
            attributes: serde_json::json!({"unit_of_measurement": "°C"}),
        });
        let readings = conn.read_data().await.expect("read");
        assert_eq!(readings.len(), 1);
        assert!((readings[0].value - 22.5).abs() < f64::EPSILON);
        assert_eq!(readings[0].unit, "°C");
    }

    #[tokio::test]
    async fn test_ha_boolean_entity_state() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_state(EntityState {
            entity_id: "switch.light".to_string(),
            state: "on".to_string(),
            attributes: serde_json::json!({}),
        });
        conn.inject_state(EntityState {
            entity_id: "switch.fan".to_string(),
            state: "off".to_string(),
            attributes: serde_json::json!({}),
        });
        let readings = conn.read_data().await.expect("read");
        assert_eq!(readings.len(), 2);
        assert!((readings[0].value - 1.0).abs() < f64::EPSILON);
        assert!(readings[1].value.abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_ha_unknown_state_skipped() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_state(EntityState {
            entity_id: "sensor.status".to_string(),
            state: "unavailable".to_string(),
            attributes: serde_json::json!({}),
        });
        let readings = conn.read_data().await.expect("read");
        assert!(readings.is_empty());
    }

    #[tokio::test]
    async fn test_ha_send_toggle() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.send_command(DeviceCommand::Toggle {
            entity_id: "switch.light".to_string(),
            state: true,
        })
        .await
        .expect("send");
    }

    #[tokio::test]
    async fn test_ha_send_set_value() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.send_command(DeviceCommand::SetValue {
            entity_id: "climate.thermostat".to_string(),
            value: 21.5,
        })
        .await
        .expect("send");
    }

    #[tokio::test]
    async fn test_ha_send_not_connected() {
        let conn = HomeAssistantConnector::new(test_config());
        assert!(conn
            .send_command(DeviceCommand::Toggle {
                entity_id: "x".to_string(),
                state: true,
            })
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_ha_disconnect_clears_state() {
        let mut conn = HomeAssistantConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_state(EntityState {
            entity_id: "sensor.temp".to_string(),
            state: "20.0".to_string(),
            attributes: serde_json::json!({}),
        });
        conn.disconnect().await.expect("disconnect");
        conn.connect().await.expect("reconnect");
        let readings = conn.read_data().await.expect("read");
        assert!(readings.is_empty());
    }

    #[test]
    fn test_ha_name_and_type() {
        let conn = HomeAssistantConnector::new(test_config());
        assert_eq!(conn.name(), "home-assistant");
        assert_eq!(conn.device_type(), DeviceType::SmartHome);
    }
}
