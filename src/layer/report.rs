use crate::error::Result;
use crate::layer::{LayerInfo, ValidationReport};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReportFormat {
    AsciiTable,
    Json,
    Html,
    Markdown,
}

impl ReportFormat {
    pub fn parse_format(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ascii" | "table" => Some(Self::AsciiTable),
            "json" => Some(Self::Json),
            "html" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::AsciiTable => "txt",
            Self::Json => "json",
            Self::Html => "html",
            Self::Markdown => "md",
        }
    }
}

pub struct ReportGenerator {
    report_dir: PathBuf,
}

impl ReportGenerator {
    pub fn new(workspace_dir: impl AsRef<Path>) -> Result<Self> {
        let report_dir = workspace_dir.as_ref().join(".fuse/report");
        Ok(Self { report_dir })
    }

    pub async fn generate_inspection_report(
        &self,
        model_name: &str,
        layers: &[LayerInfo],
        format: ReportFormat,
        output_path: Option<&Path>,
    ) -> Result<String> {
        let content = match format {
            ReportFormat::AsciiTable => self.generate_inspection_ascii(model_name, layers),
            ReportFormat::Json => self.generate_inspection_json(model_name, layers)?,
            ReportFormat::Html => self.generate_inspection_html(model_name, layers),
            ReportFormat::Markdown => self.generate_inspection_markdown(model_name, layers),
        };

        // Save to file if not ASCII (ASCII is printed to stdout)
        if !matches!(format, ReportFormat::AsciiTable) {
            let path = if let Some(custom_path) = output_path {
                custom_path.to_path_buf()
            } else {
                let inspect_dir = self.report_dir.join("inspect");
                fs::create_dir_all(&inspect_dir).await?;
                inspect_dir.join(format!("{}_layers.{}", model_name, format.extension()))
            };

            fs::write(&path, &content).await?;
            info!("Inspection report saved to: {}", path.display());
        }

        Ok(content)
    }

    pub async fn generate_validation_report(
        &self,
        model_name: &str,
        report: &ValidationReport,
        format: ReportFormat,
        output_path: Option<&Path>,
    ) -> Result<String> {
        let content = match format {
            ReportFormat::AsciiTable => self.generate_validation_ascii(report),
            ReportFormat::Json => self.generate_validation_json(report)?,
            ReportFormat::Html => self.generate_validation_html(report),
            ReportFormat::Markdown => self.generate_validation_markdown(report),
        };

        if !matches!(format, ReportFormat::AsciiTable) {
            let path = if let Some(custom_path) = output_path {
                custom_path.to_path_buf()
            } else {
                let validation_dir = self.report_dir.join("validation");
                fs::create_dir_all(&validation_dir).await?;
                validation_dir.join(format!("{}_validation.{}", model_name, format.extension()))
            };

            fs::write(&path, &content).await?;
            info!("Validation report saved to: {}", path.display());
        }

        Ok(content)
    }

    fn generate_inspection_ascii(&self, model_name: &str, layers: &[LayerInfo]) -> String {
        let mut output = String::new();
        output.push_str(&format!("Layer Inspection Report: {}\n", model_name));
        output.push_str(&format!(
            "Generated: {}\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Calculate column widths
        let id_width = layers
            .iter()
            .map(|l| l.id.len())
            .max()
            .unwrap_or(10)
            .max(10);
        let name_width = layers
            .iter()
            .map(|l| l.name.len())
            .max()
            .unwrap_or(15)
            .max(15);
        let type_width = layers
            .iter()
            .map(|l| l.layer_type.len())
            .max()
            .unwrap_or(15)
            .max(15);

        // Header
        output.push_str(&format!(
            "{:<id_width$} | {:<name_width$} | {:<type_width$} | {:>12} | {:>12}\n",
            "ID",
            "Name",
            "Type",
            "Size",
            "Parameters",
            id_width = id_width,
            name_width = name_width,
            type_width = type_width
        ));
        output.push_str(&"-".repeat(id_width + name_width + type_width + 40));
        output.push('\n');

        // Rows
        for layer in layers {
            output.push_str(&format!(
                "{:<id_width$} | {:<name_width$} | {:<type_width$} | {:>12} | {:>12}\n",
                layer.id,
                layer.name,
                layer.layer_type,
                layer.format_size(),
                layer.format_parameters(),
                id_width = id_width,
                name_width = name_width,
                type_width = type_width
            ));
        }

        // Summary
        let total_size: u64 = layers.iter().map(|l| l.size_bytes).sum();
        let total_params: u64 = layers.iter().map(|l| l.parameters).sum();
        output.push('\n');
        output.push_str(&format!("Total Layers: {}\n", layers.len()));
        output.push_str(&format!(
            "Total Size: {:.2} GB\n",
            total_size as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
        output.push_str(&format!(
            "Total Parameters: {:.2}B\n",
            total_params as f64 / 1_000_000_000.0
        ));

        output
    }

    fn generate_inspection_json(&self, model_name: &str, layers: &[LayerInfo]) -> Result<String> {
        let report = serde_json::json!({
            "model_name": model_name,
            "generated_at": Utc::now().to_rfc3339(),
            "layer_count": layers.len(),
            "layers": layers,
            "summary": {
                "total_size_bytes": layers.iter().map(|l| l.size_bytes).sum::<u64>(),
                "total_parameters": layers.iter().map(|l| l.parameters).sum::<u64>(),
            }
        });

        Ok(serde_json::to_string_pretty(&report)?)
    }

    fn generate_inspection_html(&self, model_name: &str, layers: &[LayerInfo]) -> String {
        let mut html = String::from(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Layer Inspection Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        h1 { color: #333; border-bottom: 2px solid #4CAF50; padding-bottom: 10px; }
        .meta { color: #666; margin-bottom: 20px; }
        table { width: 100%; border-collapse: collapse; margin: 20px 0; }
        th { background: #4CAF50; color: white; padding: 12px; text-align: left; }
        td { padding: 10px; border-bottom: 1px solid #ddd; }
        tr:hover { background: #f5f5f5; }
        .summary { background: #e8f5e9; padding: 15px; border-radius: 4px; margin-top: 20px; }
        .summary h3 { margin-top: 0; color: #2e7d32; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Layer Inspection Report</h1>
        <div class="meta">
            <strong>Model:</strong> "#,
        );
        html.push_str(model_name);
        html.push_str("<br>\n            <strong>Generated:</strong> ");
        html.push_str(&Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string());
        html.push_str(
            r#"
        </div>
        <table>
            <thead>
                <tr>
                    <th>ID</th>
                    <th>Name</th>
                    <th>Type</th>
                    <th>Size</th>
                    <th>Parameters</th>
                    <th>Input Shape</th>
                    <th>Output Shape</th>
                </tr>
            </thead>
            <tbody>
"#,
        );

        for layer in layers {
            html.push_str(&format!(
                "                <tr>\n                    <td>{}</td>\n                    <td>{}</td>\n                    <td>{}</td>\n                    <td>{}</td>\n                    <td>{}</td>\n                    <td>{:?}</td>\n                    <td>{:?}</td>\n                </tr>\n",
                layer.id, layer.name, layer.layer_type, layer.format_size(), layer.format_parameters(),
                layer.input_shape, layer.output_shape
            ));
        }

        let total_size: u64 = layers.iter().map(|l| l.size_bytes).sum();
        let total_params: u64 = layers.iter().map(|l| l.parameters).sum();

        html.push_str(
            r#"            </tbody>
        </table>
        <div class="summary">
            <h3>Summary</h3>
            <p><strong>Total Layers:</strong> "#,
        );
        html.push_str(&layers.len().to_string());
        html.push_str("</p>\n            <p><strong>Total Size:</strong> ");
        html.push_str(&format!(
            "{:.2} GB",
            total_size as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
        html.push_str("</p>\n            <p><strong>Total Parameters:</strong> ");
        html.push_str(&format!("{:.2}B", total_params as f64 / 1_000_000_000.0));
        html.push_str(
            r#"</p>
        </div>
    </div>
</body>
</html>"#,
        );

        html
    }

    fn generate_inspection_markdown(&self, model_name: &str, layers: &[LayerInfo]) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Layer Inspection Report: {}\n\n", model_name));
        md.push_str(&format!(
            "**Generated:** {}\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        md.push_str("## Layers\n\n");
        md.push_str("| ID | Name | Type | Size | Parameters | Input Shape | Output Shape |\n");
        md.push_str("|---|---|---|---|---|---|---|\n");

        for layer in layers {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {:?} | {:?} |\n",
                layer.id,
                layer.name,
                layer.layer_type,
                layer.format_size(),
                layer.format_parameters(),
                layer.input_shape,
                layer.output_shape
            ));
        }

        let total_size: u64 = layers.iter().map(|l| l.size_bytes).sum();
        let total_params: u64 = layers.iter().map(|l| l.parameters).sum();

        md.push_str("\n## Summary\n\n");
        md.push_str(&format!("- **Total Layers:** {}\n", layers.len()));
        md.push_str(&format!(
            "- **Total Size:** {:.2} GB\n",
            total_size as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
        md.push_str(&format!(
            "- **Total Parameters:** {:.2}B\n",
            total_params as f64 / 1_000_000_000.0
        ));

        md
    }

    fn generate_validation_ascii(&self, report: &ValidationReport) -> String {
        let mut output = String::new();
        output.push_str(&format!("Model Validation Report: {}\n", report.model_name));
        output.push_str(&format!(
            "Status: {}\n\n",
            if report.is_valid {
                "✓ VALID"
            } else {
                "✗ INVALID"
            }
        ));

        output.push_str("Validation Checks:\n");
        output.push_str(&format!(
            "  Architecture: {}\n",
            if report.architecture_valid {
                "✓"
            } else {
                "✗"
            }
        ));
        output.push_str(&format!(
            "  Tensor Shapes: {}\n",
            if report.tensor_shapes_valid {
                "✓"
            } else {
                "✗"
            }
        ));
        output.push_str(&format!(
            "  Connections: {}\n\n",
            if report.connections_valid {
                "✓"
            } else {
                "✗"
            }
        ));

        output.push_str("Model Statistics:\n");
        output.push_str(&format!("  Layers: {}\n", report.layer_count));
        output.push_str(&format!("  Parameters: {}\n", report.format_parameters()));
        output.push_str(&format!("  Size: {}\n\n", report.format_size()));

        if !report.issues.is_empty() {
            output.push_str("Issues:\n");
            for issue in &report.issues {
                let severity_symbol = match issue.severity {
                    crate::layer::validator::IssueSeverity::Error => "✗",
                    crate::layer::validator::IssueSeverity::Warning => "⚠",
                    crate::layer::validator::IssueSeverity::Info => "ℹ",
                };
                output.push_str(&format!("  {} {}\n", severity_symbol, issue.message));
                if let Some(suggestion) = &issue.suggestion {
                    output.push_str(&format!("    Suggestion: {}\n", suggestion));
                }
            }
        }

        output
    }

    fn generate_validation_json(&self, report: &ValidationReport) -> Result<String> {
        Ok(serde_json::to_string_pretty(report)?)
    }

    fn generate_validation_html(&self, report: &ValidationReport) -> String {
        let status_color = if report.is_valid {
            "#4CAF50"
        } else {
            "#f44336"
        };
        let status_text = if report.is_valid { "VALID" } else { "INVALID" };

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Model Validation Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }}
        .container {{ max-width: 900px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        h1 {{ color: #333; border-bottom: 2px solid {}; padding-bottom: 10px; }}
        .status {{ font-size: 24px; font-weight: bold; color: {}; margin: 20px 0; }}
        .checks {{ margin: 20px 0; }}
        .check {{ padding: 10px; margin: 5px 0; background: #f5f5f5; border-radius: 4px; }}
        .check.pass {{ border-left: 4px solid #4CAF50; }}
        .check.fail {{ border-left: 4px solid #f44336; }}
        .stats {{ background: #e3f2fd; padding: 15px; border-radius: 4px; margin: 20px 0; }}
        .issues {{ margin: 20px 0; }}
        .issue {{ padding: 10px; margin: 5px 0; border-radius: 4px; }}
        .issue.error {{ background: #ffebee; border-left: 4px solid #f44336; }}
        .issue.warning {{ background: #fff3e0; border-left: 4px solid #ff9800; }}
        .issue.info {{ background: #e3f2fd; border-left: 4px solid #2196F3; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Model Validation Report</h1>
        <div class="status">{}</div>
        <p><strong>Model:</strong> {}</p>
        
        <div class="checks">
            <h2>Validation Checks</h2>
            <div class="check {}">Architecture: {}</div>
            <div class="check {}">Tensor Shapes: {}</div>
            <div class="check {}">Connections: {}</div>
        </div>
        
        <div class="stats">
            <h2>Model Statistics</h2>
            <p><strong>Layers:</strong> {}</p>
            <p><strong>Parameters:</strong> {}</p>
            <p><strong>Size:</strong> {}</p>
        </div>
        
        <div class="issues">
            <h2>Issues ({} errors, {} warnings)</h2>
            {}
        </div>
    </div>
</body>
</html>"#,
            status_color,
            status_color,
            status_text,
            report.model_name,
            if report.architecture_valid {
                "pass"
            } else {
                "fail"
            },
            if report.architecture_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            },
            if report.tensor_shapes_valid {
                "pass"
            } else {
                "fail"
            },
            if report.tensor_shapes_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            },
            if report.connections_valid {
                "pass"
            } else {
                "fail"
            },
            if report.connections_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            },
            report.layer_count,
            report.format_parameters(),
            report.format_size(),
            report.error_count(),
            report.warning_count(),
            report
                .issues
                .iter()
                .map(|issue| {
                    let (class, severity) = match issue.severity {
                        crate::layer::validator::IssueSeverity::Error => ("error", "Error"),
                        crate::layer::validator::IssueSeverity::Warning => ("warning", "Warning"),
                        crate::layer::validator::IssueSeverity::Info => ("info", "Info"),
                    };
                    format!(
                        "<div class=\"issue {}\"><strong>{}:</strong> {}</div>",
                        class, severity, issue.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n            ")
        )
    }

    fn generate_validation_markdown(&self, report: &ValidationReport) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Model Validation Report: {}\n\n",
            report.model_name
        ));
        md.push_str(&format!(
            "**Status:** {}\n\n",
            if report.is_valid {
                "✓ VALID"
            } else {
                "✗ INVALID"
            }
        ));

        md.push_str("## Validation Checks\n\n");
        md.push_str(&format!(
            "- Architecture: {}\n",
            if report.architecture_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            }
        ));
        md.push_str(&format!(
            "- Tensor Shapes: {}\n",
            if report.tensor_shapes_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            }
        ));
        md.push_str(&format!(
            "- Connections: {}\n\n",
            if report.connections_valid {
                "✓ Pass"
            } else {
                "✗ Fail"
            }
        ));

        md.push_str("## Model Statistics\n\n");
        md.push_str(&format!("- **Layers:** {}\n", report.layer_count));
        md.push_str(&format!(
            "- **Parameters:** {}\n",
            report.format_parameters()
        ));
        md.push_str(&format!("- **Size:** {}\n\n", report.format_size()));

        if !report.issues.is_empty() {
            md.push_str(&format!(
                "## Issues ({} errors, {} warnings)\n\n",
                report.error_count(),
                report.warning_count()
            ));
            for issue in &report.issues {
                let severity = match issue.severity {
                    crate::layer::validator::IssueSeverity::Error => "❌ Error",
                    crate::layer::validator::IssueSeverity::Warning => "⚠️ Warning",
                    crate::layer::validator::IssueSeverity::Info => "ℹ️ Info",
                };
                md.push_str(&format!("- **{}:** {}\n", severity, issue.message));
                if let Some(suggestion) = &issue.suggestion {
                    md.push_str(&format!("  - *Suggestion:* {}\n", suggestion));
                }
            }
        }

        md
    }
}
