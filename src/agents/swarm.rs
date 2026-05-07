//! Multi-agent swarm orchestration [8.4].

use crate::error::{FuseError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Output from a single agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// Name of the agent that produced this output.
    pub agent_name: String,
    /// The output text/data.
    pub content: String,
    /// Confidence score (0.0 - 1.0).
    pub score: f64,
    /// Optional structured data.
    pub metadata: Option<serde_json::Value>,
}

/// Strategy for reaching consensus among multiple agents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusStrategy {
    /// Use the first agent to complete.
    FirstComplete,
    /// Use the majority answer (by content equality).
    Majority,
    /// Use the output with the highest score.
    #[default]
    BestScore,
}

/// Configuration for the agent swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// Maximum number of concurrent agents.
    pub max_agents: usize,
    /// Timeout for the entire orchestration.
    pub timeout: Duration,
    /// Strategy for consensus.
    pub consensus_strategy: ConsensusStrategy,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_agents: 10,
            timeout: Duration::from_secs(60),
            consensus_strategy: ConsensusStrategy::BestScore,
        }
    }
}

impl SwarmConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.max_agents == 0 {
            return Err(FuseError::ValidationError(
                "max_agents must be greater than 0".to_string(),
            ));
        }
        if self.timeout.is_zero() {
            return Err(FuseError::ValidationError(
                "timeout must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// A task to be executed by agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// Task description / prompt.
    pub description: String,
    /// Optional context data.
    pub context: Option<serde_json::Value>,
}

/// Core agent trait — each agent can execute tasks.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent name.
    fn name(&self) -> &str;

    /// List of capabilities this agent supports.
    fn capabilities(&self) -> Vec<String>;

    /// Execute a task and return output.
    async fn execute(&self, task: &AgentTask) -> Result<AgentOutput>;
}

/// Result of swarm orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmResult {
    /// All agent outputs.
    pub outputs: Vec<AgentOutput>,
    /// The consensus output (selected by strategy).
    pub consensus: Option<AgentOutput>,
    /// Total orchestration duration.
    pub duration: Duration,
}

/// Multi-agent orchestrator.
pub struct AgentSwarm {
    config: SwarmConfig,
}

impl AgentSwarm {
    /// Create a new agent swarm with the given configuration.
    pub fn new(config: SwarmConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Get the swarm configuration.
    pub fn config(&self) -> &SwarmConfig {
        &self.config
    }

    /// Orchestrate a task across multiple agents.
    pub async fn orchestrate(
        &self,
        task: &AgentTask,
        agents: &[Box<dyn Agent>],
    ) -> Result<SwarmResult> {
        if agents.is_empty() {
            return Err(FuseError::ValidationError(
                "At least one agent is required".to_string(),
            ));
        }

        if agents.len() > self.config.max_agents {
            return Err(FuseError::ResourceLimitExceeded(format!(
                "Too many agents: {} exceeds max of {}",
                agents.len(),
                self.config.max_agents
            )));
        }

        let start = Instant::now();
        let mut outputs = Vec::with_capacity(agents.len());

        // Execute agents sequentially for now (parallel via tokio::spawn in future)
        for agent in agents {
            match tokio::time::timeout(self.config.timeout, agent.execute(task)).await {
                Ok(Ok(output)) => outputs.push(output),
                Ok(Err(e)) => {
                    tracing::warn!(agent = agent.name(), error = %e, "Agent execution failed");
                }
                Err(_) => {
                    tracing::warn!(agent = agent.name(), "Agent execution timed out");
                }
            }

            // For FirstComplete, return as soon as we have one result
            if self.config.consensus_strategy == ConsensusStrategy::FirstComplete
                && !outputs.is_empty()
            {
                break;
            }
        }

        let consensus = Self::select_consensus(&self.config.consensus_strategy, &outputs);
        let duration = start.elapsed();

        Ok(SwarmResult {
            outputs,
            consensus,
            duration,
        })
    }

    /// Select the consensus output based on strategy.
    fn select_consensus(
        strategy: &ConsensusStrategy,
        outputs: &[AgentOutput],
    ) -> Option<AgentOutput> {
        if outputs.is_empty() {
            return None;
        }

        match strategy {
            ConsensusStrategy::FirstComplete => outputs.first().cloned(),
            ConsensusStrategy::BestScore => outputs
                .iter()
                .max_by(|a, b| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned(),
            ConsensusStrategy::Majority => {
                // Group by content, pick the most common
                let mut counts: std::collections::HashMap<&str, (usize, &AgentOutput)> =
                    std::collections::HashMap::new();
                for output in outputs {
                    let entry = counts.entry(output.content.as_str()).or_insert((0, output));
                    entry.0 += 1;
                }
                counts
                    .into_values()
                    .max_by_key(|(count, _)| *count)
                    .map(|(_, output)| output.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAgent {
        agent_name: String,
        caps: Vec<String>,
        output_content: String,
        output_score: f64,
    }

    impl TestAgent {
        fn new(name: &str, content: &str, score: f64) -> Self {
            Self {
                agent_name: name.to_string(),
                caps: vec!["test".to_string()],
                output_content: content.to_string(),
                output_score: score,
            }
        }
    }

    #[async_trait]
    impl Agent for TestAgent {
        fn name(&self) -> &str {
            &self.agent_name
        }
        fn capabilities(&self) -> Vec<String> {
            self.caps.clone()
        }
        async fn execute(&self, _task: &AgentTask) -> Result<AgentOutput> {
            Ok(AgentOutput {
                agent_name: self.agent_name.clone(),
                content: self.output_content.clone(),
                score: self.output_score,
                metadata: None,
            })
        }
    }

    fn sample_task() -> AgentTask {
        AgentTask {
            description: "Summarize this document".to_string(),
            context: None,
        }
    }

    #[test]
    fn test_swarm_config_default() {
        let config = SwarmConfig::default();
        assert_eq!(config.max_agents, 10);
        assert_eq!(config.consensus_strategy, ConsensusStrategy::BestScore);
    }

    #[test]
    fn test_swarm_config_validation() {
        let config = SwarmConfig::default();
        assert!(config.validate().is_ok());

        let bad = SwarmConfig {
            max_agents: 0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());

        let bad_timeout = SwarmConfig {
            timeout: Duration::ZERO,
            ..Default::default()
        };
        assert!(bad_timeout.validate().is_err());
    }

    #[test]
    fn test_consensus_strategy_default() {
        assert_eq!(ConsensusStrategy::default(), ConsensusStrategy::BestScore);
    }

    #[test]
    fn test_agent_output_creation() {
        let output = AgentOutput {
            agent_name: "test".to_string(),
            content: "result".to_string(),
            score: 0.95,
            metadata: None,
        };
        assert_eq!(output.agent_name, "test");
        assert!((output.score - 0.95).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_single_agent_orchestration() {
        let config = SwarmConfig::default();
        let swarm = AgentSwarm::new(config).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![Box::new(TestAgent::new("agent1", "result", 0.9))];

        let result = swarm.orchestrate(&sample_task(), &agents).await;
        assert!(result.is_ok());

        let result = result.expect("orchestration failed");
        assert_eq!(result.outputs.len(), 1);
        assert!(result.consensus.is_some());
        assert_eq!(
            result.consensus.as_ref().map(|c| &c.content),
            Some(&"result".to_string())
        );
    }

    #[tokio::test]
    async fn test_multi_agent_best_score() {
        let config = SwarmConfig {
            consensus_strategy: ConsensusStrategy::BestScore,
            ..Default::default()
        };
        let swarm = AgentSwarm::new(config).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(TestAgent::new("low", "low result", 0.3)),
            Box::new(TestAgent::new("high", "high result", 0.95)),
            Box::new(TestAgent::new("mid", "mid result", 0.6)),
        ];

        let result = swarm
            .orchestrate(&sample_task(), &agents)
            .await
            .expect("orchestration failed");

        assert_eq!(result.outputs.len(), 3);
        let consensus = result.consensus.expect("no consensus");
        assert_eq!(consensus.agent_name, "high");
        assert_eq!(consensus.content, "high result");
    }

    #[tokio::test]
    async fn test_multi_agent_majority() {
        let config = SwarmConfig {
            consensus_strategy: ConsensusStrategy::Majority,
            ..Default::default()
        };
        let swarm = AgentSwarm::new(config).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(TestAgent::new("a1", "answer A", 0.8)),
            Box::new(TestAgent::new("a2", "answer A", 0.7)),
            Box::new(TestAgent::new("a3", "answer B", 0.9)),
        ];

        let result = swarm
            .orchestrate(&sample_task(), &agents)
            .await
            .expect("orchestration failed");

        let consensus = result.consensus.expect("no consensus");
        assert_eq!(consensus.content, "answer A");
    }

    #[tokio::test]
    async fn test_first_complete_strategy() {
        let config = SwarmConfig {
            consensus_strategy: ConsensusStrategy::FirstComplete,
            ..Default::default()
        };
        let swarm = AgentSwarm::new(config).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(TestAgent::new("first", "first result", 0.5)),
            Box::new(TestAgent::new("second", "second result", 0.9)),
        ];

        let result = swarm
            .orchestrate(&sample_task(), &agents)
            .await
            .expect("orchestration failed");

        // With FirstComplete, should stop after first successful result
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].agent_name, "first");
    }

    #[tokio::test]
    async fn test_empty_agents_error() {
        let swarm = AgentSwarm::new(SwarmConfig::default()).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![];
        let result = swarm.orchestrate(&sample_task(), &agents).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_too_many_agents_error() {
        let config = SwarmConfig {
            max_agents: 1,
            ..Default::default()
        };
        let swarm = AgentSwarm::new(config).expect("swarm creation failed");
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(TestAgent::new("a1", "r1", 0.5)),
            Box::new(TestAgent::new("a2", "r2", 0.5)),
        ];
        let result = swarm.orchestrate(&sample_task(), &agents).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_trait_implementation() {
        let agent = TestAgent::new("test-agent", "output", 0.8);
        assert_eq!(agent.name(), "test-agent");
        assert_eq!(agent.capabilities(), vec!["test".to_string()]);
    }

    #[test]
    fn test_swarm_result_duration() {
        let result = SwarmResult {
            outputs: vec![],
            consensus: None,
            duration: Duration::from_millis(100),
        };
        assert_eq!(result.duration.as_millis(), 100);
    }

    #[test]
    fn test_consensus_empty_outputs() {
        let result = AgentSwarm::select_consensus(&ConsensusStrategy::BestScore, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_agent_task_serialization() {
        let task = sample_task();
        let json = serde_json::to_string(&task).expect("serialize failed");
        let deserialized: AgentTask = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.description, task.description);
    }
}
