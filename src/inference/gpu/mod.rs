//! GPU inference backends — feature-gated Metal and CUDA support.

#[cfg(feature = "metal")]
pub mod metal;

#[cfg(feature = "cuda")]
pub mod cuda;
