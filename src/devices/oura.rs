//! Oura Ring connector (feature: oura).

use super::traits::{DeviceCommand, DeviceType, SensorReading};
use crate::error::{FuseError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for the Oura Ring connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuraConfig {
    pub api_token: String,
    pub sync_interval_secs: u64,
}

impl Default for OuraConfig {
    fn default() -> Self {
        Self {
            api_token: String::new(),
            sync_interval_secs: 900,
        }
    }
}

/// Sleep data from Oura Ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepData {
    pub total_sleep_secs: u64,
    pub deep_sleep_secs: u64,
    pub rem_sleep_secs: u64,
    pub efficiency: f64,
    pub date: String,
}

/// Readiness data from Oura Ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessData {
    pub score: u32,
    pub temperature_deviation: f64,
    pub hrv_balance: f64,
    pub date: String,
}

/// Activity data from Oura Ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityData {
    pub steps: u64,
    pub active_calories: u64,
    pub total_calories: u64,
    pub date: String,
}

/// Oura Ring connector implementing DeviceConnector.
pub struct OuraConnector {
    config: OuraConfig,
    connected: bool,
    /// Cached data (populated by sync or mock).
    sleep_data: Vec<SleepData>,
    readiness_data: Vec<ReadinessData>,
    activity_data: Vec<ActivityData>,
    last_sync: Option<DateTime<Utc>>,
}

impl OuraConnector {
    pub fn new(config: OuraConfig) -> Self {
        Self {
            config,
            connected: false,
            sleep_data: Vec::new(),
            readiness_data: Vec::new(),
            activity_data: Vec::new(),
            last_sync: None,
        }
    }

    /// Inject mock data for testing.
    #[cfg(test)]
    pub fn inject_sleep(&mut self, data: SleepData) {
        self.sleep_data.push(data);
    }

    #[cfg(test)]
    pub fn inject_readiness(&mut self, data: ReadinessData) {
        self.readiness_data.push(data);
    }

    #[cfg(test)]
    pub fn inject_activity(&mut self, data: ActivityData) {
        self.activity_data.push(data);
    }

    fn to_sensor_readings(&self) -> Vec<SensorReading> {
        let now = Utc::now();
        let mut readings = Vec::new();

        for s in &self.sleep_data {
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "sleep_total".to_string(),
                value: s.total_sleep_secs as f64,
                unit: "seconds".to_string(),
                timestamp: now,
            });
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "sleep_efficiency".to_string(),
                value: s.efficiency,
                unit: "%".to_string(),
                timestamp: now,
            });
        }

        for r in &self.readiness_data {
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "readiness_score".to_string(),
                value: r.score as f64,
                unit: "score".to_string(),
                timestamp: now,
            });
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "hrv_balance".to_string(),
                value: r.hrv_balance,
                unit: "ms".to_string(),
                timestamp: now,
            });
        }

        for a in &self.activity_data {
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "steps".to_string(),
                value: a.steps as f64,
                unit: "count".to_string(),
                timestamp: now,
            });
            readings.push(SensorReading {
                device_name: "oura-ring".to_string(),
                metric: "active_calories".to_string(),
                value: a.active_calories as f64,
                unit: "kcal".to_string(),
                timestamp: now,
            });
        }

        readings
    }
}

#[async_trait]
impl super::traits::DeviceConnector for OuraConnector {
    fn name(&self) -> &str {
        "oura-ring"
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Wearable
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.api_token.is_empty() {
            return Err(FuseError::DeviceError {
                device: "oura-ring".to_string(),
                message: "api_token cannot be empty".to_string(),
            });
        }
        // In a real implementation, validate the token against the Oura API.
        self.connected = true;
        self.last_sync = Some(Utc::now());
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn read_data(&self) -> Result<Vec<SensorReading>> {
        if !self.connected {
            return Err(FuseError::DeviceError {
                device: "oura-ring".to_string(),
                message: "not connected".to_string(),
            });
        }
        Ok(self.to_sensor_readings())
    }

    async fn send_command(&self, _command: DeviceCommand) -> Result<()> {
        Err(FuseError::DeviceError {
            device: "oura-ring".to_string(),
            message: "Oura Ring does not accept commands".to_string(),
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::super::traits::DeviceConnector;
    use super::*;

    fn test_config() -> OuraConfig {
        OuraConfig {
            api_token: "test-token-123".to_string(),
            sync_interval_secs: 60,
        }
    }

    #[tokio::test]
    async fn test_oura_connect_disconnect() {
        let mut conn = OuraConnector::new(test_config());
        assert!(!conn.is_connected());
        conn.connect().await.expect("connect failed");
        assert!(conn.is_connected());
        conn.disconnect().await.expect("disconnect failed");
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_oura_connect_empty_token_fails() {
        let mut conn = OuraConnector::new(OuraConfig::default());
        assert!(conn.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_oura_read_not_connected() {
        let conn = OuraConnector::new(test_config());
        assert!(conn.read_data().await.is_err());
    }

    #[tokio::test]
    async fn test_oura_send_command_fails() {
        let mut conn = OuraConnector::new(test_config());
        conn.connect().await.expect("connect");
        let result = conn
            .send_command(DeviceCommand::Toggle {
                entity_id: "x".to_string(),
                state: true,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_oura_sleep_data_to_readings() {
        let mut conn = OuraConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_sleep(SleepData {
            total_sleep_secs: 28800,
            deep_sleep_secs: 7200,
            rem_sleep_secs: 5400,
            efficiency: 92.0,
            date: "2026-04-06".to_string(),
        });
        let readings = conn.read_data().await.expect("read");
        assert_eq!(readings.len(), 2);
        assert!(readings.iter().any(|r| r.metric == "sleep_total"));
        assert!(readings.iter().any(|r| r.metric == "sleep_efficiency"));
    }

    #[tokio::test]
    async fn test_oura_readiness_data_to_readings() {
        let mut conn = OuraConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_readiness(ReadinessData {
            score: 85,
            temperature_deviation: 0.1,
            hrv_balance: 45.0,
            date: "2026-04-06".to_string(),
        });
        let readings = conn.read_data().await.expect("read");
        assert!(readings.iter().any(|r| r.metric == "readiness_score"));
        assert!(readings.iter().any(|r| r.metric == "hrv_balance"));
    }

    #[tokio::test]
    async fn test_oura_activity_data_to_readings() {
        let mut conn = OuraConnector::new(test_config());
        conn.connect().await.expect("connect");
        conn.inject_activity(ActivityData {
            steps: 10000,
            active_calories: 350,
            total_calories: 2200,
            date: "2026-04-06".to_string(),
        });
        let readings = conn.read_data().await.expect("read");
        assert!(readings.iter().any(|r| r.metric == "steps"));
        assert!(readings.iter().any(|r| r.metric == "active_calories"));
    }

    #[test]
    fn test_oura_name_and_type() {
        let conn = OuraConnector::new(test_config());
        assert_eq!(conn.name(), "oura-ring");
        assert_eq!(conn.device_type(), DeviceType::Wearable);
    }

    #[test]
    fn test_sleep_data_serialization() {
        let data = SleepData {
            total_sleep_secs: 28800,
            deep_sleep_secs: 7200,
            rem_sleep_secs: 5400,
            efficiency: 92.0,
            date: "2026-04-06".to_string(),
        };
        let json = serde_json::to_string(&data).expect("serialize");
        let deser: SleepData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.total_sleep_secs, 28800);
    }
}
