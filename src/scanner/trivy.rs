//! Trivy integration for vulnerability scanning

use crate::error::{FuseError, Result};
use crate::scanner::report::{ScanReport, Vulnerability, Severity};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Trivy scanner implementation
pub struct TrivyScanner {
    trivy_path: Option<String>,
}

impl TrivyScanner {
    /// Create a new Trivy scanner
    pub fn new() -> Result<Self> {
        // Try to find trivy in PATH
        let trivy_path = std::env::var("TRIVY_PATH").ok()
            .or_else(|| Self::find_trivy().ok());

        Ok(Self { trivy_path })
    }

    /// Check if Trivy is available
    pub fn is_available(&self) -> bool {
        self.trivy_path.is_some()
    }

    /// Scan a model file for vulnerabilities
    pub async fn scan_model(&self, model_path: &Path) -> Result<ScanReport> {
        if !self.is_available() {
            return Err(FuseError::InternalError("Trivy scanner not available".to_string()));
        }

        let output = self.run_trivy_scan(model_path, &["--format", "json"]).await?;
        self.parse_trivy_output(&output, model_path)
    }

    /// Scan a directory for vulnerabilities
    pub async fn scan_directory(&self, dir_path: &Path) -> Result<ScanReport> {
        if !self.is_available() {
            return Err(FuseError::InternalError("Trivy scanner not available".to_string()));
        }

        let output = self.run_trivy_scan(dir_path, &["--format", "json", "--scanners", "vuln,secret"]).await?;
        self.parse_trivy_output(&output, dir_path)
    }

    /// Generate SBOM for a model
    pub async fn generate_sbom(&self, model_path: &Path) -> Result<String> {
        if !self.is_available() {
            return Err(FuseError::InternalError("Trivy scanner not available".to_string()));
        }

        let output = self.run_trivy_scan(model_path, &["--format", "cyclonedx", "--output", "-"]).await?;
        Ok(output)
    }

    /// Run Trivy command
    async fn run_trivy_scan(&self, path: &Path, args: &[&str]) -> Result<String> {
        let trivy_cmd = self.trivy_path.as_ref()
            .ok_or_else(|| FuseError::InternalError("Trivy not found".to_string()))?;

        let mut cmd = Command::new(trivy_cmd);
        cmd.arg("fs")
           .arg(path)
           .args(args)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let output = cmd.output().await
            .map_err(|e| FuseError::InternalError(format!("Failed to run trivy: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FuseError::InternalError(format!("Trivy scan failed: {}", stderr)));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| FuseError::InternalError(format!("Invalid trivy output: {}", e)))
    }

    /// Parse Trivy JSON output
    fn parse_trivy_output(&self, output: &str, target_path: &Path) -> Result<ScanReport> {
        if output.trim().is_empty() {
            return Ok(ScanReport {
                target: target_path.to_string_lossy().to_string(),
                vulnerabilities: vec![],
                scan_time: chrono::Utc::now(),
                scanner_version: "trivy".to_string(),
            });
        }

        // Parse Trivy JSON output
        let trivy_result: TrivyResult = serde_json::from_str(output)
            .map_err(|e| FuseError::SerializationError(e.to_string()))?;

        let vulnerabilities = trivy_result.results.into_iter()
            .flat_map(|result| result.vulnerabilities.into_iter()
                .map(|vuln| Vulnerability {
                    id: vuln.vulnerability_id,
                    title: vuln.title,
                    description: vuln.description,
                    severity: self.map_severity(&vuln.severity),
                    cvss_score: vuln.cvss.as_ref().and_then(|cvss| cvss.score),
                    package_name: vuln.pkg_name,
                    package_version: vuln.installed_version,
                    fixed_version: vuln.fixed_version,
                    references: vuln.references,
                })
            )
            .collect();

        Ok(ScanReport {
            target: target_path.to_string_lossy().to_string(),
            vulnerabilities,
            scan_time: chrono::Utc::now(),
            scanner_version: trivy_result.metadata.version,
        })
    }

    /// Map Trivy severity to our Severity enum
    fn map_severity(&self, severity: &str) -> Severity {
        match severity.to_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "HIGH" => Severity::High,
            "MEDIUM" => Severity::Medium,
            "LOW" => Severity::Low,
            _ => Severity::Unknown,
        }
    }

    /// Find trivy executable in PATH
    fn find_trivy() -> Result<String> {
        let paths = std::env::var("PATH")?;
        for path in std::env::split_paths(&paths) {
            let trivy_path = path.join("trivy");
            if trivy_path.exists() {
                return Ok(trivy_path.to_string_lossy().to_string());
            }
            #[cfg(windows)]
            {
                let trivy_path = path.join("trivy.exe");
                if trivy_path.exists() {
                    return Ok(trivy_path.to_string_lossy().to_string());
                }
            }
        }
        Err(FuseError::InternalError("Trivy not found in PATH".to_string()))
    }
}

/// Trivy scan result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyResult {
    #[serde(rename = "SchemaVersion")]
    schema_version: u32,
    #[serde(rename = "ArtifactName")]
    artifact_name: String,
    #[serde(rename = "ArtifactType")]
    artifact_type: String,
    #[serde(rename = "CreatedAt")]
    created_at: String,
    #[serde(rename = "Results")]
    results: Vec<TrivyScanResult>,
    #[serde(rename = "Metadata")]
    metadata: TrivyMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyScanResult {
    #[serde(rename = "Target")]
    target: String,
    #[serde(rename = "Class")]
    class: String,
    #[serde(rename = "Type")]
    result_type: String,
    #[serde(rename = "Vulnerabilities")]
    vulnerabilities: Vec<TrivyVulnerability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyVulnerability {
    #[serde(rename = "VulnerabilityID")]
    vulnerability_id: String,
    #[serde(rename = "PkgName")]
    pkg_name: String,
    #[serde(rename = "PkgPath")]
    pkg_path: Option<String>,
    #[serde(rename = "InstalledVersion")]
    installed_version: String,
    #[serde(rename = "FixedVersion")]
    fixed_version: String,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "Severity")]
    severity: String,
    #[serde(rename = "CVSS")]
    cvss: Option<TrivyCVSS>,
    #[serde(rename = "References")]
    references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyCVSS {
    #[serde(rename = "V3Score")]
    score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyMetadata {
    #[serde(rename = "Version")]
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivy_scanner_creation() {
        let scanner = TrivyScanner::new();
        // Scanner creation should succeed even if trivy is not installed
        assert!(scanner.is_ok());
    }

    #[test]
    fn test_severity_mapping() {
        let scanner = TrivyScanner::new().unwrap();

        assert_eq!(scanner.map_severity("CRITICAL"), Severity::Critical);
        assert_eq!(scanner.map_severity("HIGH"), Severity::High);
        assert_eq!(scanner.map_severity("MEDIUM"), Severity::Medium);
        assert_eq!(scanner.map_severity("LOW"), Severity::Low);
        assert_eq!(scanner.map_severity("UNKNOWN"), Severity::Unknown);
    }
}