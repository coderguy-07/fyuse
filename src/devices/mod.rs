//! Device Hub — connectors for hardware, wearables, and IoT devices.

pub mod automation;
pub mod correlator;
pub mod traits;

#[cfg(feature = "home-assistant")]
pub mod home_assistant;

#[cfg(feature = "mqtt-devices")]
pub mod mqtt;

#[cfg(feature = "oura")]
pub mod oura;

pub use automation::{Action, AutomationEngine, AutomationRule, Condition, Trigger};
pub use correlator::{DataCorrelator, Insight};
pub use traits::{
    DataQuery, DeviceCommand, DeviceConfig, DeviceConnector, DeviceData, DeviceEvent, DeviceType,
    SensorReading,
};
