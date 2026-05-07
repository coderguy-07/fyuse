//! Interactive chat TUI — high-performance terminal UI for Fuse.
//!
//! Provides a rich, GUI-like terminal UI with:
//! - 120Hz render loop with diff-based rendering
//! - Multi-turn conversation with streaming
//! - Sidebar navigation (Chat, Models, Sessions, Settings)
//! - Command palette (/ key) with fuzzy matching
//! - Help overlay (? key)
//! - Mouse scroll support
//! - Dark/light theme toggle
//! - Markdown rendering with code blocks
//! - Scrollbar and scroll indicators
//!
//! Gated behind the `tui` feature flag.

#[cfg(feature = "tui")]
pub mod app;
pub mod chat;
pub mod render;
#[cfg(feature = "tui")]
pub mod theme;
#[cfg(feature = "tui")]
pub mod widgets;

#[cfg(feature = "tui")]
pub use app::run_tui;

// Fallback when TUI feature is not enabled
#[cfg(not(feature = "tui"))]
pub async fn run_tui(
    _model: &str,
    _config: &crate::config::FuseConfig,
) -> crate::error::Result<()> {
    Err(crate::error::FuseError::FeatureDisabled(
        "TUI requires the 'tui' feature flag. Rebuild with: cargo build --features tui".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tui_module_exists() {
        assert!(true);
    }

    #[cfg(not(feature = "tui"))]
    #[tokio::test]
    async fn test_tui_disabled_returns_error() {
        let config = crate::config::FuseConfig::default();
        let result = super::run_tui("test-model", &config).await;
        assert!(result.is_err());
    }
}
