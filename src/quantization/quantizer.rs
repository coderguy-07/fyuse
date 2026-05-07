use crate::error::{FuseError, Result};
use crate::quantization::{QuantizationConfig, QuantizationMethod};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

pub struct Quantizer {
    workspace_dir: PathBuf,
}

impl Quantizer {
    pub fn new(workspace_dir: impl AsRef<Path>) -> Self {
        Self {
            workspace_dir: workspace_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn quantize(
        &self,
        model_path: &Path,
        output_path: &Path,
        config: &QuantizationConfig,
    ) -> Result<()> {
        config.validate()?;

        info!("Starting quantization with method: {:?}", config.method);
        info!("Input: {}", model_path.display());
        info!("Output: {}", output_path.display());

        match config.method {
            QuantizationMethod::Q4_0
            | QuantizationMethod::Q4_1
            | QuantizationMethod::Q5_0
            | QuantizationMethod::Q5_1
            | QuantizationMethod::Q8_0
            | QuantizationMethod::Q4_K_M
            | QuantizationMethod::Q5_K_M
            | QuantizationMethod::Q6_K => self.quantize_gguf(model_path, output_path, config).await,
            QuantizationMethod::GPTQ => self.quantize_gptq(model_path, output_path, config).await,
            QuantizationMethod::AWQ => self.quantize_awq(model_path, output_path, config).await,
            QuantizationMethod::GGML => self.quantize_ggml(model_path, output_path, config).await,
        }
    }

    async fn quantize_gguf(
        &self,
        model_path: &Path,
        output_path: &Path,
        config: &QuantizationConfig,
    ) -> Result<()> {
        info!("Quantizing to GGUF format: {}", config.method.as_str());

        // Check if llama.cpp quantize tool is available
        let quantize_tool = self.find_llama_cpp_quantize()?;

        let mut cmd = Command::new(quantize_tool);
        cmd.arg(model_path)
            .arg(output_path)
            .arg(config.method.as_str());

        if let Some(block_size) = config.block_size {
            cmd.arg("--block-size").arg(block_size.to_string());
        }

        info!("Running quantization command...");
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FuseError::QuantizationError(format!(
                "Quantization failed: {}",
                stderr
            )));
        }

        info!("GGUF quantization completed successfully");
        Ok(())
    }

    async fn quantize_gptq(
        &self,
        model_path: &Path,
        output_path: &Path,
        config: &QuantizationConfig,
    ) -> Result<()> {
        info!("Quantizing with GPTQ method");

        let calibration_dataset = config.calibration_dataset.as_ref().ok_or_else(|| {
            FuseError::ValidationError("GPTQ requires calibration dataset".to_string())
        })?;

        // Check if auto-gptq is available
        let python = self.find_python()?;

        // Create a temporary Python script for GPTQ quantization
        let script = self.create_gptq_script(
            model_path,
            output_path,
            calibration_dataset,
            config.num_samples.unwrap_or(128),
        )?;

        info!("Running GPTQ quantization script...");
        let output = Command::new(python)
            .arg(&script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        // Clean up script
        let _ = tokio::fs::remove_file(&script).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FuseError::QuantizationError(format!(
                "GPTQ quantization failed: {}",
                stderr
            )));
        }

        info!("GPTQ quantization completed successfully");
        Ok(())
    }

    async fn quantize_awq(
        &self,
        model_path: &Path,
        output_path: &Path,
        config: &QuantizationConfig,
    ) -> Result<()> {
        info!("Quantizing with AWQ method");

        let python = self.find_python()?;

        // Create a temporary Python script for AWQ quantization
        let script =
            self.create_awq_script(model_path, output_path, config.num_samples.unwrap_or(128))?;

        info!("Running AWQ quantization script...");
        let output = Command::new(python)
            .arg(&script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        // Clean up script
        let _ = tokio::fs::remove_file(&script).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FuseError::QuantizationError(format!(
                "AWQ quantization failed: {}",
                stderr
            )));
        }

        info!("AWQ quantization completed successfully");
        Ok(())
    }

    async fn quantize_ggml(
        &self,
        model_path: &Path,
        output_path: &Path,
        _config: &QuantizationConfig,
    ) -> Result<()> {
        info!("Quantizing to GGML format");

        // GGML quantization is similar to GGUF but uses older format
        let quantize_tool = self.find_ggml_quantize()?;

        let output = Command::new(quantize_tool)
            .arg(model_path)
            .arg(output_path)
            .arg("q4_0") // Default GGML quantization
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FuseError::QuantizationError(format!(
                "GGML quantization failed: {}",
                stderr
            )));
        }

        info!("GGML quantization completed successfully");
        Ok(())
    }

    fn find_llama_cpp_quantize(&self) -> Result<PathBuf> {
        // Try common locations for llama.cpp quantize tool
        let possible_paths = vec![
            "quantize",
            "llama-quantize",
            "./llama.cpp/quantize",
            "/usr/local/bin/quantize",
        ];

        for path in possible_paths {
            if let Ok(output) = std::process::Command::new(path).arg("--help").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        Err(FuseError::QuantizationError(
            "llama.cpp quantize tool not found. Please install llama.cpp.".to_string(),
        ))
    }

    fn find_ggml_quantize(&self) -> Result<PathBuf> {
        let possible_paths = vec![
            "ggml-quantize",
            "./ggml/quantize",
            "/usr/local/bin/ggml-quantize",
        ];

        for path in possible_paths {
            if let Ok(output) = std::process::Command::new(path).arg("--help").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        Err(FuseError::QuantizationError(
            "GGML quantize tool not found.".to_string(),
        ))
    }

    fn find_python(&self) -> Result<PathBuf> {
        for python in &["python3", "python"] {
            if let Ok(output) = std::process::Command::new(python).arg("--version").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(python));
                }
            }
        }

        Err(FuseError::QuantizationError(
            "Python not found. Required for GPTQ/AWQ quantization.".to_string(),
        ))
    }

    fn create_gptq_script(
        &self,
        model_path: &Path,
        output_path: &Path,
        calibration_dataset: &str,
        num_samples: usize,
    ) -> Result<PathBuf> {
        let script_content = format!(
            r#"
import sys
try:
    from auto_gptq import AutoGPTQForCausalLM, BaseQuantizeConfig
    from transformers import AutoTokenizer
except ImportError:
    print("Error: auto-gptq not installed. Install with: pip install auto-gptq", file=sys.stderr)
    sys.exit(1)

model_path = "{}"
output_path = "{}"
calibration_dataset = "{}"
num_samples = {}

print(f"Loading model from {{model_path}}...")
tokenizer = AutoTokenizer.from_pretrained(model_path)
model = AutoGPTQForCausalLM.from_pretrained(model_path, quantize_config=None)

print("Configuring quantization...")
quantize_config = BaseQuantizeConfig(
    bits=4,
    group_size=128,
    desc_act=False,
)

print(f"Loading calibration data from {{calibration_dataset}}...")
# Load calibration data (simplified)
calibration_data = []
with open(calibration_dataset, 'r') as f:
    for i, line in enumerate(f):
        if i >= num_samples:
            break
        calibration_data.append(line.strip())

print("Quantizing model...")
model.quantize(calibration_data, quantize_config=quantize_config)

print(f"Saving quantized model to {{output_path}}...")
model.save_quantized(output_path)
tokenizer.save_pretrained(output_path)

print("GPTQ quantization completed successfully!")
"#,
            model_path.display(),
            output_path.display(),
            calibration_dataset,
            num_samples
        );

        let script_path = self.workspace_dir.join("gptq_quantize_temp.py");
        std::fs::write(&script_path, script_content)?;
        Ok(script_path)
    }

    fn create_awq_script(
        &self,
        model_path: &Path,
        output_path: &Path,
        num_samples: usize,
    ) -> Result<PathBuf> {
        let script_content = format!(
            r#"
import sys
try:
    from awq import AutoAWQForCausalLM
    from transformers import AutoTokenizer
except ImportError:
    print("Error: autoawq not installed. Install with: pip install autoawq", file=sys.stderr)
    sys.exit(1)

model_path = "{}"
output_path = "{}"
num_samples = {}

print(f"Loading model from {{model_path}}...")
model = AutoAWQForCausalLM.from_pretrained(model_path)
tokenizer = AutoTokenizer.from_pretrained(model_path)

print("Configuring AWQ quantization...")
quant_config = {{
    "zero_point": True,
    "q_group_size": 128,
    "w_bit": 4,
    "version": "GEMM"
}}

print("Quantizing model with AWQ...")
model.quantize(tokenizer, quant_config=quant_config)

print(f"Saving quantized model to {{output_path}}...")
model.save_quantized(output_path)
tokenizer.save_pretrained(output_path)

print("AWQ quantization completed successfully!")
"#,
            model_path.display(),
            output_path.display(),
            num_samples
        );

        let script_path = self.workspace_dir.join("awq_quantize_temp.py");
        std::fs::write(&script_path, script_content)?;
        Ok(script_path)
    }

    pub async fn validate_quantized_model(&self, model_path: &Path) -> Result<bool> {
        info!("Validating quantized model: {}", model_path.display());

        // Basic validation: check if file exists and has reasonable size
        if !model_path.exists() {
            return Ok(false);
        }

        let metadata = tokio::fs::metadata(model_path).await?;
        if metadata.len() == 0 {
            return Ok(false);
        }

        // TODO: Add more sophisticated validation
        // - Check file format
        // - Verify tensor shapes
        // - Test inference

        info!("Model validation passed");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_quantizer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let quantizer = Quantizer::new(temp_dir.path());
        assert_eq!(quantizer.workspace_dir, temp_dir.path());
    }

    #[test]
    fn test_find_python() {
        let temp_dir = TempDir::new().unwrap();
        let quantizer = Quantizer::new(temp_dir.path());

        // This test will pass if Python is installed
        let result = quantizer.find_python();
        if result.is_ok() {
            assert!(result.unwrap().to_string_lossy().contains("python"));
        }
    }
}
