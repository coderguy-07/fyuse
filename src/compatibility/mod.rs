use crate::error::{FuseError, Result};
use crate::model::ModelMetadata;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub models: Vec<String>,
    pub overall_score: f32,
    pub factors: Vec<CompatibilityFactor>,
    pub recommendations: Vec<String>,
    pub merge_strategies: Vec<MergeStrategy>,
    pub timestamp: chrono::DateTime<Utc>,
    pub analysis_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityFactor {
    pub name: String,
    pub score: f32,  // 0.0 to 1.0
    pub weight: f32, // Importance weight
    pub details: String,
    pub category: FactorCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FactorCategory {
    Architecture,
    Parameters,
    Quantization,
    Size,
    Training,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReportFormat {
    AsciiTable, // Default, printed to stdout
    Json,       // Structured data for programmatic access
    Html,       // Interactive report with charts
    Markdown,   // Formatted text for documentation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    Average,
    Weighted,
    SLERP(SlerpConfig),
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlerpConfig {
    pub t: f32, // Interpolation parameter (0.0 to 1.0)
    pub base_model_index: usize,
}

pub struct CompatibilityChecker {
    analysis_cache: HashMap<String, CompatibilityReport>,
}

impl CompatibilityChecker {
    pub fn new() -> Self {
        Self {
            analysis_cache: HashMap::new(),
        }
    }

    pub async fn check_compatibility(
        &self,
        models: &[ModelMetadata],
    ) -> Result<CompatibilityReport> {
        if models.len() < 2 {
            return Err(FuseError::ValidationError(
                "At least 2 models are required for compatibility analysis".to_string(),
            ));
        }

        let start_time = std::time::Instant::now();
        let cache_key = self.generate_cache_key(models);

        // Check cache first
        if let Some(cached_report) = self.analysis_cache.get(&cache_key) {
            debug!("Using cached compatibility analysis for {}", cache_key);
            return Ok(cached_report.clone());
        }

        info!("Analyzing compatibility for {} models", models.len());

        let factors = self.analyze_compatibility_factors(models).await?;
        let overall_score = self.calculate_overall_score(&factors);
        let recommendations = self.generate_recommendations(&factors, overall_score);
        let merge_strategies = self.suggest_merge_strategies(models, &factors);

        let report = CompatibilityReport {
            models: models.iter().map(|m| m.name.clone()).collect(),
            overall_score,
            factors,
            recommendations,
            merge_strategies,
            timestamp: Utc::now(),
            analysis_duration_ms: start_time.elapsed().as_millis() as u64,
        };

        // Cache the result
        // Note: In a real implementation, we'd need mutable access
        // For now, we'll skip caching to avoid borrow checker issues

        info!(
            "Compatibility analysis completed. Score: {:.1}%",
            overall_score * 100.0
        );
        Ok(report)
    }

    async fn analyze_compatibility_factors(
        &self,
        models: &[ModelMetadata],
    ) -> Result<Vec<CompatibilityFactor>> {
        let mut factors = Vec::new();

        // Architecture compatibility
        factors.push(self.analyze_architecture_compatibility(models).await?);

        // Parameter count compatibility
        factors.push(self.analyze_parameter_compatibility(models).await?);

        // Quantization compatibility
        factors.push(self.analyze_quantization_compatibility(models).await?);

        // Size compatibility
        factors.push(self.analyze_size_compatibility(models).await?);

        // Training data compatibility (if available)
        if let Some(training_factor) = self.analyze_training_compatibility(models).await? {
            factors.push(training_factor);
        }

        Ok(factors)
    }

    async fn analyze_architecture_compatibility(
        &self,
        models: &[ModelMetadata],
    ) -> Result<CompatibilityFactor> {
        let architectures: Vec<String> = models
            .iter()
            .map(|m| {
                m.architecture
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .collect();

        let all_same = architectures.windows(2).all(|w| w[0] == w[1]);
        let score = if all_same { 1.0 } else { 0.3 }; // Some architectures can still be merged

        let details = if all_same {
            format!("All models use the same architecture: {}", architectures[0])
        } else {
            format!(
                "Mixed architectures: {}. Merging may require architecture alignment.",
                architectures.join(", ")
            )
        };

        Ok(CompatibilityFactor {
            name: "Architecture Compatibility".to_string(),
            score,
            weight: 0.4, // High importance
            details,
            category: FactorCategory::Architecture,
        })
    }

    async fn analyze_parameter_compatibility(
        &self,
        models: &[ModelMetadata],
    ) -> Result<CompatibilityFactor> {
        let param_counts: Vec<u64> = models
            .iter()
            .map(|m| m.parameter_count.unwrap_or(0) as u64)
            .collect();

        if param_counts.contains(&0) {
            return Ok(CompatibilityFactor {
                name: "Parameter Count Compatibility".to_string(),
                score: 0.5, // Unknown parameter counts
                weight: 0.3,
                details:
                    "Some models have unknown parameter counts. Manual verification recommended."
                        .to_string(),
                category: FactorCategory::Parameters,
            });
        }

        let avg_params = param_counts.iter().sum::<u64>() as f32 / param_counts.len() as f32;
        let max_deviation = param_counts
            .iter()
            .map(|&p| ((p as f32 - avg_params) / avg_params).abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let score = (1.0 - max_deviation.min(1.0)).max(0.1); // At least 10% compatibility

        let details = format!(
            "Parameter count range: {}M - {}M (avg: {:.0}M). Deviation: {:.1}%",
            param_counts.iter().min().unwrap() / 1_000_000,
            param_counts.iter().max().unwrap() / 1_000_000,
            avg_params / 1_000_000.0,
            max_deviation * 100.0
        );

        Ok(CompatibilityFactor {
            name: "Parameter Count Compatibility".to_string(),
            score,
            weight: 0.3,
            details,
            category: FactorCategory::Parameters,
        })
    }

    async fn analyze_quantization_compatibility(
        &self,
        models: &[ModelMetadata],
    ) -> Result<CompatibilityFactor> {
        let quantizations: Vec<String> = models
            .iter()
            .map(|m| m.quantization.clone().unwrap_or_else(|| "none".to_string()))
            .collect();

        let all_same = quantizations.windows(2).all(|w| w[0] == w[1]);
        let score = if all_same { 1.0 } else { 0.7 }; // Different quantizations can often be merged

        let details = if all_same {
            format!("All models use the same quantization: {}", quantizations[0])
        } else {
            format!(
                "Mixed quantization methods: {}. May require requantization after merging.",
                quantizations.join(", ")
            )
        };

        Ok(CompatibilityFactor {
            name: "Quantization Compatibility".to_string(),
            score,
            weight: 0.2,
            details,
            category: FactorCategory::Quantization,
        })
    }

    async fn analyze_size_compatibility(
        &self,
        models: &[ModelMetadata],
    ) -> Result<CompatibilityFactor> {
        let sizes: Vec<u64> = models.iter().map(|m| m.size_bytes).collect();
        let total_size: u64 = sizes.iter().sum();
        let _avg_size = total_size as f32 / sizes.len() as f32;

        // Size compatibility is about whether the merged model will fit in memory
        let max_reasonable_size = 50 * 1024 * 1024 * 1024; // 50GB
        let score = if total_size > max_reasonable_size {
            0.3
        } else {
            0.9
        };

        let details = format!(
            "Individual sizes: {} - {}. Total: {:.1}GB. Memory requirements: {:.1}GB",
            self.format_size(*sizes.iter().min().unwrap()),
            self.format_size(*sizes.iter().max().unwrap()),
            total_size as f32 / (1024.0 * 1024.0 * 1024.0),
            (total_size as f32 * 1.5) / (1024.0 * 1024.0 * 1024.0) // Rough estimate for merged model
        );

        Ok(CompatibilityFactor {
            name: "Size Compatibility".to_string(),
            score,
            weight: 0.1,
            details,
            category: FactorCategory::Size,
        })
    }

    async fn analyze_training_compatibility(
        &self,
        _models: &[ModelMetadata],
    ) -> Result<Option<CompatibilityFactor>> {
        // This would analyze training data overlap, domains, etc.
        // For now, return None as we don't have training metadata
        Ok(None)
    }

    fn calculate_overall_score(&self, factors: &[CompatibilityFactor]) -> f32 {
        let weighted_sum: f32 = factors.iter().map(|f| f.score * f.weight).sum();

        let total_weight: f32 = factors.iter().map(|f| f.weight).sum();

        if total_weight > 0.0 {
            (weighted_sum / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn generate_recommendations(
        &self,
        factors: &[CompatibilityFactor],
        overall_score: f32,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if overall_score < 0.5 {
            recommendations.push("Compatibility is low. Consider using different models or manual parameter alignment.".to_string());
        } else if overall_score < 0.7 {
            recommendations.push(
                "Moderate compatibility. Test the merged model thoroughly before deployment."
                    .to_string(),
            );
        } else {
            recommendations.push(
                "Good compatibility. Standard merging strategies should work well.".to_string(),
            );
        }

        // Factor-specific recommendations
        for factor in factors {
            if factor.score < 0.5 {
                match factor.category {
                    FactorCategory::Architecture => {
                        recommendations.push(
                            "Consider models with similar architectures for better compatibility."
                                .to_string(),
                        );
                    }
                    FactorCategory::Parameters => {
                        recommendations.push(
                            "Large parameter count differences may affect merge quality."
                                .to_string(),
                        );
                    }
                    FactorCategory::Quantization => {
                        recommendations.push(
                            "Consider requantizing models to the same format before merging."
                                .to_string(),
                        );
                    }
                    FactorCategory::Size => {
                        recommendations.push(
                            "Large merged model size may require significant memory.".to_string(),
                        );
                    }
                    _ => {}
                }
            }
        }

        recommendations
    }

    fn suggest_merge_strategies(
        &self,
        models: &[ModelMetadata],
        factors: &[CompatibilityFactor],
    ) -> Vec<MergeStrategy> {
        let mut strategies = vec![MergeStrategy::Average];

        // Add weighted if parameter counts are available and vary
        let param_counts: Vec<u64> = models
            .iter()
            .map(|m| m.parameter_count.unwrap_or(0) as u64)
            .collect();

        if param_counts.iter().any(|&c| c > 0) {
            strategies.push(MergeStrategy::Weighted);
        }

        // Add SLERP for high compatibility scores
        let arch_compatible = factors
            .iter()
            .find(|f| matches!(f.category, FactorCategory::Architecture))
            .map(|f| f.score > 0.8)
            .unwrap_or(false);

        if arch_compatible && models.len() == 2 {
            strategies.push(MergeStrategy::SLERP(SlerpConfig {
                t: 0.5,
                base_model_index: 0,
            }));
        }

        strategies
    }

    fn generate_cache_key(&self, models: &[ModelMetadata]) -> String {
        let mut key_parts: Vec<String> = models
            .iter()
            .map(|m| format!("{}:{}", m.name, m.version))
            .collect();
        key_parts.sort();
        key_parts.join("|")
    }

    fn format_size(&self, bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.1}{}", size, UNITS[unit_index])
    }

    pub async fn generate_report(
        &self,
        report: &CompatibilityReport,
        format: ReportFormat,
        output: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let content = match format {
            ReportFormat::AsciiTable => self.generate_ascii_report(report),
            ReportFormat::Json => self.generate_json_report(report)?,
            ReportFormat::Html => self.generate_html_report(report),
            ReportFormat::Markdown => self.generate_markdown_report(report),
        };

        let path = if let Some(custom_path) = output {
            custom_path
        } else {
            let timestamp = report.timestamp.format("%Y-%m-%d_%H-%M-%S");
            let filename = format!("compatibility_{}.{}", timestamp, format.extension());
            PathBuf::from(".fuse/report/compatibility").join(filename)
        };

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&path, &content).await?;
        info!("Compatibility report saved to: {}", path.display());

        Ok(path)
    }

    fn generate_ascii_report(&self, report: &CompatibilityReport) -> String {
        let mut output = String::new();
        output.push_str("Model Compatibility Analysis Report\n");
        output.push_str("===================================\n\n");

        output.push_str(&format!("Models: {}\n", report.models.join(", ")));
        output.push_str(&format!(
            "Overall Compatibility Score: {:.1}%\n",
            report.score_percentage()
        ));
        output.push_str(&format!(
            "Analysis Time: {} ms\n",
            report.analysis_duration_ms
        ));
        output.push_str(&format!(
            "Generated: {}\n\n",
            report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        output.push_str("Compatibility Factors:\n");
        output.push_str("----------------------\n");

        for factor in &report.factors {
            output.push_str(&format!(
                "{:.<30} {:.1}% (weight: {:.1})\n",
                factor.name,
                factor.score * 100.0,
                factor.weight
            ));
            output.push_str(&format!("    {}\n\n", factor.details));
        }

        if !report.recommendations.is_empty() {
            output.push_str("Recommendations:\n");
            output.push_str("----------------\n");
            for rec in &report.recommendations {
                output.push_str(&format!("• {}\n", rec));
            }
            output.push('\n');
        }

        output.push_str("Suggested Merge Strategies:\n");
        output.push_str("---------------------------\n");
        for strategy in &report.merge_strategies {
            output.push_str(&format!("• {}\n", strategy.display_name()));
        }

        output
    }

    fn generate_json_report(&self, report: &CompatibilityReport) -> Result<String> {
        Ok(serde_json::to_string_pretty(report)?)
    }

    fn generate_html_report(&self, report: &CompatibilityReport) -> String {
        let score_color = if report.overall_score >= 0.7 {
            "#4CAF50"
        } else if report.overall_score >= 0.5 {
            "#FF9800"
        } else {
            "#f44336"
        };

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Model Compatibility Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }}
        .container {{ max-width: 1000px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .header {{ text-align: center; margin-bottom: 30px; }}
        .score {{ font-size: 48px; font-weight: bold; color: {}; text-align: center; margin: 20px 0; }}
        .factors {{ margin: 20px 0; }}
        .factor {{ padding: 15px; margin: 10px 0; background: #f9f9f9; border-radius: 4px; border-left: 4px solid {}; }}
        .recommendations {{ background: #e3f2fd; padding: 15px; border-radius: 4px; margin: 20px 0; }}
        .strategies {{ background: #f3e5f5; padding: 15px; border-radius: 4px; margin: 20px 0; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Model Compatibility Analysis Report</h1>
            <p>Generated: {}</p>
        </div>

        <div class="score">{:.1}%</div>
        <p><strong>Models:</strong> {}</p>
        <p><strong>Analysis Time:</strong> {} ms</p>

        <div class="factors">
            <h2>Compatibility Factors</h2>
            {}
        </div>

        <div class="recommendations">
            <h2>Recommendations</h2>
            {}
        </div>

        <div class="strategies">
            <h2>Suggested Merge Strategies</h2>
            {}
        </div>
    </div>
</body>
</html>"#,
            score_color,
            score_color,
            report.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            report.score_percentage(),
            report.models.join(", "),
            report.analysis_duration_ms,
            report
                .factors
                .iter()
                .map(|f| {
                    let factor_color = if f.score >= 0.7 {
                        "#4CAF50"
                    } else if f.score >= 0.5 {
                        "#FF9800"
                    } else {
                        "#f44336"
                    };
                    format!(
                        r#"<div class="factor" style="border-left-color: {}">
                       <strong>{}</strong> - {:.1}% (weight: {:.1})<br>
                       <small>{}</small>
                   </div>"#,
                        factor_color,
                        f.name,
                        f.score * 100.0,
                        f.weight,
                        f.details
                    )
                })
                .collect::<Vec<_>>()
                .join("\n            "),
            report
                .recommendations
                .iter()
                .map(|r| format!("<li>{}</li>", r))
                .collect::<Vec<_>>()
                .join("\n            "),
            report
                .merge_strategies
                .iter()
                .map(|s| format!("<li>{}</li>", s.display_name()))
                .collect::<Vec<_>>()
                .join("\n            ")
        )
    }

    fn generate_markdown_report(&self, report: &CompatibilityReport) -> String {
        let mut md = String::new();
        md.push_str("# Model Compatibility Analysis Report\n\n");

        md.push_str(&format!(
            "**Generated:** {}\n\n",
            report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!("**Models:** {}\n\n", report.models.join(", ")));
        md.push_str(&format!(
            "**Overall Compatibility Score:** {:.1}%\n\n",
            report.score_percentage()
        ));
        md.push_str(&format!(
            "**Analysis Time:** {} ms\n\n",
            report.analysis_duration_ms
        ));

        md.push_str("## Compatibility Factors\n\n");
        md.push_str("| Factor | Score | Weight | Details |\n");
        md.push_str("|-------|-------|--------|--------|\n");

        for factor in &report.factors {
            md.push_str(&format!(
                "| {} | {:.1}% | {:.1} | {} |\n",
                factor.name,
                factor.score * 100.0,
                factor.weight,
                factor.details
            ));
        }

        md.push_str("\n## Recommendations\n\n");
        for rec in &report.recommendations {
            md.push_str(&format!("- {}\n", rec));
        }

        md.push_str("\n## Suggested Merge Strategies\n\n");
        for strategy in &report.merge_strategies {
            md.push_str(&format!("- {}\n", strategy.display_name()));
        }

        md
    }
}

impl CompatibilityReport {
    pub fn score_percentage(&self) -> f32 {
        self.overall_score * 100.0
    }
}

impl ReportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::AsciiTable => "txt",
            Self::Json => "json",
            Self::Html => "html",
            Self::Markdown => "md",
        }
    }
}

impl MergeStrategy {
    pub fn display_name(&self) -> String {
        match self {
            MergeStrategy::Average => "Average - Simple parameter averaging".to_string(),
            MergeStrategy::Weighted => "Weighted - Parameter-weighted merging".to_string(),
            MergeStrategy::SLERP(config) => {
                format!("SLERP - Spherical interpolation (t={:.2})", config.t)
            }
            MergeStrategy::Custom(name) => format!("Custom - {}", name),
        }
    }
}

impl Default for CompatibilityChecker {
    fn default() -> Self {
        Self::new()
    }
}
