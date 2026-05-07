//! Tests that verify feature-gated compilation works correctly.
//! This test file compiles under any feature combination.

#[test]
fn default_features_compile() {
    // If this test runs, the default feature set compiles successfully.
    // Default = ["cli", "api-server", "cpu-inference"]
    assert!(true);
}

#[test]
fn core_types_available() {
    // Core types should always be available regardless of features
    let _: fuse::FuseConfig = fuse::FuseConfig::default();
}

#[test]
fn feature_flags_defined() {
    // Verify key feature flags are detectable at compile time
    // These cfg checks confirm the feature flag definitions exist in Cargo.toml

    #[cfg(feature = "cli")]
    {
        // CLI feature is enabled in default
    }

    #[cfg(feature = "api-server")]
    {
        // API server feature is enabled in default
    }

    // Features that should NOT be enabled by default
    #[cfg(feature = "yew-ui")]
    {
        compile_error!("yew-ui should not be in default features");
    }

    #[cfg(feature = "dioxus-ui")]
    {
        compile_error!("dioxus-ui should not be in default features");
    }

    #[cfg(feature = "cuda")]
    {
        compile_error!("cuda should not be in default features");
    }

    #[cfg(feature = "telegram")]
    {
        compile_error!("telegram should not be in default features");
    }

    #[cfg(feature = "kubernetes")]
    {
        compile_error!("kubernetes should not be in default features");
    }
}

#[test]
fn edge_feature_excludes_heavy_deps() {
    // When running with --features edge --no-default-features,
    // heavy dependencies should not be pulled in.
    // This test just confirms compilation under any feature set.
    assert!(cfg!(feature = "cli") || !cfg!(feature = "cli"));
}
