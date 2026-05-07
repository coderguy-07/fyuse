use crate::error::{FuseError, Result};
use crate::workflow::{ExecutionContext, WorkflowExecutor};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub success: bool,
    pub errors: Vec<CompileError>,
    pub warnings: Vec<CompileWarning>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileError {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileWarning {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
    pub failures: Vec<TestFailure>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    pub test_name: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

pub struct CompileFixTestLoop {
    workspace_dir: PathBuf,
    executor: WorkflowExecutor,
    max_iterations: u32,
}

impl CompileFixTestLoop {
    pub fn new(workspace_dir: &Path) -> Result<Self> {
        Ok(Self {
            workspace_dir: workspace_dir.to_path_buf(),
            executor: WorkflowExecutor::new(),
            max_iterations: 5,
        })
    }
    
    pub async fn run_loop(&self) -> Result<()> {
        info!("Starting compile-fix-test loop");
        
        let mut iteration = 0;
        
        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                warn!("Maximum iterations ({}) reached", self.max_iterations);
                break;
            }
            
            info!("Iteration {}", iteration);
            
            let compile_result = self.compile().await?;
            
            if compile_result.success {
                info!("Compilation successful");
                
                let test_result = self.test().await?;
                
                if test_result.success {
                    info!("All tests passed!");
                    break;
                } else {
                    info!("Tests failed, attempting to fix...");
                    self.fix_tests(&test_result).await?;
                }
            } else {
                info!("Compilation failed, attempting to fix...");
                self.fix_compilation(&compile_result).await?;
            }
        }
        
        Ok(())
    }
    
    async fn compile(&self) -> Result<CompileResult> {
        info!("Running compilation check");
        
        let output = Command::new("cargo")
            .args(["check", "--message-format=json"])
            .current_dir(&self.workspace_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let full_output = format!("{}{}", stdout, stderr);
        
        let success = output.status.success();
        let (errors, warnings) = self.parse_cargo_output(&stdout);
        
        Ok(CompileResult {
            success,
            errors,
            warnings,
            output: full_output,
        })
    }
    
    async fn test(&self) -> Result<TestResult> {
        info!("Running tests");
        
        let output = Command::new("cargo")
            .args(["test"])
            .current_dir(&self.workspace_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let success = output.status.success();
        let (passed, failed, ignored, failures) = self.parse_test_output(&output_str);
        
        Ok(TestResult {
            success,
            passed,
            failed,
            ignored,
            failures,
            output: output_str.to_string(),
        })
    }
    
    async fn fix_compilation(&self, compile_result: &CompileResult) -> Result<()> {
        info!("Attempting to fix compilation errors");
        
        let mut context = ExecutionContext::default();
        let error_summary = self.format_compile_errors(&compile_result.errors);
        context.compilation_errors = Some(error_summary);
        
        if let Some(first_error) = compile_result.errors.first() {
            if let Ok(code_context) = self.get_file_context(&first_error.file, first_error.line).await {
                context.code_context = Some(code_context);
            }
        }
        
        warn!("Compilation errors: {}", context.compilation_errors.as_ref().unwrap());
        Ok(())
    }
    
    async fn fix_tests(&self, test_result: &TestResult) -> Result<()> {
        info!("Attempting to fix test failures");
        
        let mut context = ExecutionContext::default();
        let failure_summary = self.format_test_failures(&test_result.failures);
        context.test_failures = Some(failure_summary);
        
        warn!("Test failures: {}", context.test_failures.as_ref().unwrap());
        Ok(())
    }
    
    fn parse_cargo_output(&self, output: &str) -> (Vec<CompileError>, Vec<CompileWarning>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        for line in output.lines() {
            if let Ok(message) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(reason) = message.get("reason").and_then(|r| r.as_str()) {
                    if reason == "compiler-message" {
                        if let Some(msg_obj) = message.get("message") {
                            if let Some(level) = msg_obj.get("level").and_then(|l| l.as_str()) {
                                let msg = msg_obj.get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                
                                let code = msg_obj.get("code")
                                    .and_then(|c| c.get("code"))
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string());
                                
                                let (file, line, column) = self.extract_span_info(msg_obj);
                                
                                match level {
                                    "error" => errors.push(CompileError { file, line, column, message: msg, code }),
                                    "warning" => warnings.push(CompileWarning { file, line, column, message: msg, code }),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        
        (errors, warnings)
    }
    
    fn extract_span_info(&self, msg_obj: &serde_json::Value) -> (String, u32, u32) {
        if let Some(spans) = msg_obj.get("spans").and_then(|s| s.as_array()) {
            if let Some(primary) = spans.iter().find(|s| {
                s.get("is_primary").and_then(|p| p.as_bool()).unwrap_or(false)
            }) {
                let file = primary.get("file_name")
                    .and_then(|f| f.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let line = primary.get("line_start")
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0) as u32;
                let column = primary.get("column_start")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as u32;
                return (file, line, column);
            }
        }
        ("unknown".to_string(), 0, 0)
    }
    
    fn parse_test_output(&self, output: &str) -> (u32, u32, u32, Vec<TestFailure>) {
        let mut passed = 0;
        let mut failed = 0;
        let mut ignored = 0;
        let mut failures = Vec::new();
        
        for line in output.lines() {
            if line.contains("test result:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if part == &"passed;" && i > 0 {
                        passed = parts[i - 1].parse().unwrap_or(0);
                    } else if part == &"failed;" && i > 0 {
                        failed = parts[i - 1].parse().unwrap_or(0);
                    } else if part == &"ignored" && i > 0 {
                        ignored = parts[i - 1].parse().unwrap_or(0);
                    }
                }
            } else if line.contains("FAILED") {
                let test_name = line.split_whitespace().next().unwrap_or("unknown").to_string();
                failures.push(TestFailure {
                    test_name,
                    message: line.to_string(),
                    file: None,
                    line: None,
                });
            }
        }
        
        (passed, failed, ignored, failures)
    }
    
    fn format_compile_errors(&self, errors: &[CompileError]) -> String {
        errors.iter()
            .map(|e| format!("{}:{}:{}: {}", e.file, e.line, e.column, e.message))
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    fn format_test_failures(&self, failures: &[TestFailure]) -> String {
        failures.iter()
            .map(|f| format!("Test '{}': {}", f.test_name, f.message))
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    async fn get_file_context(&self, file: &str, line: u32) -> Result<String> {
        let path = self.workspace_dir.join(file);
        let content = tokio::fs::read_to_string(&path).await?;
        
        let lines: Vec<&str> = content.lines().collect();
        let start = line.saturating_sub(5) as usize;
        let end = ((line + 5) as usize).min(lines.len());
        
        Ok(lines[start..end].join("\n"))
    }
}
