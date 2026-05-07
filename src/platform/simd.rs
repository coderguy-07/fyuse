//! SIMD capability detection.

use serde::{Deserialize, Serialize};

/// SIMD capability level, from least to most capable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SimdLevel {
    None,
    Sse2,
    Sse42,
    Avx,
    Avx2,
    Avx512,
    Neon,
    NeonDotprod,
    Amx,
}

/// Detected SIMD capabilities for the current platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdCapability {
    pub level: SimdLevel,
    pub features: Vec<String>,
}

impl SimdCapability {
    /// Detect SIMD capabilities of the current CPU.
    pub fn detect() -> Self {
        let mut features = Vec::new();
        let level;

        #[cfg(target_arch = "x86_64")]
        {
            level = Self::detect_x86(&mut features);
        }

        #[cfg(target_arch = "aarch64")]
        {
            level = Self::detect_arm(&mut features);
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            level = SimdLevel::None;
        }

        Self { level, features }
    }

    #[cfg(target_arch = "x86_64")]
    fn detect_x86(features: &mut Vec<String>) -> SimdLevel {
        let mut level = SimdLevel::None;

        if std::is_x86_feature_detected!("sse2") {
            features.push("sse2".to_string());
            level = SimdLevel::Sse2;
        }
        if std::is_x86_feature_detected!("sse4.2") {
            features.push("sse4.2".to_string());
            level = SimdLevel::Sse42;
        }
        if std::is_x86_feature_detected!("avx") {
            features.push("avx".to_string());
            level = SimdLevel::Avx;
        }
        if std::is_x86_feature_detected!("avx2") {
            features.push("avx2".to_string());
            level = SimdLevel::Avx2;
        }
        if std::is_x86_feature_detected!("fma") {
            features.push("fma".to_string());
        }
        if std::is_x86_feature_detected!("avx512f") {
            features.push("avx512f".to_string());
            level = SimdLevel::Avx512;
        }

        level
    }

    #[cfg(target_arch = "aarch64")]
    fn detect_arm(features: &mut Vec<String>) -> SimdLevel {
        // NEON is always available on aarch64
        features.push("neon".to_string());
        let mut level = SimdLevel::Neon;

        if std::arch::is_aarch64_feature_detected!("dotprod") {
            features.push("dotprod".to_string());
            level = SimdLevel::NeonDotprod;
        }
        if std::arch::is_aarch64_feature_detected!("fp16") {
            features.push("fp16".to_string());
        }

        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let cap = SimdCapability::detect();
        // On any modern system, we should detect at least something
        assert!(!cap.features.is_empty());
        assert!(cap.level > SimdLevel::None);
    }

    #[test]
    fn test_simd_level_ordering() {
        assert!(SimdLevel::Avx2 > SimdLevel::Avx);
        assert!(SimdLevel::Avx512 > SimdLevel::Avx2);
        assert!(SimdLevel::Neon > SimdLevel::None);
    }
}
