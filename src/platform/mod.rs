//! Platform abstraction — hardware detection, SIMD capabilities, OS utilities.

pub mod hardware;
pub mod simd;

pub use hardware::{GpuInfo, HardwareProfile, HardwareProfiler};
pub use simd::{SimdCapability, SimdLevel};
