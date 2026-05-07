pub mod inspector;
pub mod manipulator;
pub mod report;
pub mod validator;

pub use inspector::{LayerInfo, LayerInspector};
pub use manipulator::{LayerConfig, LayerManipulator, LayerType};
pub use report::{ReportFormat, ReportGenerator};
pub use validator::{ModelValidator, ValidationReport};

use crate::error::Result;
use std::path::Path;

pub struct LayerService {
    inspector: LayerInspector,
    manipulator: LayerManipulator,
    validator: ModelValidator,
    report_generator: ReportGenerator,
}

impl LayerService {
    pub fn new(workspace_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inspector: LayerInspector::new(),
            manipulator: LayerManipulator::new(),
            validator: ModelValidator::new(),
            report_generator: ReportGenerator::new(workspace_dir)?,
        })
    }

    pub async fn inspect_layers(&self, model_path: &Path, wide: bool) -> Result<Vec<LayerInfo>> {
        self.inspector.inspect(model_path, wide).await
    }

    pub async fn remove_layer(&self, model_path: &Path, layer_id: &str) -> Result<()> {
        self.manipulator.remove_layer(model_path, layer_id).await
    }

    pub async fn add_layer(
        &self,
        model_path: &Path,
        layer_type: LayerType,
        config: LayerConfig,
    ) -> Result<()> {
        self.manipulator
            .add_layer(model_path, layer_type, config)
            .await
    }

    pub async fn validate_model(&self, model_path: &Path) -> Result<ValidationReport> {
        self.validator.validate(model_path).await
    }

    pub async fn generate_inspection_report(
        &self,
        model_name: &str,
        layers: &[LayerInfo],
        format: ReportFormat,
        output_path: Option<&Path>,
    ) -> Result<String> {
        self.report_generator
            .generate_inspection_report(model_name, layers, format, output_path)
            .await
    }

    pub async fn generate_validation_report(
        &self,
        model_name: &str,
        report: &ValidationReport,
        format: ReportFormat,
        output_path: Option<&Path>,
    ) -> Result<String> {
        self.report_generator
            .generate_validation_report(model_name, report, format, output_path)
            .await
    }
}
