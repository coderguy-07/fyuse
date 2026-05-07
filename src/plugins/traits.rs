//! Plugin trait — extensibility interface.

use crate::error::Result;

/// Context provided to plugins on load.
pub struct PluginContext {
    pub data_dir: std::path::PathBuf,
    pub config: std::collections::HashMap<String, String>,
}

/// Core trait for plugins.
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&mut self, ctx: &PluginContext) -> Result<()>;
    fn on_unload(&mut self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct MockPlugin {
        loaded: bool,
    }

    impl Plugin for MockPlugin {
        fn name(&self) -> &str {
            "mock-plugin"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            self.loaded = true;
            Ok(())
        }
        fn on_unload(&mut self) -> Result<()> {
            self.loaded = false;
            Ok(())
        }
    }

    #[test]
    fn test_plugin_lifecycle() {
        let mut plugin = MockPlugin { loaded: false };
        assert_eq!(plugin.name(), "mock-plugin");
        assert_eq!(plugin.version(), "0.1.0");

        let ctx = PluginContext {
            data_dir: PathBuf::from("/tmp"),
            config: HashMap::new(),
        };

        plugin.on_load(&ctx).unwrap();
        assert!(plugin.loaded);

        plugin.on_unload().unwrap();
        assert!(!plugin.loaded);
    }
}
