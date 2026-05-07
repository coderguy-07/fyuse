//! MQTT gateway for generic sensor data (feature: mqtt-devices).

use super::traits::{DeviceCommand, DeviceType, SensorReading};
use crate::error::{FuseError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

/// Configuration for the MQTT gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topics: Vec<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "fuse-mqtt".to_string(),
            topics: vec!["sensors/#".to_string()],
            username: None,
            password: None,
        }
    }
}

/// MQTT gateway implementing DeviceConnector.
pub struct MqttGateway {
    config: MqttConfig,
    connected: AtomicBool,
    /// Buffered readings (in a real impl, populated by the MQTT client callback).
    readings: parking_lot::Mutex<Vec<SensorReading>>,
}

impl MqttGateway {
    pub fn new(config: MqttConfig) -> Self {
        Self {
            config,
            connected: AtomicBool::new(false),
            readings: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Inject a reading (used for testing and by the MQTT callback).
    pub fn inject_reading(&self, reading: SensorReading) {
        self.readings.lock().push(reading);
    }
}

#[async_trait]
impl super::traits::DeviceConnector for MqttGateway {
    fn name(&self) -> &str {
        &self.config.client_id
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Sensor
    }

    async fn connect(&mut self) -> Result<()> {
        // In a real implementation, this would use rumqttc to connect to the broker.
        // For now, validate config and set connected.
        if self.config.broker_url.is_empty() {
            return Err(FuseError::DeviceError {
                device: self.config.client_id.clone(),
                message: "broker_url cannot be empty".to_string(),
            });
        }
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        self.readings.lock().clear();
        Ok(())
    }

    async fn read_data(&self) -> Result<Vec<SensorReading>> {
        if !self.is_connected() {
            return Err(FuseError::DeviceError {
                device: self.config.client_id.clone(),
                message: "not connected".to_string(),
            });
        }
        Ok(self.readings.lock().clone())
    }

    async fn send_command(&self, command: DeviceCommand) -> Result<()> {
        if !self.is_connected() {
            return Err(FuseError::DeviceError {
                device: self.config.client_id.clone(),
                message: "not connected".to_string(),
            });
        }
        // In a real implementation, publish the command to the appropriate topic.
        match &command {
            DeviceCommand::Raw { topic, .. } => {
                tracing::info!(topic = %topic, "MQTT publish command");
            }
            DeviceCommand::Toggle { entity_id, state } => {
                tracing::info!(entity = %entity_id, state = %state, "MQTT toggle");
            }
            DeviceCommand::SetValue { entity_id, value } => {
                tracing::info!(entity = %entity_id, value = %value, "MQTT set value");
            }
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::super::traits::DeviceConnector;
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_mqtt_connect_disconnect() {
        let mut gw = MqttGateway::new(MqttConfig::default());
        assert!(!gw.is_connected());

        gw.connect().await.expect("connect failed");
        assert!(gw.is_connected());

        gw.disconnect().await.expect("disconnect failed");
        assert!(!gw.is_connected());
    }

    #[tokio::test]
    async fn test_mqtt_connect_empty_url_fails() {
        let mut gw = MqttGateway::new(MqttConfig {
            broker_url: String::new(),
            ..MqttConfig::default()
        });
        let result = gw.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mqtt_read_data_not_connected() {
        let gw = MqttGateway::new(MqttConfig::default());
        let result = gw.read_data().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mqtt_inject_and_read() {
        let mut gw = MqttGateway::new(MqttConfig::default());
        gw.connect().await.expect("connect failed");

        gw.inject_reading(SensorReading {
            device_name: "sensor1".to_string(),
            metric: "temperature".to_string(),
            value: 22.5,
            unit: "C".to_string(),
            timestamp: Utc::now(),
        });

        let readings = gw.read_data().await.expect("read failed");
        assert_eq!(readings.len(), 1);
        assert!((readings[0].value - 22.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_mqtt_send_command_not_connected() {
        let gw = MqttGateway::new(MqttConfig::default());
        let result = gw
            .send_command(DeviceCommand::Raw {
                topic: "test".to_string(),
                payload: serde_json::json!({}),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mqtt_send_command_connected() {
        let mut gw = MqttGateway::new(MqttConfig::default());
        gw.connect().await.expect("connect failed");
        gw.send_command(DeviceCommand::Toggle {
            entity_id: "light.1".to_string(),
            state: true,
        })
        .await
        .expect("send failed");
    }

    #[tokio::test]
    async fn test_mqtt_disconnect_clears_readings() {
        let mut gw = MqttGateway::new(MqttConfig::default());
        gw.connect().await.expect("connect failed");
        gw.inject_reading(SensorReading {
            device_name: "s1".to_string(),
            metric: "temp".to_string(),
            value: 20.0,
            unit: "C".to_string(),
            timestamp: Utc::now(),
        });
        gw.disconnect().await.expect("disconnect failed");
        // After reconnect, readings should be empty
        gw.connect().await.expect("reconnect failed");
        let readings = gw.read_data().await.expect("read failed");
        assert!(readings.is_empty());
    }

    #[test]
    fn test_mqtt_config_default() {
        let cfg = MqttConfig::default();
        assert_eq!(cfg.port, 1883);
        assert_eq!(cfg.broker_url, "localhost");
    }

    #[test]
    fn test_mqtt_name_and_type() {
        let gw = MqttGateway::new(MqttConfig {
            client_id: "my-gateway".to_string(),
            ..MqttConfig::default()
        });
        assert_eq!(gw.name(), "my-gateway");
        assert_eq!(gw.device_type(), DeviceType::Sensor);
    }
}
