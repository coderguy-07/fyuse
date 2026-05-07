//! CPU inference engine using candle for tensor operations.

#[cfg(feature = "cpu-inference")]
pub mod engine;
#[cfg(feature = "cpu-inference")]
pub mod kv_cache;

#[cfg(feature = "cpu-inference")]
pub use engine::CpuInferenceBackend;
