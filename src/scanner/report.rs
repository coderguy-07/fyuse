//! Scan report structures and formatting

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Vulnerability severity levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
            Severity::Unknown => "UNKNOWN",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Severity::Critical => "🔴",
            Severity::High => "🟠",
            Severity::Medium => "🟡",
            Severity::Low => "🟢",
            Severity::Unknown => "⚪",
        }
    }
}

/// Individual vulnerability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    pub cvss_score: Option<f64>,
    pub package_name: String,
    pub package_version: String,
    pub fixed_version: String,
    pub references: Vec<String>,
}

impl Vulnerability {
    pub fn severity_score(&self) -> u8 {
        match self.severity {
            Severity::Critical => 5,
            Severity::High => 4,
            Severity::Medium => 3,
            Severity::Low => 2,
            Severity::Unknown => 1,
        }
    }
}

/// Complete scan report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub target: String,
    pub vulnerabilities: Vec<Vulnerability>,
    pub scan_time: chrono::DateTime<chrono::Utc>,
    pub scanner_version: String,
}

impl ScanReport {
    /// Get vulnerabilities by severity
    pub fn vulnerabilities_by_severity(&self) -> HashMap<Severity, Vec<&Vulnerability>> {
        let mut grouped = HashMap::new();

        for vuln in &self.vulnerabilities {
            grouped.entry(vuln.severity.clone()).or_insert_with(Vec::new).push(vuln);
        }

        grouped
    }

    /// Get severity breakdown counts
    pub fn severity_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();

        for vuln in &self.vulnerabilities {
            let count = counts.entry(vuln.severity.as_str().to_string()).or_insert(0);
            *count += 1;
        }

        counts
    }

    /// Check if report has critical vulnerabilities
    pub fn has_critical_vulnerabilities(&self) -> bool {
        self.vulnerabilities.iter().any(|v| matches!(v.severity, Severity::Critical))
    }

    /// Get highest severity vulnerability
    pub fn highest_severity(&self) -> Option<Severity> {
        self.vulnerabilities.iter()
            .max_by_key(|v| v.severity_score())
            .map(|v| v.severity.clone())
    }

    /// Format as ASCII table
    pub fn format_ascii(&self) -> String {
        let mut output = format!("🔍 Security Scan Report for {}\n", self.target);
        output.push_str(&format!("📅 Scan Time: {}\n", self.scan_time.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("🔧 Scanner: {}\n", self.scanner_version));
        output.push_str(&format!("📊 Total Vulnerabilities: {}\n\n", self.vulnerabilities.len()));

        if self.vulnerabilities.is_empty() {
            output.push_str("✅ No vulnerabilities found!\n");
            return output;
        }

        // Severity breakdown
        let counts = self.severity_counts();
        output.push_str("📈 Severity Breakdown:\n");
        for (severity, count) in counts {
            output.push_str(&format!("  {}: {}\n", severity, count));
        }
        output.push_str("\n");

        // Vulnerabilities table
        output.push_str("📋 Vulnerabilities:\n");
        output.push_str("┌─────────────────────────────────────┬─────────┬──────────────────────┐\n");
        output.push_str("│ ID                                  │ Severity│ Package              │\n");
        output.push_str("├─────────────────────────────────────┼─────────┼──────────────────────┤\n");

        for vuln in &self.vulnerabilities {
            output.push_str(&format!("│ {:35} │ {:7} │ {:20} │\n",
                vuln.id.chars().take(35).collect::<String>(),
                vuln.severity.as_str(),
                vuln.package_name.chars().take(20).collect::<String>()
            ));
        }

        output.push_str("└─────────────────────────────────────┴─────────┴──────────────────────┘\n");

        output
    }

    /// Format as JSON
    pub fn format_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::FuseError::SerializationError(e.to_string()).into())
    }

    /// Format as HTML
    pub fn format_html(&self) -> String {
        let mut html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Security Scan Report - {}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .header {{ border-bottom: 2px solid #e5e7eb; padding-bottom: 20px; margin-bottom: 30px; }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 30px; }}
        .stat-card {{ background: #f9fafb; padding: 20px; border-radius: 6px; text-align: center; }}
        .severity-critical {{ color: #dc2626; }}
        .severity-high {{ color: #ea580c; }}
        .severity-medium {{ color: #d97706; }}
        .severity-low {{ color: #16a34a; }}
        .severity-unknown {{ color: #6b7280; }}
        table {{ width: 100%; border-collapse: collapse; margin-top: 20px; }}
        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #e5e7eb; }}
        th {{ background: #f9fafb; font-weight: 600; }}
        .severity-badge {{ padding: 4px 8px; border-radius: 4px; font-size: 0.875rem; font-weight: 500; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔍 Security Scan Report</h1>
            <p><strong>Target:</strong> {}</p>
            <p><strong>Scan Time:</strong> {}</p>
            <p><strong>Scanner:</strong> {}</p>
        </div>
"#,
            self.target,
            self.target,
            self.scan_time.format("%Y-%m-%d %H:%M:%S UTC"),
            self.scanner_version
        );

        // Stats
        let counts = self.severity_counts();
        html.push_str("<div class=\"stats\">");
        for (severity, count) in counts {
            let css_class = format!("severity-{}", severity.to_lowercase());
            html.push_str(&format!(
                "<div class=\"stat-card\"><div class=\"{} {}\">{}</div><div>{} vulnerabilities</div></div>",
                severity, css_class, severity, count
            ));
        }
        html.push_str("</div>");

        // Vulnerabilities table
        if !self.vulnerabilities.is_empty() {
            html.push_str("<h2>📋 Vulnerabilities</h2><table><thead><tr>");
            html.push_str("<th>ID</th><th>Severity</th><th>Package</th><th>Description</th><th>Fixed Version</th></tr></thead><tbody>");

            for vuln in &self.vulnerabilities {
                let severity_class = format!("severity-badge severity-{}", vuln.severity.as_str().to_lowercase());
                html.push_str(&format!(
                    "<tr><td>{}</td><td><span class=\"{}\">{}</span></td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    vuln.id,
                    severity_class,
                    vuln.severity.as_str(),
                    vuln.package_name,
                    vuln.description.chars().take(100).collect::<String>(),
                    vuln.fixed_version
                ));
            }

            html.push_str("</tbody></table>");
        } else {
            html.push_str("<div style=\"text-align: center; padding: 40px; color: #16a34a;\"><h2>✅ No vulnerabilities found!</h2></div>");
        }

        html.push_str("</div></body></html>");
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Critical.as_str(), "CRITICAL");
        assert_eq!(Severity::High.as_str(), "HIGH");
        assert_eq!(Severity::Medium.as_str(), "MEDIUM");
        assert_eq!(Severity::Low.as_str(), "LOW");
        assert_eq!(Severity::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn test_vulnerability_severity_score() {
        let vuln = Vulnerability {
            id: "TEST-001".to_string(),
            title: "Test vulnerability".to_string(),
            description: "Test description".to_string(),
            severity: Severity::Critical,
            cvss_score: None,
            package_name: "test-package".to_string(),
            package_version: "1.0.0".to_string(),
            fixed_version: "1.0.1".to_string(),
            references: vec![],
        };

        assert_eq!(vuln.severity_score(), 5);
    }

    #[test]
    fn test_scan_report_empty() {
        let report = ScanReport {
            target: "test-target".to_string(),
            vulnerabilities: vec![],
            scan_time: chrono::Utc::now(),
            scanner_version: "test".to_string(),
        };

        assert!(!report.has_critical_vulnerabilities());
        assert_eq!(report.highest_severity(), None);
    }

    #[test]
    fn test_scan_report_with_vulnerabilities() {
        let vuln = Vulnerability {
            id: "CVE-2023-12345".to_string(),
            title: "Test vulnerability".to_string(),
            description: "Test description".to_string(),
            severity: Severity::High,
            cvss_score: Some(7.5),
            package_name: "openssl".to_string(),
            package_version: "1.1.1".to_string(),
            fixed_version: "1.1.2".to_string(),
            references: vec!["https://example.com".to_string()],
        };

        let report = ScanReport {
            target: "test-target".to_string(),
            vulnerabilities: vec![vuln],
            scan_time: chrono::Utc::now(),
            scanner_version: "test".to_string(),
        };

        assert!(!report.has_critical_vulnerabilities());
        assert_eq!(report.highest_severity(), Some(Severity::High));

        let counts = report.severity_counts();
        assert_eq!(counts.get("HIGH"), Some(&1));
    }
}