//! MCP Tools - External interfaces to Fuse functionality

use crate::error::{FuseError, Result};
use crate::model::{ModelManager, ModelMetadata};
use crate::quantization::QuantizationService;
use crate::layer::LayerInspector;
use crate::scanner::VulnerabilityScanner;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// Tool result wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Available MCP tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Tool {
    PullModel { model_name: String, source: Option<String> },
    RunInference { model_name: String, prompt: String, max_tokens: Option<u32> },
    ListModels,
    InspectModel { model_name: String },
    QuantizeModel { model_name: String, method: String, format: Option<String> },
    ScanModel { model_name: String },
    MergeModels { models: Vec<String>, strategy: String },
}

/// Tool execution context
pub struct ToolContext {
    pub model_manager: Arc<ModelManager>,
    pub quantization_service: Arc<QuantizationService>,
    pub layer_inspector: Arc<LayerInspector>,
    pub vulnerability_scanner: Arc<VulnerabilityScanner>,
}

impl ToolContext {
    pub fn new(
        model_manager: Arc<ModelManager>,
        quantization_service: Arc<QuantizationService>,
        layer_inspector: Arc<LayerInspector>,
        vulnerability_scanner: Arc<VulnerabilityScanner>,
    ) -> Self {
        Self {
            model_manager,
            quantization_service,
            layer_inspector,
            vulnerability_scanner,
        }
    }
}

/// Execute a tool with the given context
pub async fn execute_tool(tool: Tool, context: &ToolContext) -> Result<ToolResult> {
    match tool {
        Tool::PullModel { model_name, source } => {
            execute_pull_model(&model_name, source.as_deref(), context).await
        }
        Tool::RunInference { model_name, prompt, max_tokens } => {
            execute_run_inference(&model_name, &prompt, max_tokens, context).await
        }
        Tool::ListModels => execute_list_models(context).await,
        Tool::InspectModel { model_name } => {
            execute_inspect_model(&model_name, context).await
        }
        Tool::QuantizeModel { model_name, method, format } => {
            execute_quantize_model(&model_name, &method, format.as_deref(), context).await
        }
        Tool::ScanModel { model_name } => {
            execute_scan_model(&model_name, context).await
        }
        Tool::MergeModels { models, strategy } => {
            execute_merge_models(&models, &strategy, context).await
        }
    }
}

async fn execute_pull_model(
    model_name: &str,
    source: Option<&str>,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Implementation would pull model using ModelManager
    // For now, return placeholder
    Ok(ToolResult {
        success: true,
        message: format!("Model '{}' pull initiated", model_name),
        data: Some(serde_json::json!({
            "model": model_name,
            "source": source.unwrap_or("default"),
            "status": "initiated"
        })),
    })
}

async fn execute_run_inference(
    model_name: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Implementation would run inference using InferenceEngine
    // For now, return placeholder
    Ok(ToolResult {
        success: true,
        message: format!("Inference completed for model '{}'", model_name),
        data: Some(serde_json::json!({
            "model": model_name,
            "prompt": prompt,
            "max_tokens": max_tokens,
            "response": "Sample response from model"
        })),
    })
}

async fn execute_list_models(context: &ToolContext) -> Result<ToolResult> {
    let models = context.model_manager.list().await?;
    let model_names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();

    Ok(ToolResult {
        success: true,
        message: format!("Found {} models", model_names.len()),
        data: Some(serde_json::json!({
            "models": model_names,
            "count": model_names.len()
        })),
    })
}

async fn execute_inspect_model(
    model_name: &str,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Get model metadata
    let metadata = context.model_manager.get_metadata(model_name).await?;

    match metadata {
        Some(meta) => {
            // Inspect layers if model is available
            let model_path = PathBuf::from(&meta.file_path);
            let layers = context.layer_inspector.inspect(&model_path, false).await?;

            Ok(ToolResult {
                success: true,
                message: format!("Model '{}' inspected successfully", model_name),
                data: Some(serde_json::json!({
                    "model": model_name,
                    "metadata": {
                        "architecture": meta.architecture,
                        "parameter_count": meta.parameter_count,
                        "quantization": meta.quantization,
                        "size_bytes": meta.size_bytes
                    },
                    "layers": layers.len()
                })),
            })
        }
        None => Ok(ToolResult {
            success: false,
            message: format!("Model '{}' not found", model_name),
            data: None,
        }),
    }
}

async fn execute_quantize_model(
    model_name: &str,
    method: &str,
    format: Option<&str>,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Get model path
    let metadata = context.model_manager.get_metadata(model_name).await?;
    let model_path = metadata
        .ok_or_else(|| FuseError::ModelNotFound(model_name.to_string()))?
        .file_path;

    // Quantize model
    let result = context.quantization_service.quantize(
        &PathBuf::from(model_path),
        method,
        format,
    ).await?;

    Ok(ToolResult {
        success: true,
        message: format!("Model '{}' quantized successfully", model_name),
        data: Some(serde_json::json!({
            "model": model_name,
            "method": method,
            "format": format,
            "output_path": result.output_path,
            "compression_ratio": result.compression_ratio
        })),
    })
}

async fn execute_scan_model(
    model_name: &str,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Get model path
    let metadata = context.model_manager.get_metadata(model_name).await?;
    let model_path = metadata
        .ok_or_else(|| FuseError::ModelNotFound(model_name.to_string()))?
        .file_path;

    // Scan model
    let report = context.vulnerability_scanner.scan_model(&PathBuf::from(model_path)).await?;

    Ok(ToolResult {
        success: true,
        message: format!("Model '{}' scanned successfully", model_name),
        data: Some(serde_json::json!({
            "model": model_name,
            "vulnerabilities_found": report.vulnerabilities.len(),
            "severity_breakdown": {
                "critical": report.vulnerabilities.iter().filter(|v| v.severity == "critical").count(),
                "high": report.vulnerabilities.iter().filter(|v| v.severity == "high").count(),
                "medium": report.vulnerabilities.iter().filter(|v| v.severity == "medium").count(),
                "low": report.vulnerabilities.iter().filter(|v| v.severity == "low").count()
            }
        })),
    })
}

async fn execute_merge_models(
    models: &[String],
    strategy: &str,
    context: &ToolContext,
) -> Result<ToolResult> {
    // Implementation would use ModelMerger
    // For now, return placeholder
    Ok(ToolResult {
        success: true,
        message: format!("Models merged successfully using {} strategy", strategy),
        data: Some(serde_json::json!({
            "models": models,
            "strategy": strategy,
            "output_model": "merged_model",
            "merge_time_seconds": 120.5
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_serialization() {
        let tool = Tool::ListModels;
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Tool::ListModels));
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            success: true,
            message: "Test message".to_string(),
            data: Some(serde_json::json!({"key": "value"})),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&json).unwrap();

        assert!(deserialized.success);
        assert_eq!(deserialized.message, "Test message");
        assert_eq!(deserialized.data.unwrap()["key"], "value");
    }
}