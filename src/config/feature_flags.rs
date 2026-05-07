use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Feature flags for enabling/disabling optional capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureFlags {
    #[serde(default)]
    pub agentic_coding: bool,

    #[serde(default)]
    pub thinking_visualization: bool,

    #[serde(default)]
    pub generative_ui: bool,

    #[serde(default)]
    pub mcp_server: bool,

    #[serde(default)]
    pub vulnerability_scanning: bool,
}

impl FeatureFlags {
    /// Check if a feature is enabled
    pub fn is_enabled(&self, feature: Feature) -> bool {
        match feature {
            Feature::AgenticCoding => self.agentic_coding,
            Feature::ThinkingVisualization => self.thinking_visualization,
            Feature::GenerativeUi => self.generative_ui,
            Feature::McpServer => self.mcp_server,
            Feature::VulnerabilityScanning => self.vulnerability_scanning,
        }
    }

    /// Enable a feature
    pub fn enable(&mut self, feature: Feature) {
        match feature {
            Feature::AgenticCoding => self.agentic_coding = true,
            Feature::ThinkingVisualization => self.thinking_visualization = true,
            Feature::GenerativeUi => self.generative_ui = true,
            Feature::McpServer => self.mcp_server = true,
            Feature::VulnerabilityScanning => self.vulnerability_scanning = true,
        }
    }

    /// Disable a feature
    pub fn disable(&mut self, feature: Feature) {
        match feature {
            Feature::AgenticCoding => self.agentic_coding = false,
            Feature::ThinkingVisualization => self.thinking_visualization = false,
            Feature::GenerativeUi => self.generative_ui = false,
            Feature::McpServer => self.mcp_server = false,
            Feature::VulnerabilityScanning => self.vulnerability_scanning = false,
        }
    }
}

/// Available features that can be toggled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    AgenticCoding,
    ThinkingVisualization,
    GenerativeUi,
    McpServer,
    VulnerabilityScanning,
}

impl Feature {
    /// Get all available features
    pub fn all() -> Vec<Feature> {
        vec![
            Feature::AgenticCoding,
            Feature::ThinkingVisualization,
            Feature::GenerativeUi,
            Feature::McpServer,
            Feature::VulnerabilityScanning,
        ]
    }

    /// Get feature name as string
    pub fn name(&self) -> &'static str {
        match self {
            Feature::AgenticCoding => "agentic-coding",
            Feature::ThinkingVisualization => "thinking-visualization",
            Feature::GenerativeUi => "generative-ui",
            Feature::McpServer => "mcp-server",
            Feature::VulnerabilityScanning => "vulnerability-scanning",
        }
    }

    /// Get feature description
    pub fn description(&self) -> &'static str {
        match self {
            Feature::AgenticCoding => "Automated workflow execution with fix-compile-test loops",
            Feature::ThinkingVisualization => {
                "Display model thinking and planning stages in real-time"
            }
            Feature::GenerativeUi => "Interactive UI with real-time action feedback",
            Feature::McpServer => "Model Context Protocol server support",
            Feature::VulnerabilityScanning => "Scan models for security vulnerabilities",
        }
    }

    /// Parse feature from string
    pub fn parse_feature(s: &str) -> Option<Feature> {
        match s.to_lowercase().as_str() {
            "agentic-coding" | "agentic_coding" => Some(Feature::AgenticCoding),
            "thinking-visualization" | "thinking_visualization" => {
                Some(Feature::ThinkingVisualization)
            }
            "generative-ui" | "generative_ui" => Some(Feature::GenerativeUi),
            "mcp-server" | "mcp_server" => Some(Feature::McpServer),
            "vulnerability-scanning" | "vulnerability_scanning" => {
                Some(Feature::VulnerabilityScanning)
            }
            _ => None,
        }
    }
}

/// Thread-safe feature flag manager
#[derive(Debug, Clone)]
pub struct FeatureFlagManager {
    flags: Arc<RwLock<FeatureFlags>>,
}

impl FeatureFlagManager {
    /// Create a new feature flag manager
    pub fn new(flags: FeatureFlags) -> Self {
        Self {
            flags: Arc::new(RwLock::new(flags)),
        }
    }

    /// Check if a feature is enabled
    pub fn is_enabled(&self, feature: Feature) -> bool {
        self.flags.read().is_enabled(feature)
    }

    /// Enable a feature
    pub fn enable(&self, feature: Feature) {
        self.flags.write().enable(feature);
    }

    /// Disable a feature
    pub fn disable(&self, feature: Feature) {
        self.flags.write().disable(feature);
    }

    /// Get current feature flags
    pub fn get_flags(&self) -> FeatureFlags {
        self.flags.read().clone()
    }

    /// Check if feature is enabled, return error if not
    pub fn require_feature(&self, feature: Feature) -> Result<(), String> {
        if self.is_enabled(feature) {
            Ok(())
        } else {
            Err(format!(
                "Feature '{}' is not enabled. Enable it in your configuration file or use 'fuse features enable {}'",
                feature.name(),
                feature.name()
            ))
        }
    }
}

impl Default for FeatureFlagManager {
    fn default() -> Self {
        Self::new(FeatureFlags::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flags_default() {
        let flags = FeatureFlags::default();
        assert!(!flags.agentic_coding);
        assert!(!flags.thinking_visualization);
        assert!(!flags.generative_ui);
        assert!(!flags.mcp_server);
        assert!(!flags.vulnerability_scanning);
    }

    #[test]
    fn test_feature_flags_is_enabled() {
        let mut flags = FeatureFlags::default();
        flags.agentic_coding = true;

        assert!(flags.is_enabled(Feature::AgenticCoding));
        assert!(!flags.is_enabled(Feature::ThinkingVisualization));
    }

    #[test]
    fn test_feature_flags_enable() {
        let mut flags = FeatureFlags::default();
        flags.enable(Feature::ThinkingVisualization);

        assert!(flags.thinking_visualization);
        assert!(flags.is_enabled(Feature::ThinkingVisualization));
    }

    #[test]
    fn test_feature_flags_disable() {
        let mut flags = FeatureFlags::default();
        flags.agentic_coding = true;
        flags.disable(Feature::AgenticCoding);

        assert!(!flags.agentic_coding);
        assert!(!flags.is_enabled(Feature::AgenticCoding));
    }

    #[test]
    fn test_feature_all() {
        let features = Feature::all();
        assert_eq!(features.len(), 5);
        assert!(features.contains(&Feature::AgenticCoding));
        assert!(features.contains(&Feature::ThinkingVisualization));
        assert!(features.contains(&Feature::GenerativeUi));
        assert!(features.contains(&Feature::McpServer));
        assert!(features.contains(&Feature::VulnerabilityScanning));
    }

    #[test]
    fn test_feature_name() {
        assert_eq!(Feature::AgenticCoding.name(), "agentic-coding");
        assert_eq!(
            Feature::ThinkingVisualization.name(),
            "thinking-visualization"
        );
        assert_eq!(Feature::GenerativeUi.name(), "generative-ui");
        assert_eq!(Feature::McpServer.name(), "mcp-server");
        assert_eq!(
            Feature::VulnerabilityScanning.name(),
            "vulnerability-scanning"
        );
    }

    #[test]
    fn test_feature_description() {
        let desc = Feature::AgenticCoding.description();
        assert!(desc.contains("Automated workflow"));

        let desc = Feature::ThinkingVisualization.description();
        assert!(desc.contains("thinking"));
    }

    #[test]
    fn test_feature_from_str() {
        assert_eq!(
            Feature::parse_feature("agentic-coding"),
            Some(Feature::AgenticCoding)
        );
        assert_eq!(
            Feature::parse_feature("agentic_coding"),
            Some(Feature::AgenticCoding)
        );
        assert_eq!(
            Feature::parse_feature("thinking-visualization"),
            Some(Feature::ThinkingVisualization)
        );
        assert_eq!(
            Feature::parse_feature("generative-ui"),
            Some(Feature::GenerativeUi)
        );
        assert_eq!(
            Feature::parse_feature("mcp-server"),
            Some(Feature::McpServer)
        );
        assert_eq!(
            Feature::parse_feature("vulnerability-scanning"),
            Some(Feature::VulnerabilityScanning)
        );
        assert_eq!(Feature::parse_feature("invalid"), None);
    }

    #[test]
    fn test_feature_flag_manager_new() {
        let flags = FeatureFlags::default();
        let manager = FeatureFlagManager::new(flags);

        assert!(!manager.is_enabled(Feature::AgenticCoding));
    }

    #[test]
    fn test_feature_flag_manager_enable() {
        let flags = FeatureFlags::default();
        let manager = FeatureFlagManager::new(flags);

        manager.enable(Feature::AgenticCoding);
        assert!(manager.is_enabled(Feature::AgenticCoding));
    }

    #[test]
    fn test_feature_flag_manager_disable() {
        let mut flags = FeatureFlags::default();
        flags.agentic_coding = true;
        let manager = FeatureFlagManager::new(flags);

        assert!(manager.is_enabled(Feature::AgenticCoding));
        manager.disable(Feature::AgenticCoding);
        assert!(!manager.is_enabled(Feature::AgenticCoding));
    }

    #[test]
    fn test_feature_flag_manager_get_flags() {
        let mut flags = FeatureFlags::default();
        flags.agentic_coding = true;
        let manager = FeatureFlagManager::new(flags.clone());

        let retrieved_flags = manager.get_flags();
        assert_eq!(retrieved_flags.agentic_coding, flags.agentic_coding);
    }

    #[test]
    fn test_feature_flag_manager_require_feature_enabled() {
        let mut flags = FeatureFlags::default();
        flags.agentic_coding = true;
        let manager = FeatureFlagManager::new(flags);

        assert!(manager.require_feature(Feature::AgenticCoding).is_ok());
    }

    #[test]
    fn test_feature_flag_manager_require_feature_disabled() {
        let flags = FeatureFlags::default();
        let manager = FeatureFlagManager::new(flags);

        let result = manager.require_feature(Feature::AgenticCoding);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not enabled"));
    }

    #[test]
    fn test_feature_flag_manager_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let flags = FeatureFlags::default();
        let manager = Arc::new(FeatureFlagManager::new(flags));

        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            manager_clone.enable(Feature::AgenticCoding);
        });

        handle.join().unwrap();
        assert!(manager.is_enabled(Feature::AgenticCoding));
    }

    #[test]
    fn test_feature_flag_manager_default() {
        let manager = FeatureFlagManager::default();
        assert!(!manager.is_enabled(Feature::AgenticCoding));
        assert!(!manager.is_enabled(Feature::ThinkingVisualization));
    }
}
