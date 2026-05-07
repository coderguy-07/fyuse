//! Channel router — routes messages to the correct model per channel config.

use super::ChannelType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-channel routing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRouteConfig {
    pub channel_type: ChannelType,
    pub model_name: String,
    pub system_prompt: Option<String>,
    pub max_tokens: usize,
}

/// Routes incoming messages to the correct model based on channel type.
#[derive(Debug, Clone)]
pub struct ChannelRouter {
    routes: HashMap<ChannelType, ChannelRouteConfig>,
    fallback_model: String,
    fallback_max_tokens: usize,
}

impl ChannelRouter {
    /// Create a new router with a fallback model.
    pub fn new(fallback_model: &str, fallback_max_tokens: usize) -> Self {
        Self {
            routes: HashMap::new(),
            fallback_model: fallback_model.to_string(),
            fallback_max_tokens,
        }
    }

    /// Add a route for a channel type.
    pub fn add_route(&mut self, config: ChannelRouteConfig) {
        self.routes.insert(config.channel_type, config);
    }

    /// Route a message — returns (model_name, system_prompt, max_tokens).
    pub fn route(&self, channel_type: ChannelType) -> (&str, Option<&str>, usize) {
        match self.routes.get(&channel_type) {
            Some(config) => (
                &config.model_name,
                config.system_prompt.as_deref(),
                config.max_tokens,
            ),
            None => (&self.fallback_model, None, self.fallback_max_tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_with_config() {
        let mut router = ChannelRouter::new("default-model", 1024);
        router.add_route(ChannelRouteConfig {
            channel_type: ChannelType::Telegram,
            model_name: "llama-7b".to_string(),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            max_tokens: 2048,
        });

        let (model, prompt, tokens) = router.route(ChannelType::Telegram);
        assert_eq!(model, "llama-7b");
        assert_eq!(prompt, Some("You are a helpful assistant."));
        assert_eq!(tokens, 2048);
    }

    #[test]
    fn test_route_fallback() {
        let router = ChannelRouter::new("fallback-model", 512);

        let (model, prompt, tokens) = router.route(ChannelType::Discord);
        assert_eq!(model, "fallback-model");
        assert!(prompt.is_none());
        assert_eq!(tokens, 512);
    }

    #[test]
    fn test_multiple_routes() {
        let mut router = ChannelRouter::new("default", 1024);
        router.add_route(ChannelRouteConfig {
            channel_type: ChannelType::Telegram,
            model_name: "model-a".to_string(),
            system_prompt: None,
            max_tokens: 512,
        });
        router.add_route(ChannelRouteConfig {
            channel_type: ChannelType::Slack,
            model_name: "model-b".to_string(),
            system_prompt: Some("Slack bot".to_string()),
            max_tokens: 4096,
        });

        let (m, _, _) = router.route(ChannelType::Telegram);
        assert_eq!(m, "model-a");

        let (m, p, _) = router.route(ChannelType::Slack);
        assert_eq!(m, "model-b");
        assert_eq!(p, Some("Slack bot"));

        // Unconfigured falls back
        let (m, _, _) = router.route(ChannelType::Matrix);
        assert_eq!(m, "default");
    }

    #[test]
    fn test_route_override() {
        let mut router = ChannelRouter::new("default", 1024);
        router.add_route(ChannelRouteConfig {
            channel_type: ChannelType::Discord,
            model_name: "old-model".to_string(),
            system_prompt: None,
            max_tokens: 512,
        });
        router.add_route(ChannelRouteConfig {
            channel_type: ChannelType::Discord,
            model_name: "new-model".to_string(),
            system_prompt: None,
            max_tokens: 1024,
        });

        let (m, _, tokens) = router.route(ChannelType::Discord);
        assert_eq!(m, "new-model");
        assert_eq!(tokens, 1024);
    }
}
