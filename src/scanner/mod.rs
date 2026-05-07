//! Vulnerability Scanner - Security scanning for models and configurations

pub mod trivy;
pub mod report;

pub use trivy::TrivyScanner;
pub use report::{ScanReport, Vulnerability, Severity};

use crate::error::{FuseError, Result};
use std::path::Path;

/// Main vulnerability scanner service
pub struct VulnerabilityScanner {
    trivy_scanner: TrivyScanner,
}

impl VulnerabilityScanner {
    /// Create a new vulnerability scanner
    pub fn new() -> Result<Self> {
        Ok(Self {
            trivy_scanner: TrivyScanner::new()?,
        })
    }

    /// Scan a model file for vulnerabilities
    pub async fn scan_model(&self, model_path: &Path) -> Result<ScanReport> {
        self.trivy_scanner.scan_model(model_path).await
    }

    /// Scan a directory for vulnerabilities
    pub async fn scan_directory(&self, dir_path: &Path) -> Result<ScanReport> {
        self.trivy_scanner.scan_directory(dir_path).await
    }

    /// Scan configuration files for security issues
    pub async fn scan_config(&self, config_path: &Path) -> Result<ScanReport> {
        // Configuration scanning would check for:
        // - Hardcoded secrets
        // - Insecure defaults
        // - Permission issues
        // For now, return empty report
        Ok(ScanReport {
            target: config_path.to_string_lossy().to_string(),
            vulnerabilities: vec![],
            scan_time: chrono::Utc::now(),
            scanner_version: "1.0.0".to_string(),
        })
    }

    /// Generate SBOM (Software Bill of Materials)
    pub async fn generate_sbom(&self, model_path: &Path) -> Result<String> {
        self.trivy_scanner.generate_sbom(model_path).await
    }

    /// Check if scanner is available
    pub fn is_available(&self) -> bool {
        self.trivy_scanner.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scanner_creation() {
        let scanner = VulnerabilityScanner::new();
        assert!(scanner.is_ok());
    }

    #[tokio::test]
    async fn test_scan_nonexistent_file() {
        let scanner = VulnerabilityScanner::new().unwrap();
        let result = scanner.scan_model(Path::new("/nonexistent/file")).await;
        // Should return error for nonexistent file
        assert!(result.is_err());
    }
}