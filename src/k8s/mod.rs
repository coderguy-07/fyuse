//! Kubernetes operator for Fuse model deployments.

#[cfg(feature = "kubernetes")]
pub mod operator;

// Stub types available without the kubernetes feature so downstream code compiles.
#[cfg(not(feature = "kubernetes"))]
pub mod operator {
    use serde::{Deserialize, Serialize};

    /// CRD spec for deploying a model (stub).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FuseModelSpec {
        pub model_name: String,
        pub replicas: u32,
        pub resources: ModelResources,
        pub quantization: Option<String>,
    }

    /// Resource requirements for a model deployment (stub).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ModelResources {
        pub cpu: String,
        pub memory: String,
        pub gpu: Option<String>,
    }

    /// CRD status for a model deployment (stub).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FuseModelStatus {
        pub phase: ModelPhase,
        pub ready_replicas: u32,
        pub message: Option<String>,
    }

    /// Phase of a model deployment.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ModelPhase {
        Pending,
        Running,
        Failed,
    }

    /// Action returned by reconciliation.
    #[derive(Debug, Clone)]
    pub struct ReconcileAction {
        pub requeue_after: Option<std::time::Duration>,
    }

    /// Placeholder operator (stub when kubernetes feature is disabled).
    pub struct ModelOperator;

    impl ModelOperator {
        pub fn new() -> Self {
            Self
        }

        pub fn reconcile(&self, _spec: &FuseModelSpec) -> crate::error::Result<ReconcileAction> {
            Err(crate::error::FuseError::FeatureDisabled(
                "kubernetes".to_string(),
            ))
        }
    }

    impl Default for ModelOperator {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub use operator::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_serialization() {
        let spec = FuseModelSpec {
            model_name: "llama-7b".to_string(),
            replicas: 3,
            resources: ModelResources {
                cpu: "4".to_string(),
                memory: "16Gi".to_string(),
                gpu: Some("1".to_string()),
            },
            quantization: Some("q4_k_m".to_string()),
        };

        let json = serde_json::to_string(&spec).expect("serialize should succeed");
        let deser: FuseModelSpec = serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(deser.model_name, "llama-7b");
        assert_eq!(deser.replicas, 3);
        assert_eq!(deser.resources.gpu, Some("1".to_string()));
    }

    #[test]
    fn test_status_phase_transitions() {
        let pending = FuseModelStatus {
            phase: ModelPhase::Pending,
            ready_replicas: 0,
            message: Some("Pulling model image".to_string()),
        };
        assert_eq!(pending.phase, ModelPhase::Pending);
        assert_eq!(pending.ready_replicas, 0);

        let running = FuseModelStatus {
            phase: ModelPhase::Running,
            ready_replicas: 3,
            message: None,
        };
        assert_eq!(running.phase, ModelPhase::Running);
        assert_eq!(running.ready_replicas, 3);

        let failed = FuseModelStatus {
            phase: ModelPhase::Failed,
            ready_replicas: 0,
            message: Some("OOM killed".to_string()),
        };
        assert_eq!(failed.phase, ModelPhase::Failed);
    }

    #[test]
    fn test_reconcile_placeholder() {
        let operator = ModelOperator::new();
        let spec = FuseModelSpec {
            model_name: "test".to_string(),
            replicas: 1,
            resources: ModelResources {
                cpu: "1".to_string(),
                memory: "4Gi".to_string(),
                gpu: None,
            },
            quantization: None,
        };

        // Without kubernetes feature, reconcile returns FeatureDisabled
        #[cfg(not(feature = "kubernetes"))]
        {
            let result = operator.reconcile(&spec);
            assert!(result.is_err());
        }

        // With kubernetes feature, reconcile succeeds with requeue
        #[cfg(feature = "kubernetes")]
        {
            let result = operator.reconcile(&spec);
            assert!(result.is_ok());
            let action = result.expect("reconcile should succeed");
            assert!(action.requeue_after.is_some());
        }
    }
}
