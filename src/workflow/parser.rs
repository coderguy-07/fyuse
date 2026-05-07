//! Workflow parser for YAML and TOML formats

use crate::error::{FuseError, Result};
use crate::workflow::{RetryPolicy, Workflow, WorkflowAction, WorkflowStep};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parse workflow from file (auto-detects YAML/TOML based on extension)
pub async fn parse_workflow(path: &Path) -> Result<Workflow> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| FuseError::ValidationError("File must have an extension".to_string()))?;

    match extension.to_lowercase().as_str() {
        "yaml" | "yml" => parse_yaml_workflow(path).await,
        "toml" => parse_toml_workflow(path).await,
        "md" => parse_markdown_workflow(path).await,
        _ => Err(FuseError::ValidationError(format!(
            "Unsupported file extension: {}",
            extension
        ))),
    }
}

/// Parse markdown content into workflow
fn parse_markdown_content(content: &str) -> Result<Workflow> {
    // Simple markdown parser for fuse.md format
    // This is a basic implementation - could be enhanced with proper markdown parsing

    let mut name = "Unnamed Workflow".to_string();
    let description = None;
    let mut steps = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with("# ") && name == "Unnamed Workflow" {
            name = line[2..].trim().to_string();
        } else if line.starts_with("## ") && line.to_lowercase().contains("step") {
            // Parse step
            let step_id = format!("step_{}", steps.len() + 1);
            let step_name = line[3..].trim().to_string();
            let mut step_description = None;

            // Look for description in next lines
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() {
                if lines[i].trim().starts_with("- ") {
                    step_description = Some(lines[i].trim()[2..].trim().to_string());
                    break;
                }
                i += 1;
            }

            // Create action based on step name
            let action = if step_name.to_lowercase().contains("compile") {
                WorkflowAction::Compile
            } else if step_name.to_lowercase().contains("test") {
                WorkflowAction::Test
            } else if step_name.to_lowercase().contains("fix") {
                WorkflowAction::Fix {
                    error_context: "compilation error".to_string(),
                }
            } else {
                WorkflowAction::Execute {
                    command: "echo".to_string(),
                }
            };

            steps.push(WorkflowStep {
                id: step_id,
                name: step_name,
                description: step_description,
                action,
                on_success: None,
                on_failure: None,
                retry_policy: RetryPolicy::default(),
                depends_on: vec![],
                timeout_secs: Some(300),
            });
        }

        i += 1;
    }

    Ok(Workflow {
        name,
        description,
        steps,
        max_iterations: 10,
        timeout_secs: 3600,
        parallel_execution: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Parse workflow from YAML file
pub async fn parse_yaml_workflow(path: &Path) -> Result<Workflow> {
    let content = fs::read_to_string(path).map_err(|e| FuseError::InternalError(e.to_string()))?;

    let yaml_workflow: YamlWorkflow =
        serde_yaml::from_str(&content).map_err(|e| FuseError::SerializationError(e.to_string()))?;

    yaml_workflow.into_workflow()
}

/// Parse workflow from TOML file
pub async fn parse_toml_workflow(path: &Path) -> Result<Workflow> {
    let content = fs::read_to_string(path).map_err(|e| FuseError::InternalError(e.to_string()))?;

    let toml_workflow: TomlWorkflow =
        toml::from_str(&content).map_err(|e| FuseError::SerializationError(e.to_string()))?;

    toml_workflow.into_workflow()
}

/// Parse workflow from Markdown file (fuse.md format)
pub async fn parse_markdown_workflow(path: &Path) -> Result<Workflow> {
    let content = fs::read_to_string(path).map_err(|e| FuseError::InternalError(e.to_string()))?;

    parse_markdown_content(&content)
}

/// YAML workflow structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct YamlWorkflow {
    name: String,
    description: Option<String>,
    version: Option<String>,
    timeout: Option<u64>,
    max_iterations: Option<usize>,
    parallel: Option<bool>,

    #[serde(default)]
    environment: HashMap<String, String>,

    #[serde(default)]
    steps: Vec<YamlStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct YamlStep {
    id: String,
    name: String,
    description: Option<String>,

    #[serde(flatten)]
    action: YamlAction,

    #[serde(default)]
    depends_on: Vec<String>,

    retry: Option<YamlRetry>,
    timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum YamlAction {
    RunCommand {
        command: String,
        args: Option<Vec<String>>,
    },
    PullModel {
        model: String,
        source: Option<String>,
    },
    RunInference {
        model: String,
        prompt: String,
        parameters: Option<YamlInferenceParams>,
    },
    QuantizeModel {
        model: String,
        method: String,
        format: Option<String>,
    },
    MergeModels {
        models: Vec<String>,
        strategy: String,
        output: String,
    },
    ScanModel {
        model: String,
    },
    InspectModel {
        model: String,
    },
    LayerManipulate {
        model: String,
        operation: String,
        layer_id: Option<String>,
    },
    Custom {
        tool: String,
        parameters: HashMap<String, serde_yaml::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct YamlInferenceParams {
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct YamlRetry {
    max_attempts: Option<usize>,
    delay_ms: Option<u64>,
    backoff_multiplier: Option<f32>,
}

/// TOML workflow structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TomlWorkflow {
    name: String,
    description: Option<String>,
    version: Option<String>,
    timeout: Option<u64>,
    max_iterations: Option<usize>,
    parallel: Option<bool>,

    #[serde(default)]
    environment: HashMap<String, String>,

    #[serde(default)]
    steps: Vec<TomlStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TomlStep {
    id: String,
    name: String,
    description: Option<String>,

    #[serde(flatten)]
    action: TomlAction,

    #[serde(default)]
    depends_on: Vec<String>,

    retry: Option<TomlRetry>,
    timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum TomlAction {
    RunCommand {
        command: String,
        args: Option<Vec<String>>,
    },
    PullModel {
        model: String,
        source: Option<String>,
    },
    RunInference {
        model: String,
        prompt: String,
        parameters: Option<TomlInferenceParams>,
    },
    QuantizeModel {
        model: String,
        method: String,
        format: Option<String>,
    },
    MergeModels {
        models: Vec<String>,
        strategy: String,
        output: String,
    },
    ScanModel {
        model: String,
    },
    InspectModel {
        model: String,
    },
    LayerManipulate {
        model: String,
        operation: String,
        layer_id: Option<String>,
    },
    Custom {
        tool: String,
        parameters: HashMap<String, toml::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TomlInferenceParams {
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TomlRetry {
    max_attempts: Option<usize>,
    delay_ms: Option<u64>,
    backoff_multiplier: Option<f32>,
}

impl YamlWorkflow {
    fn into_workflow(self) -> Result<Workflow> {
        let steps: Result<Vec<WorkflowStep>> = self
            .steps
            .into_iter()
            .map(|step| step.into_step())
            .collect();

        Ok(Workflow {
            name: self.name,
            description: self.description,
            max_iterations: self.max_iterations.unwrap_or(10),
            timeout_secs: self.timeout.unwrap_or(3600),
            parallel_execution: self.parallel.unwrap_or(false),
            steps: steps?,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }
}

impl YamlStep {
    fn into_step(self) -> Result<WorkflowStep> {
        let action = self.action.into_action()?;
        let retry_policy = self.retry.map(|r| r.into_policy()).unwrap_or_default();

        Ok(WorkflowStep {
            id: self.id,
            name: self.name,
            description: self.description,
            action,
            on_success: None,
            on_failure: None,
            depends_on: self.depends_on,
            retry_policy,
            timeout_secs: self.timeout,
        })
    }
}

impl YamlAction {
    fn into_action(self) -> Result<WorkflowAction> {
        match self {
            YamlAction::RunCommand { command, args } => {
                let args = args.unwrap_or_default();
                if args.is_empty() {
                    Ok(WorkflowAction::Execute { command })
                } else {
                    Ok(WorkflowAction::Custom { command, args })
                }
            }
            YamlAction::PullModel { model, source: _ } => Ok(WorkflowAction::Execute {
                command: format!("fuse pull {}", model),
            }),
            YamlAction::RunInference {
                model,
                prompt,
                parameters: _,
            } => Ok(WorkflowAction::RunInference { model, prompt }),
            YamlAction::QuantizeModel {
                model,
                method,
                format: _,
            } => Ok(WorkflowAction::Quantize { model, method }),
            YamlAction::MergeModels {
                models,
                strategy,
                output: _,
            } => Ok(WorkflowAction::Merge { models, strategy }),
            YamlAction::ScanModel { model } => Ok(WorkflowAction::Scan { target: model }),
            YamlAction::InspectModel { model } => Ok(WorkflowAction::Execute {
                command: format!("fuse inspect {}", model),
            }),
            YamlAction::LayerManipulate {
                model,
                operation,
                layer_id,
            } => {
                let cmd = if let Some(lid) = layer_id {
                    format!("fuse layer {} {} {}", operation, model, lid)
                } else {
                    format!("fuse layer {} {}", operation, model)
                };
                Ok(WorkflowAction::Execute { command: cmd })
            }
            YamlAction::Custom {
                tool,
                parameters: _,
            } => Ok(WorkflowAction::Execute { command: tool }),
        }
    }
}

impl YamlRetry {
    fn into_policy(self) -> RetryPolicy {
        RetryPolicy {
            max_retries: self.max_attempts.unwrap_or(3),
            backoff_secs: self.delay_ms.unwrap_or(1000) / 1000,
            exponential_backoff: self.backoff_multiplier.unwrap_or(1.0) > 1.0,
            max_backoff_secs: 60,
        }
    }
}

impl TomlWorkflow {
    fn into_workflow(self) -> Result<Workflow> {
        let steps: Result<Vec<WorkflowStep>> = self
            .steps
            .into_iter()
            .map(|step| step.into_step())
            .collect();

        Ok(Workflow {
            name: self.name,
            description: self.description,
            max_iterations: self.max_iterations.unwrap_or(10),
            timeout_secs: self.timeout.unwrap_or(3600),
            parallel_execution: self.parallel.unwrap_or(false),
            steps: steps?,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }
}

impl TomlStep {
    fn into_step(self) -> Result<WorkflowStep> {
        let action = self.action.into_action()?;
        let retry_policy = self.retry.map(|r| r.into_policy()).unwrap_or_default();

        Ok(WorkflowStep {
            id: self.id,
            name: self.name,
            description: self.description,
            action,
            on_success: None,
            on_failure: None,
            depends_on: self.depends_on,
            retry_policy,
            timeout_secs: self.timeout,
        })
    }
}

impl TomlAction {
    fn into_action(self) -> Result<WorkflowAction> {
        match self {
            TomlAction::RunCommand { command, args } => {
                let args = args.unwrap_or_default();
                if args.is_empty() {
                    Ok(WorkflowAction::Execute { command })
                } else {
                    Ok(WorkflowAction::Custom { command, args })
                }
            }
            TomlAction::PullModel { model, source: _ } => Ok(WorkflowAction::Execute {
                command: format!("fuse pull {}", model),
            }),
            TomlAction::RunInference {
                model,
                prompt,
                parameters: _,
            } => Ok(WorkflowAction::RunInference { model, prompt }),
            TomlAction::QuantizeModel {
                model,
                method,
                format: _,
            } => Ok(WorkflowAction::Quantize { model, method }),
            TomlAction::MergeModels {
                models,
                strategy,
                output: _,
            } => Ok(WorkflowAction::Merge { models, strategy }),
            TomlAction::ScanModel { model } => Ok(WorkflowAction::Scan { target: model }),
            TomlAction::InspectModel { model } => Ok(WorkflowAction::Execute {
                command: format!("fuse inspect {}", model),
            }),
            TomlAction::LayerManipulate {
                model,
                operation,
                layer_id,
            } => {
                let cmd = if let Some(lid) = layer_id {
                    format!("fuse layer {} {} {}", operation, model, lid)
                } else {
                    format!("fuse layer {} {}", operation, model)
                };
                Ok(WorkflowAction::Execute { command: cmd })
            }
            TomlAction::Custom {
                tool,
                parameters: _,
            } => Ok(WorkflowAction::Execute { command: tool }),
        }
    }
}

impl TomlRetry {
    fn into_policy(self) -> RetryPolicy {
        RetryPolicy {
            max_retries: self.max_attempts.unwrap_or(3),
            backoff_secs: self.delay_ms.unwrap_or(1000) / 1000,
            exponential_backoff: self.backoff_multiplier.unwrap_or(1.0) > 1.0,
            max_backoff_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_yaml_workflow() {
        let yaml_content = r#"
name: "Test Workflow"
description: "A test workflow"
version: "1.0.0"
steps:
  - id: "step1"
    name: "Pull Model"
    action: "pull_model"
    model: "gpt2"
  - id: "step2"
    name: "Run Inference"
    action: "run_inference"
    model: "gpt2"
    prompt: "Hello world"
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.yaml");
        std::fs::write(&file_path, yaml_content).unwrap();

        let workflow = parse_yaml_workflow(&file_path).await.unwrap();
        assert_eq!(workflow.name, "Test Workflow");
        assert_eq!(workflow.steps.len(), 2);
    }

    #[tokio::test]
    async fn test_parse_toml_workflow() {
        let toml_content = r#"
name = "Test Workflow"
description = "A test workflow"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Pull Model"
action = "pull_model"
model = "gpt2"

[[steps]]
id = "step2"
name = "Run Inference"
action = "run_inference"
model = "gpt2"
prompt = "Hello world"
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.toml");
        std::fs::write(&file_path, toml_content).unwrap();

        let workflow = parse_toml_workflow(&file_path).await.unwrap();
        assert_eq!(workflow.name, "Test Workflow");
        assert_eq!(workflow.steps.len(), 2);
    }
}
