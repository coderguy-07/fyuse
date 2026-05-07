# Fuse Test Case Specifications

## Version: 1.0.0
## Status: Draft

---

## Table of Contents

1. [Test Strategy Overview](#1-test-strategy-overview)
2. [Unit Tests](#2-unit-tests)
3. [Integration Tests](#3-integration-tests)
4. [End-to-End Tests](#4-end-to-end-tests)
5. [Performance Tests](#5-performance-tests)
6. [Security Tests](#6-security-tests)
7. [Chaos Tests](#7-chaos-tests)

---

## 1. Test Strategy Overview

### 1.1 Testing Pyramid

```
                    ┌─────────────┐
                    │   E2E Tests │  10% - Full user journeys
                    │   (Slow)    │
                   ┌┴─────────────┴┐
                   │ Integration    │  20% - Component interactions
                   │ Tests          │
                  ┌┴────────────────┴┐
                  │   Unit Tests      │  70% - Individual functions
                  │   (Fast)          │
                  └───────────────────┘
```

### 1.2 Coverage Targets

| Category | Target | Minimum |
|----------|--------|---------|
| Unit Tests | 95% | 90% |
| Integration Tests | 85% | 80% |
| E2E Tests | 80% | 70% |
| Overall | 90% | 85% |

### 1.3 Test Categories

```rust
// Test markers for organization
#[cfg(test)]
mod unit_tests {
    // Fast, isolated tests
}

#[cfg(test)]
mod integration_tests {
    // Tests with external dependencies
}

#[cfg(test)]
mod property_tests {
    // Property-based tests with proptest
}
```

---

## 2. Unit Tests

### 2.1 Configuration Module Tests

```rust
#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_from_toml() {
        let toml = r#"
            [general]
            models_dir = "/tmp/models"
            log_level = "debug"
        "#;
        
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.general.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_invalid_log_level_fails() {
        let toml = r#"
            [general]
            log_level = "invalid"
        "#;
        
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_merge() {
        let base = Config::default();
        let override_config = Config {
            general: GeneralConfig {
                log_level: LogLevel::Trace,
                ..base.general.clone()
            },
            ..base.clone()
        };
        
        let merged = base.merge(override_config);
        assert_eq!(merged.general.log_level, LogLevel::Trace);
    }
}
```

### 2.2 Model Manager Tests

```rust
#[cfg(test)]
mod model_manager_tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_load_model_success() {
        let mut storage = MockStorage::new();
        storage
            .expect_get_model()
            .with(eq("test-model"))
            .returning(|_| Ok(Some(create_test_model())));
        
        let manager = ModelManager::new(Arc::new(storage));
        let result = manager.load("test-model").await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_model_not_found() {
        let mut storage = MockStorage::new();
        storage
            .expect_get_model()
            .returning(|_| Ok(None));
        
        let manager = ModelManager::new(Arc::new(storage));
        let result = manager.load("nonexistent").await;
        
        assert!(matches!(result, Err(ModelError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_concurrent_model_loading() {
        let manager = Arc::new(ModelManager::new(create_test_storage()));
        let mut handles = vec![];
        
        // Spawn 10 concurrent load attempts
        for _ in 0..10 {
            let manager = manager.clone();
            handles.push(tokio::spawn(async move {
                manager.load("test-model").await
            }));
        }
        
        // All should succeed
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // But model should only be loaded once
        assert_eq!(manager.loaded_model_count(), 1);
    }

    #[tokio::test]
    async fn test_model_state_transitions() {
        let model = create_test_model();
        
        // Valid transitions
        assert!(model.can_transition_to(&ModelState::Loading));
        assert!(model.transition_to(ModelState::Loading).await.is_ok());
        
        assert!(model.can_transition_to(&ModelState::Loaded));
        assert!(model.transition_to(ModelState::Loaded).await.is_ok());
        
        // Invalid transition
        assert!(!model.can_transition_to(&ModelState::Loading));
        assert!(model.transition_to(ModelState::Loading).await.is_err());
    }
}
```

### 2.3 Error Handling Tests

```rust
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_error_code_generation() {
        let error = FuseError::Model(ModelError::NotFound("test".to_string()));
        assert_eq!(error.code(), "MODEL_NOT_FOUND");
    }

    #[test]
    fn test_error_retryable_detection() {
        let transient = FuseError::Network(NetworkError::Timeout);
        assert!(transient.is_retryable());
        
        let permanent = FuseError::Config(ConfigError::InvalidFormat);
        assert!(!permanent.is_retryable());
    }

    #[test]
    fn test_error_remediation_suggestions() {
        let error = FuseError::Model(ModelError::InsufficientMemory);
        let remediation = error.remediation();
        
        assert!(remediation.is_some());
        assert!(remediation.unwrap().contains("GPU memory"));
    }
}
```

### 2.4 Resource Management Tests

```rust
#[cfg(test)]
mod resource_tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_limit_enforcement() {
        let limiter = MemoryLimiter::new(1024 * 1024 * 100); // 100MB
        
        // Should succeed within limit
        let result = limiter
            .execute(1024 * 1024 * 50, || async { "success" })
            .await;
        assert!(result.is_ok());
        
        // Should fail exceeding limit
        let result = limiter
            .execute(1024 * 1024 * 150, || async { "success" })
            .await;
        assert!(matches!(result, Err(Error::InsufficientMemory)));
    }

    #[test]
    fn test_resource_guard_cleanup() {
        let counter = Arc::new(AtomicUsize::new(0));
        
        {
            let counter_clone = counter.clone();
            let _guard = ResourceGuard::new(
                "test",
                move || {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            );
            // Guard in scope
            assert_eq!(counter.load(Ordering::SeqCst), 0);
        }
        
        // Guard dropped, cleanup called
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
```

---

## 3. Integration Tests

### 3.1 API Integration Tests

```rust
#[cfg(test)]
mod api_integration_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn create_test_app() -> Router {
        let config = create_test_config();
        let state = AppState::new(config).await;
        create_router(state)
    }

    #[tokio::test]
    async fn test_list_models_endpoint() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(Request::builder()
                .uri("/api/v1/models")
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let models: ModelList = serde_json::from_slice(&body).unwrap();
        assert!(!models.models.is_empty());
    }

    #[tokio::test]
    async fn test_generate_endpoint() {
        let app = create_test_app().await;
        
        let request = GenerateRequest {
            model: "test-model".to_string(),
            prompt: "Hello".to_string(),
            stream: false,
            options: None,
        };
        
        let response = app
            .oneshot(Request::builder()
                .method("POST")
                .uri("/api/generate")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request).unwrap()))
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let app = create_test_app().await;
        
        // Send 70 requests (limit is 60/min)
        for i in 0..70 {
            let response = app
                .clone()
                .oneshot(Request::builder()
                    .uri("/api/generate")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"model":"test","prompt":"hi"}"#))
                    .unwrap())
                .await
                .unwrap();
            
            if i < 60 {
                assert!(
                    response.status() != StatusCode::TOO_MANY_REQUESTS,
                    "Request {} should succeed", i
                );
            } else {
                assert_eq!(
                    response.status(),
                    StatusCode::TOO_MANY_REQUESTS,
                    "Request {} should be rate limited", i
                );
            }
        }
    }
}
```

### 3.2 Storage Integration Tests

```rust
#[cfg(test)]
mod storage_integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_model_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path()).await.unwrap();
        
        let model = create_test_model();
        
        // Store model metadata
        storage.save_model(&model).await.unwrap();
        
        // Retrieve model metadata
        let retrieved = storage.get_model(&model.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, model.id);
        assert_eq!(retrieved.name, model.name);
    }

    #[tokio::test]
    async fn test_concurrent_storage_access() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(Storage::new(temp_dir.path()).await.unwrap());
        
        let mut handles = vec![];
        
        for i in 0..100 {
            let storage = storage.clone();
            handles.push(tokio::spawn(async move {
                let model = create_test_model_with_id(format!("model-{}", i));
                storage.save_model(&model).await.unwrap();
            }));
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify all models saved
        let models = storage.list_models().await.unwrap();
        assert_eq!(models.len(), 100);
    }
}
```

### 3.3 Workflow Integration Tests

```rust
#[cfg(test)]
mod workflow_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_execution() {
        let executor = WorkflowExecutor::new();
        
        let workflow = Workflow {
            name: "test-workflow".to_string(),
            steps: vec![
                Step {
                    id: "step1".to_string(),
                    action: Action::Echo { message: "Hello".to_string() },
                    depends_on: vec![],
                },
                Step {
                    id: "step2".to_string(),
                    action: Action::Echo { message: "World".to_string() },
                    depends_on: vec!["step1".to_string()],
                },
            ],
        };
        
        let result = executor.execute(workflow).await.unwrap();
        
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert!(result.step_results.contains_key("step1"));
        assert!(result.step_results.contains_key("step2"));
    }

    #[tokio::test]
    async fn test_parallel_step_execution() {
        let executor = WorkflowExecutor::new();
        let start = Instant::now();
        
        let workflow = Workflow {
            name: "parallel-test".to_string(),
            steps: vec![
                Step {
                    id: "parallel1".to_string(),
                    action: Action::Sleep { duration: Duration::from_millis(100) },
                    depends_on: vec![],
                },
                Step {
                    id: "parallel2".to_string(),
                    action: Action::Sleep { duration: Duration::from_millis(100) },
                    depends_on: vec![],
                },
            ],
        };
        
        executor.execute(workflow).await.unwrap();
        let elapsed = start.elapsed();
        
        // Should complete in ~100ms, not ~200ms
        assert!(elapsed < Duration::from_millis(150));
    }
}
```

---

## 4. End-to-End Tests

### 4.1 CLI E2E Tests

```rust
#[cfg(test)]
mod cli_e2e_tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_init_command() {
        let temp_dir = TempDir::new().unwrap();
        
        let mut cmd = Command::cargo_bin("fuse").unwrap();
        cmd.arg("init")
            .arg("--config-dir")
            .arg(temp_dir.path())
            .assert()
            .success();
        
        assert!(temp_dir.path().join("config.toml").exists());
    }

    #[test]
    fn test_model_pull_list_remove() {
        let temp_dir = TempDir::new().unwrap();
        
        // Init first
        Command::cargo_bin("fuse")
            .unwrap()
            .arg("init")
            .arg("--config-dir")
            .arg(temp_dir.path())
            .assert()
            .success();
        
        // Pull model
        Command::cargo_bin("fuse")
            .unwrap()
            .arg("--config-dir")
            .arg(temp_dir.path())
            .arg("pull")
            .arg("gpt2")
            .timeout(Duration::from_secs(60))
            .assert()
            .success();
        
        // List models
        Command::cargo_bin("fuse")
            .unwrap()
            .arg("--config-dir")
            .arg(temp_dir.path())
            .arg("list")
            .assert()
            .success()
            .stdout(predicate::str::contains("gpt2"));
        
        // Remove model
        Command::cargo_bin("fuse")
            .unwrap()
            .arg("--config-dir")
            .arg(temp_dir.path())
            .arg("rm")
            .arg("gpt2")
            .assert()
            .success();
    }
}
```

### 4.2 API E2E Tests

```rust
#[cfg(test)]
mod api_e2e_tests {
    use reqwest::Client;
    use std::time::Duration;

    async fn wait_for_server(url: &str) {
        let client = Client::new();
        for _ in 0..30 {
            if client.get(url).send().await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        panic!("Server failed to start");
    }

    #[tokio::test]
    async fn test_full_inference_flow() {
        // Start server (in practice, use testcontainers or similar)
        let server = spawn_test_server().await;
        let base_url = server.base_url();
        
        wait_for_server(&format!("{}/health", base_url)).await;
        
        let client = Client::new();
        
        // 1. Pull model
        let response = client
            .post(format!("{}/api/pull", base_url))
            .json(&json!({
                "name": "test-model",
                "stream": false
            }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        
        // 2. List models
        let response = client
            .get(format!("{}/api/tags", base_url))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        
        // 3. Generate
        let response = client
            .post(format!("{}/api/generate", base_url))
            .json(&json!({
                "model": "test-model",
                "prompt": "Hello",
                "stream": false
            }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        
        let result: GenerateResponse = response.json().await.unwrap();
        assert!(!result.response.is_empty());
        
        // 4. Delete model
        let response = client
            .delete(format!("{}/api/delete", base_url))
            .json(&json!({ "name": "test-model" }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }
}
```

---

## 5. Performance Tests

### 5.1 Inference Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn inference_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("inference");
    
    for input_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(input_size),
            input_size,
            |b, &size| {
                let model = create_test_model();
                let input = create_input_of_size(size);
                
                b.to_async(&runtime).iter(|| async {
                    model.infer(black_box(&input)).await
                });
            },
        );
    }
    
    group.finish();
}

fn throughput_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("concurrent_inference_100", |b| {
        b.to_async(&runtime).iter(|| async {
            let handles: Vec<_> = (0..100)
                .map(|i| {
                    let model = get_model();
                    tokio::spawn(async move {
                        model.infer(&format!("request {}", i)).await
                    })
                })
                .collect();
            
            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

criterion_group!(benches, inference_benchmark, throughput_benchmark);
criterion_main!(benches);
```

### 5.2 Load Testing

```rust
#[cfg(test)]
mod load_tests {
    use goose::prelude::*;

    async fn inference_loadtest(user: &mut GooseUser) -> TransactionResult {
        let request = GenerateRequest {
            model: "test-model".to_string(),
            prompt: "Hello, world!".to_string(),
            stream: false,
            options: None,
        };
        
        let _goose = user
            .post_json("/api/generate", &request)
            .await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_load_100_rps() {
        let goose = GooseAttack::initialize()
            .unwrap()
            .register_scenario(
                scenario!("InferenceLoadTest")
                    .register_transaction(transaction!(inference_loadtest))
            )
            .set_default(GooseDefault::Host, "http://localhost:8080")
            .set_default(GooseDefault::Users, 100)
            .set_default(GooseDefault::StartupTime, 30)
            .set_default(GooseDefault::RunTime, 300)
            .execute()
            .await
            .unwrap();
        
        let metrics = goose.metrics;
        
        // Assert on metrics
        assert!(metrics.requests_per_second >= 90.0);
        assert!(metrics.fail_rate <= 0.01); // < 1% errors
    }
}
```

---

## 6. Security Tests

### 6.1 Authentication Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[tokio::test]
    async fn test_unauthorized_request_fails() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(Request::builder()
                .uri("/api/v1/admin/models")
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_token_fails() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(Request::builder()
                .uri("/api/v1/models")
                .header("Authorization", "Bearer invalid_token")
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_insufficient_permissions() {
        let app = create_test_app_with_roles().await;
        let user_token = create_token_with_role("user");
        
        let response = app
            .oneshot(Request::builder()
                .uri("/api/v1/admin/users")
                .header("Authorization", format!("Bearer {}", user_token))
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
```

### 6.2 Injection Tests

```rust
#[cfg(test)]
mod injection_tests {
    use super::*;

    #[test]
    fn test_sql_injection_prevention() {
        let malicious_input = "'; DROP TABLE models; --";
        let sanitized = InputSanitizer::sanitize(malicious_input);
        
        assert!(!sanitized.contains(';'));
        assert!(!sanitized.contains("DROP"));
    }

    #[test]
    fn test_path_traversal_prevention() {
        let malicious_path = "../../../etc/passwd";
        let result = SafePath::new("/safe/base", malicious_path);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_command_injection_prevention() {
        let malicious_cmd = "model; rm -rf /";
        let validator = CommandValidator::new();
        
        assert!(!validator.is_valid(malicious_cmd));
    }
}
```

---

## 7. Chaos Tests

### 7.1 Failure Injection

```rust
#[cfg(test)]
mod chaos_tests {
    use super::*;

    #[tokio::test]
    async fn test_graceful_degradation() {
        let chaos = ChaosMonkey::new()
            .with_failure_rate(0.1)
            .with_latency(Duration::from_millis(0)..Duration::from_millis(100));
        
        let system = SystemUnderTest::new();
        
        for i in 0..1000 {
            let result = chaos.intercept(
                system.process_request(Request::new(i))
            ).await;
            
            // System should never crash, may return errors
            if let Err(e) = result {
                assert!(e.is_recoverable());
            }
        }
        
        // System should still be functional
        assert!(system.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_partial_failure_handling() {
        let mut system = SystemUnderTest::new();
        
        // Simulate partial backend failure
        system.simulate_failure("storage", FailureMode::Intermittent);
        
        // System should use fallback
        let result = system.read_model("test").await;
        assert!(result.is_ok()); // Succeeds via cache fallback
        
        // Simulate complete backend failure
        system.simulate_failure("storage", FailureMode::Complete);
        
        // System should fail gracefully
        let result = system.read_model("uncached").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::StorageUnavailable));
    }
}
```

### 7.2 Resource Exhaustion Tests

```rust
#[cfg(test)]
mod resource_exhaustion_tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_exhaustion_handling() {
        let system = SystemUnderTest::new()
            .with_memory_limit(1024 * 1024 * 100); // 100MB
        
        // Try to allocate more than limit
        let result = system.allocate_memory(1024 * 1024 * 150).await;
        assert!(result.is_err());
        
        // System should still be responsive
        assert!(system.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_connection_pool_exhaustion() {
        let system = SystemUnderTest::new()
            .with_connection_pool_size(10);
        
        // Exhaust connection pool
        let handles: Vec<_> = (0..20)
            .map(|_| {
                let system = system.clone();
                tokio::spawn(async move {
                    system.use_connection().await
                })
            })
            .collect();
        
        // Some should succeed, some should fail with pool exhausted
        let results: Vec<_> = futures::future::join_all(handles).await;
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let failures = results.iter().filter(|r| r.is_err()).count();
        
        assert_eq!(successes, 10);
        assert_eq!(failures, 10);
    }
}
```

---

## Appendix: Test Utilities

### Mock Implementations

```rust
#[cfg(test)]
pub mod mocks {
    use mockall::mock;
    
    mock! {
        pub Storage {}
        
        #[async_trait]
        impl Storage for Storage {
            async fn get_model(&self, id: &str) -> Result<Option<Model>, Error>;
            async fn save_model(&self, model: &Model) -> Result<(), Error>;
            async fn delete_model(&self, id: &str) -> Result<(), Error>;
            async fn list_models(&self) -> Result<Vec<Model>, Error>;
        }
    }
    
    mock! {
        pub Model {}
        
        impl Model for Model {
            fn id(&self) -> &str;
            fn name(&self) -> &str;
        }
        
        #[async_trait]
        impl Inference for Model {
            async fn infer(&self, input: &str) -> Result<String, Error>;
        }
    }
}
```

### Test Fixtures

```rust
#[cfg(test)]
pub fn create_test_model() -> Model {
    Model {
        id: "test-model".to_string(),
        name: "Test Model".to_string(),
        source: ModelSource::HuggingFace,
        format: ModelFormat::GGUF,
        size_bytes: 1024 * 1024 * 1024,
        quantization: Some(Quantization::GGUF(GGUFQuantization::Q4_0)),
        status: ModelStatus::Ready,
    }
}

#[cfg(test)]
pub fn create_test_config() -> Config {
    Config {
        general: GeneralConfig {
            models_dir: PathBuf::from("/tmp/test-models"),
            cache_dir: PathBuf::from("/tmp/test-cache"),
            log_level: LogLevel::Debug,
        },
        ..Default::default()
    }
}
```

---

*End of Test Case Specifications*
