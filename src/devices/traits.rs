//! DeviceConnector trait — abstraction for hardware/wearable/IoT connectors.

use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Wearable,
    SmartHome,
    Sensor,
    HealthKit,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub enabled: bool,
    pub sync_interval_secs: u64,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sync_interval_secs: 300,
            api_key: None,
            endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQuery {
    pub metric: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceData {
    pub metric: String,
    pub values: Vec<f64>,
    pub timestamps: Vec<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub device: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// A single sensor reading with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub device_name: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
}

/// Commands that can be sent to devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceCommand {
    /// Toggle a device on/off.
    Toggle { entity_id: String, state: bool },
    /// Set a numeric value (e.g., thermostat temperature).
    SetValue { entity_id: String, value: f64 },
    /// Send a raw payload.
    Raw {
        topic: String,
        payload: serde_json::Value,
    },
}

/// Core trait for device connectors.
#[async_trait]
pub trait DeviceConnector: Send + Sync {
    fn name(&self) -> &str;
    fn device_type(&self) -> DeviceType;
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn read_data(&self) -> Result<Vec<SensorReading>>;
    async fn send_command(&self, command: DeviceCommand) -> Result<()>;
    fn is_connected(&self) -> bool;
    /// Legacy query-based read.
    async fn read_data_query(&self, _query: &DataQuery) -> Result<DeviceData> {
        Err(crate::error::FuseError::DeviceError {
            device: self.name().to_string(),
            message: "query-based read not supported".to_string(),
        })
    }
    /// Subscribe to device events.
    fn subscribe(&self) -> BoxStream<'_, DeviceEvent> {
        Box::pin(futures::stream::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDevice {
        connected: bool,
    }

    impl MockDevice {
        fn new() -> Self {
            Self { connected: false }
        }
    }

    #[async_trait]
    impl DeviceConnector for MockDevice {
        fn name(&self) -> &str {
            "mock-sensor"
        }
        fn device_type(&self) -> DeviceType {
            DeviceType::Sensor
        }
        async fn connect(&mut self) -> Result<()> {
            self.connected = true;
            Ok(())
        }
        async fn disconnect(&mut self) -> Result<()> {
            self.connected = false;
            Ok(())
        }
        async fn read_data(&self) -> Result<Vec<SensorReading>> {
            Ok(vec![
                SensorReading {
                    device_name: "mock-sensor".to_string(),
                    metric: "temperature".to_string(),
                    value: 23.5,
                    unit: "°C".to_string(),
                    timestamp: chrono::Utc::now(),
                },
                SensorReading {
                    device_name: "mock-sensor".to_string(),
                    metric: "humidity".to_string(),
                    value: 55.0,
                    unit: "%".to_string(),
                    timestamp: chrono::Utc::now(),
                },
            ])
        }
        async fn send_command(&self, _command: DeviceCommand) -> Result<()> {
            Ok(())
        }
        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    #[tokio::test]
    async fn test_device_lifecycle() {
        let mut dev = MockDevice::new();
        assert_eq!(dev.name(), "mock-sensor");
        assert_eq!(dev.device_type(), DeviceType::Sensor);
        assert!(!dev.is_connected());

        dev.connect().await.expect("connect failed");
        assert!(dev.is_connected());

        let readings = dev.read_data().await.expect("read_data failed");
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].metric, "temperature");

        dev.send_command(DeviceCommand::Toggle {
            entity_id: "light.1".to_string(),
            state: true,
        })
        .await
        .expect("send_command failed");

        dev.disconnect().await.expect("disconnect failed");
        assert!(!dev.is_connected());
    }

    #[tokio::test]
    async fn test_sensor_reading_serialization() {
        let reading = SensorReading {
            device_name: "test".to_string(),
            metric: "temp".to_string(),
            value: 22.5,
            unit: "C".to_string(),
            timestamp: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&reading).expect("serialize failed");
        let deser: SensorReading = serde_json::from_str(&json).expect("deserialize failed");
        assert!((deser.value - 22.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_device_command_variants() {
        let toggle = DeviceCommand::Toggle {
            entity_id: "switch.1".to_string(),
            state: true,
        };
        let json = serde_json::to_string(&toggle).expect("serialize failed");
        assert!(json.contains("Toggle"));

        let set_val = DeviceCommand::SetValue {
            entity_id: "thermostat.1".to_string(),
            value: 21.0,
        };
        let json = serde_json::to_string(&set_val).expect("serialize failed");
        assert!(json.contains("SetValue"));
    }
}
