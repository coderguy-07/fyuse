//! Skill trait — reusable AI capabilities (inspired by OpenClaw).

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillTrigger {
    Command(String),
    Pattern(String),
    Event(String),
}

#[derive(Debug, Clone)]
pub struct SkillContext {
    pub session_id: String,
    pub user_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    pub text: String,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub text: String,
    pub data: Option<serde_json::Value>,
}

/// Core trait for reusable AI skills.
#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn triggers(&self) -> &[SkillTrigger];
    async fn execute(&self, ctx: &SkillContext, input: &SkillInput) -> Result<SkillOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockSkill;

    #[async_trait]
    impl Skill for MockSkill {
        fn name(&self) -> &str {
            "summarize"
        }
        fn description(&self) -> &str {
            "Summarize text"
        }
        fn triggers(&self) -> &[SkillTrigger] {
            &[]
        }
        async fn execute(&self, _ctx: &SkillContext, input: &SkillInput) -> Result<SkillOutput> {
            Ok(SkillOutput {
                text: format!("Summary of: {}", input.text),
                data: None,
            })
        }
    }

    #[tokio::test]
    async fn test_skill_execute() {
        let skill = MockSkill;
        assert_eq!(skill.name(), "summarize");

        let ctx = SkillContext {
            session_id: "s1".to_string(),
            user_id: None,
            model: None,
        };
        let input = SkillInput {
            text: "Long text here".to_string(),
            parameters: HashMap::new(),
        };
        let output = skill.execute(&ctx, &input).await.unwrap();
        assert!(output.text.contains("Long text here"));
    }
}
