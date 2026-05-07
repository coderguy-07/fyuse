//! Edge build verification tests [8.1]

#[cfg(feature = "edge")]
mod edge_tests {
    use fuse::error::{FuseError, Result};
    use fuse::inference::backend::{
        BackendInfo, BackendType, InferenceRequest, InferenceResponse, ModelConfig, ResourceUsage,
        StopReason, Token,
    };

    #[test]
    fn test_edge_feature_compiles() {
        // Verify core inference types are available under edge feature
        let _req = InferenceRequest::default();
        let _cfg = ModelConfig::default();
        let _usage = ResourceUsage::default();
    }

    #[test]
    fn test_edge_backend_types_available() {
        let info = BackendInfo {
            name: "edge-cpu".to_string(),
            backend_type: BackendType::CpuSimd,
            supports_streaming: true,
            supports_embeddings: false,
            max_batch_size: 1,
        };
        assert_eq!(info.backend_type, BackendType::CpuSimd);
    }

    #[test]
    fn test_edge_inference_request_defaults() {
        let req = InferenceRequest::default();
        assert_eq!(req.max_tokens, 256);
        assert!((req.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_edge_error_types() {
        let err = FuseError::FeatureDisabled("wasm-runtime".to_string());
        assert_eq!(err.error_code(), "FEATURE_DISABLED");
        assert_eq!(err.http_status_code(), 501);
    }
}
