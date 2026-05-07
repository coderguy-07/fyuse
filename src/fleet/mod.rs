//! Edge fleet management [9.6] — deploy models to N devices from one control plane.
//!
//! Manages a fleet of Fuse edge devices, enabling model deployment,
//! health monitoring, and rolling updates.

use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A registered edge device in the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetDevice {
    pub id: String,
    pub name: String,
    pub address: String,
    pub status: DeviceStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub models: Vec<String>,
    pub hardware: DeviceHardware,
}

/// Device health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Online,
    Offline,
    Deploying,
    Error,
}

/// Hardware profile of a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHardware {
    pub cpu_cores: u32,
    pub ram_mb: u64,
    pub gpu: Option<String>,
}

/// A deployment request for the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRequest {
    pub model_name: String,
    pub target_devices: Vec<String>,
    pub strategy: DeployStrategy,
}

/// Deployment strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployStrategy {
    /// Deploy to all targets simultaneously.
    AllAtOnce,
    /// Rolling update — one at a time.
    Rolling,
    /// Canary — deploy to first device, verify, then proceed.
    Canary,
}

/// Status of a deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStatus {
    pub id: String,
    pub model_name: String,
    pub total_devices: usize,
    pub deployed: usize,
    pub failed: usize,
    pub in_progress: usize,
    pub phase: DeployPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployPhase {
    Pending,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

/// Fleet manager — coordinates device registration, deployment, and health.
pub struct FleetManager {
    devices: HashMap<String, FleetDevice>,
    deployments: Vec<DeploymentStatus>,
    heartbeat_timeout_secs: u64,
}

impl FleetManager {
    pub fn new(heartbeat_timeout_secs: u64) -> Self {
        Self {
            devices: HashMap::new(),
            deployments: Vec::new(),
            heartbeat_timeout_secs,
        }
    }

    /// Register a device.
    pub fn register_device(&mut self, device: FleetDevice) -> Result<()> {
        if device.id.is_empty() {
            return Err(FuseError::ValidationError(
                "Device ID cannot be empty".into(),
            ));
        }
        self.devices.insert(device.id.clone(), device);
        Ok(())
    }

    /// Remove a device.
    pub fn remove_device(&mut self, device_id: &str) -> bool {
        self.devices.remove(device_id).is_some()
    }

    /// Get a device by ID.
    pub fn get_device(&self, device_id: &str) -> Option<&FleetDevice> {
        self.devices.get(device_id)
    }

    /// List all devices.
    pub fn list_devices(&self) -> Vec<&FleetDevice> {
        self.devices.values().collect()
    }

    /// List online devices.
    pub fn online_devices(&self) -> Vec<&FleetDevice> {
        self.devices
            .values()
            .filter(|d| d.status == DeviceStatus::Online)
            .collect()
    }

    /// Record a heartbeat from a device.
    pub fn heartbeat(&mut self, device_id: &str) -> Result<()> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or_else(|| FuseError::ValidationError(format!("Device not found: {device_id}")))?;
        device.last_heartbeat = Utc::now();
        device.status = DeviceStatus::Online;
        Ok(())
    }

    /// Check for offline devices (no heartbeat within timeout).
    pub fn check_health(&mut self) -> Vec<String> {
        let timeout = self.heartbeat_timeout_secs as i64;
        let now = Utc::now();
        let mut offline = Vec::new();

        for device in self.devices.values_mut() {
            if device.status == DeviceStatus::Online {
                let elapsed = now
                    .signed_duration_since(device.last_heartbeat)
                    .num_seconds();
                if elapsed > timeout {
                    device.status = DeviceStatus::Offline;
                    offline.push(device.id.clone());
                }
            }
        }

        offline
    }

    /// Create a deployment.
    pub fn create_deployment(&mut self, req: &DeploymentRequest) -> Result<DeploymentStatus> {
        if req.model_name.is_empty() {
            return Err(FuseError::ValidationError(
                "Model name cannot be empty".into(),
            ));
        }
        if req.target_devices.is_empty() {
            return Err(FuseError::ValidationError(
                "Target devices cannot be empty".into(),
            ));
        }

        // Verify all target devices exist
        for device_id in &req.target_devices {
            if !self.devices.contains_key(device_id) {
                return Err(FuseError::ValidationError(format!(
                    "Device not found: {device_id}"
                )));
            }
        }

        let status = DeploymentStatus {
            id: uuid::Uuid::new_v4().to_string(),
            model_name: req.model_name.clone(),
            total_devices: req.target_devices.len(),
            deployed: 0,
            failed: 0,
            in_progress: req.target_devices.len(),
            phase: DeployPhase::InProgress,
        };

        // Mark devices as deploying
        for device_id in &req.target_devices {
            if let Some(device) = self.devices.get_mut(device_id) {
                device.status = DeviceStatus::Deploying;
            }
        }

        self.deployments.push(status.clone());
        Ok(status)
    }

    /// Get device count.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get deployment count.
    pub fn deployment_count(&self) -> usize {
        self.deployments.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_device(id: &str) -> FleetDevice {
        FleetDevice {
            id: id.to_string(),
            name: format!("device-{id}"),
            address: format!("192.168.1.{id}"),
            status: DeviceStatus::Online,
            last_heartbeat: Utc::now(),
            models: vec![],
            hardware: DeviceHardware {
                cpu_cores: 4,
                ram_mb: 8192,
                gpu: None,
            },
        }
    }

    fn make_manager() -> FleetManager {
        FleetManager::new(60)
    }

    #[test]
    fn test_register_device() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        assert_eq!(fm.device_count(), 1);
    }

    #[test]
    fn test_register_empty_id_fails() {
        let mut fm = make_manager();
        let mut d = make_device("1");
        d.id = String::new();
        assert!(fm.register_device(d).is_err());
    }

    #[test]
    fn test_remove_device() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        assert!(fm.remove_device("1"));
        assert!(!fm.remove_device("1"));
        assert_eq!(fm.device_count(), 0);
    }

    #[test]
    fn test_get_device() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        assert!(fm.get_device("1").is_some());
        assert!(fm.get_device("99").is_none());
    }

    #[test]
    fn test_list_and_online_devices() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        let mut d2 = make_device("2");
        d2.status = DeviceStatus::Offline;
        fm.register_device(d2).unwrap();

        assert_eq!(fm.list_devices().len(), 2);
        assert_eq!(fm.online_devices().len(), 1);
    }

    #[test]
    fn test_heartbeat() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        assert!(fm.heartbeat("1").is_ok());
        assert!(fm.heartbeat("99").is_err());
    }

    #[test]
    fn test_check_health_marks_offline() {
        let mut fm = make_manager();
        let mut d = make_device("1");
        d.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        fm.register_device(d).unwrap();

        let offline = fm.check_health();
        assert_eq!(offline.len(), 1);
        assert_eq!(fm.get_device("1").unwrap().status, DeviceStatus::Offline);
    }

    #[test]
    fn test_create_deployment() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        fm.register_device(make_device("2")).unwrap();

        let req = DeploymentRequest {
            model_name: "llama3:7b".into(),
            target_devices: vec!["1".into(), "2".into()],
            strategy: DeployStrategy::Rolling,
        };

        let status = fm.create_deployment(&req).unwrap();
        assert_eq!(status.total_devices, 2);
        assert_eq!(status.phase, DeployPhase::InProgress);
        assert_eq!(fm.deployment_count(), 1);
    }

    #[test]
    fn test_create_deployment_empty_model() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        let req = DeploymentRequest {
            model_name: String::new(),
            target_devices: vec!["1".into()],
            strategy: DeployStrategy::AllAtOnce,
        };
        assert!(fm.create_deployment(&req).is_err());
    }

    #[test]
    fn test_create_deployment_empty_targets() {
        let mut fm = make_manager();
        let req = DeploymentRequest {
            model_name: "llama3".into(),
            target_devices: vec![],
            strategy: DeployStrategy::AllAtOnce,
        };
        assert!(fm.create_deployment(&req).is_err());
    }

    #[test]
    fn test_create_deployment_unknown_device() {
        let mut fm = make_manager();
        let req = DeploymentRequest {
            model_name: "llama3".into(),
            target_devices: vec!["nonexistent".into()],
            strategy: DeployStrategy::AllAtOnce,
        };
        assert!(fm.create_deployment(&req).is_err());
    }

    #[test]
    fn test_deployment_marks_devices_deploying() {
        let mut fm = make_manager();
        fm.register_device(make_device("1")).unwrap();
        let req = DeploymentRequest {
            model_name: "llama3".into(),
            target_devices: vec!["1".into()],
            strategy: DeployStrategy::Canary,
        };
        fm.create_deployment(&req).unwrap();
        assert_eq!(fm.get_device("1").unwrap().status, DeviceStatus::Deploying);
    }

    #[test]
    fn test_serde_roundtrip() {
        let d = make_device("1");
        let json = serde_json::to_string(&d).unwrap();
        let back: FleetDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "1");
        assert_eq!(back.status, DeviceStatus::Online);
    }

    #[test]
    fn test_deploy_strategy_serde() {
        let s = DeployStrategy::Rolling;
        let json = serde_json::to_string(&s).unwrap();
        let back: DeployStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DeployStrategy::Rolling);
    }
}
