# Fuse TDD Strategy & Quality Gates

## Version: 2.0.0 | Date: 2026-04-04

---

## 1. Test Pyramid

```
                    ┌───────┐
                    │  E2E  │  5%  — Full user flows
                   ┌┴───────┴┐
                   │  Integ  │  20% — Module interactions
                  ┌┴─────────┴┐
                  │   Unit    │  55% — Functions/methods
                 ┌┴───────────┴┐
                 │  Property   │  15% — Invariant verification
                ┌┴─────────────┴┐
                │  Benchmarks   │  5%  — Performance gates
                └───────────────┘
```

---

## 2. Test Categories

### 2.1 Unit Tests (in-file, `#[cfg(test)]`)

**Where**: Bottom of every `.rs` file
**What**: Test individual functions/methods in isolation
**Mocking**: `mockall` for trait-based dependencies

```rust
// src/inference/coordinator.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    mock! {
        Backend {}
        #[async_trait]
        impl InferenceBackend for Backend {
            fn info(&self) -> BackendInfo;
            async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle>;
            async fn unload_model(&self, handle: &ModelHandle) -> Result<()>;
            async fn infer(&self, handle: &ModelHandle, req: InferenceRequest) -> Result<InferenceResponse>;
            fn stream(&self, handle: &ModelHandle, req: InferenceRequest)
                -> Pin<Box<dyn Stream<Item = Result<Token>> + Send>>;
            async fn embed(&self, handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>>;
            fn resource_usage(&self) -> ResourceUsage;
        }
    }

    #[test]
    fn coordinator_routes_to_cpu_when_no_gpu() {
        let mut cpu = MockBackend::new();
        cpu.expect_info().returning(|| BackendInfo {
            backend_type: BackendType::CpuSimd,
            ..Default::default()
        });
        
        let coordinator = InferenceCoordinator::new(vec![Box::new(cpu)]);
        let selected = coordinator.select_backend("any-model");
        assert_eq!(selected.info().backend_type, BackendType::CpuSimd);
    }

    #[tokio::test]
    async fn coordinator_batches_concurrent_requests() {
        let mut backend = MockBackend::new();
        backend.expect_infer()
            .times(1)  // Should batch into ONE call
            .returning(|_, _| Ok(InferenceResponse::default()));
        
        let coordinator = InferenceCoordinator::new(vec![Box::new(backend)]);
        let (r1, r2) = tokio::join!(
            coordinator.infer(req1),
            coordinator.infer(req2),
        );
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    #[test]
    fn coordinator_evicts_lru_model_when_over_budget() {
        // Arrange: load 3 models (budget = 2)
        // Act: load 4th model
        // Assert: oldest model evicted
    }
}
```

### 2.2 Integration Tests (`tests/`)

**Where**: `tests/` directory
**What**: Multiple modules working together, real I/O

```rust
// tests/api/ollama_compat.rs
use fuse::test_helpers::spawn_test_server;

#[tokio::test]
async fn test_ollama_api_tags() {
    let server = spawn_test_server().await;
    let resp = server.get("/api/tags").send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["models"].is_array());
}

#[tokio::test]
async fn test_ollama_api_generate_streaming() {
    let server = spawn_test_server_with_model("tiny-test").await;
    let resp = server.post("/api/generate")
        .json(&json!({
            "model": "tiny-test",
            "prompt": "Hello",
            "stream": true
        }))
        .send().await.unwrap();
    
    assert_eq!(resp.status(), 200);
    
    // Read NDJSON stream
    let mut lines = 0;
    let mut done = false;
    let body = resp.text().await.unwrap();
    for line in body.lines() {
        let obj: serde_json::Value = serde_json::from_str(line).unwrap();
        if obj["done"].as_bool() == Some(true) {
            done = true;
        }
        lines += 1;
    }
    assert!(lines > 1, "Should stream multiple chunks");
    assert!(done, "Should end with done=true");
}

#[tokio::test]
async fn test_openai_chat_completions() {
    let server = spawn_test_server_with_model("tiny-test").await;
    let resp = server.post("/v1/chat/completions")
        .json(&json!({
            "model": "tiny-test",
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": false
        }))
        .send().await.unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["choices"][0]["message"]["content"].is_string());
    assert!(body["usage"]["total_tokens"].as_u64().unwrap() > 0);
}

// tests/channels/telegram_mock.rs
#[tokio::test]
async fn test_telegram_channel_receives_and_responds() {
    let mock_telegram = MockTelegramServer::start().await;
    let channel = TelegramChannel::new(&ChannelConfig {
        token: "test-token".into(),
        model: "tiny-test".into(),
        ..Default::default()
    });
    
    // Simulate incoming message
    mock_telegram.send_update(json!({
        "message": {
            "chat": {"id": 123},
            "text": "Hello"
        }
    })).await;
    
    // Verify response was sent
    let response = mock_telegram.last_sent_message().await;
    assert!(response.text.len() > 0);
    assert_eq!(response.chat_id, 123);
}
```

### 2.3 Property Tests (`proptest`)

```rust
// src/quantization/methods/gguf.rs
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn quantize_dequantize_q4_0_bounded_error(
            weights in prop::collection::vec(-2.0f32..2.0, 32..1024)
        ) {
            let quantized = quantize_q4_0(&weights);
            let dequantized = dequantize_q4_0(&quantized);
            
            for (orig, deq) in weights.iter().zip(dequantized.iter()) {
                let error = (orig - deq).abs();
                prop_assert!(error < 0.5, "Q4_0 error exceeded: {} vs {}", orig, deq);
            }
        }

        #[test]
        fn quantized_size_is_correct(
            weights in prop::collection::vec(-1.0f32..1.0, 32..10000)
        ) {
            let quantized = quantize_q4_0(&weights);
            let expected_size = weights.len() / 2 + /* block overhead */ (weights.len() / 32) * 2;
            prop_assert_eq!(quantized.len(), expected_size);
        }

        #[test]
        fn gguf_header_roundtrip(
            name in "[a-z]{3,20}",
            params in 1u64..100_000_000_000,
        ) {
            let header = GgufHeader { name, parameter_count: params, ..Default::default() };
            let bytes = header.serialize();
            let parsed = GgufHeader::parse(&bytes).unwrap();
            prop_assert_eq!(header.name, parsed.name);
            prop_assert_eq!(header.parameter_count, parsed.parameter_count);
        }
    }

    proptest! {
        #[test]
        fn session_key_serialization_roundtrip(
            channel in "[a-z]{3,10}",
            user_id in "[0-9]{5,20}",
        ) {
            let key = SessionKey { channel, user_id };
            let serialized = serde_json::to_string(&key).unwrap();
            let deserialized: SessionKey = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(key, deserialized);
        }
    }
}
```

### 2.4 Benchmarks (`criterion`)

```rust
// benches/inference_throughput.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_cpu_inference(c: &mut Criterion) {
    let engine = setup_cpu_engine("tiny-model");
    let prompt = tokenize("The quick brown fox");
    
    let mut group = c.benchmark_group("cpu_inference");
    
    for num_tokens in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("generate", num_tokens),
            &num_tokens,
            |b, &n| {
                b.iter(|| engine.generate(&prompt, n))
            },
        );
    }
    group.finish();
}

fn bench_quantization(c: &mut Criterion) {
    let weights = generate_random_weights(1_000_000);
    
    let mut group = c.benchmark_group("quantization");
    
    group.bench_function("q4_0_quantize", |b| {
        b.iter(|| quantize_q4_0(black_box(&weights)))
    });
    
    group.bench_function("q4_0_dequantize", |b| {
        let q = quantize_q4_0(&weights);
        b.iter(|| dequantize_q4_0(black_box(&q)))
    });
    
    group.bench_function("turboquant_4bit", |b| {
        b.iter(|| turboquant_quantize(black_box(&weights), 4))
    });
    
    group.finish();
}

fn bench_api_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(spawn_bench_server());
    
    c.bench_function("api_chat_completion_p99", |b| {
        b.iter(|| {
            rt.block_on(async {
                server.post("/v1/chat/completions")
                    .json(&json!({"model": "tiny", "messages": [{"role": "user", "content": "Hi"}]}))
                    .send().await.unwrap()
            })
        })
    });
}

criterion_group!(benches, bench_cpu_inference, bench_quantization, bench_api_throughput);
criterion_main!(benches);
```

### 2.5 End-to-End Tests

```rust
// tests/e2e/full_flow.rs
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_full_flow_pull_quantize_serve_query() {
    // 1. Pull a tiny test model
    let output = Command::new("cargo")
        .args(["run", "--", "pull", "tiny-test-model"])
        .output().await.unwrap();
    assert!(output.status.success());
    
    // 2. Quantize it
    let output = Command::new("cargo")
        .args(["run", "--", "quantize", "tiny-test-model", "--method", "q4_0"])
        .output().await.unwrap();
    assert!(output.status.success());
    
    // 3. Start server
    let mut server = Command::new("cargo")
        .args(["run", "--", "serve", "--port", "19876"])
        .spawn().unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // 4. Query via API
    let client = reqwest::Client::new();
    let resp = client.post("http://localhost:19876/api/generate")
        .json(&json!({"model": "tiny-test-model", "prompt": "Hello"}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
    
    // 5. Cleanup
    server.kill().await.unwrap();
}
```

---

## 3. Quality Gates

### 3.1 Pre-Commit (local, fast — <30s)

```bash
#!/bin/bash
# .git/hooks/pre-commit
cargo fmt --check || exit 1
cargo clippy -- -D warnings || exit 1
cargo test --lib -- --quiet || exit 1
```

### 3.2 PR Gate (CI, thorough — <10 min)

| Gate | Command | Threshold |
|------|---------|-----------|
| Format | `cargo fmt --check` | Zero diff |
| Lint | `cargo clippy -- -D warnings` | Zero warnings |
| Unit tests | `cargo test --lib` | 100% pass |
| Integration tests | `cargo test --test '*'` | 100% pass |
| Property tests | `cargo test proptest` | 100% pass |
| Coverage | `cargo tarpaulin` | >= 85% |
| Security | `cargo audit` | Zero high/critical |
| Build (Linux) | `cargo build --release` | Success |
| Build (Mac ARM) | `cargo build --release` | Success |
| Build (Windows) | `cargo build --release` | Success |
| Build (WASM) | `cargo build --target wasm32-unknown-unknown` | Success |
| Binary size | `ls -la target/release/fuse` | < 15MB |
| Unsafe audit | Custom check | All `unsafe` has `// SAFETY:` |
| Benchmarks | `cargo bench` | No >5% regression vs main |

### 3.3 Release Gate (before tag)

| Gate | Threshold |
|------|-----------|
| All PR gates pass | Required |
| E2E tests pass | `cargo test -- --ignored` |
| Cross-platform CI green | Mac ARM, Mac Intel, Linux x86, Windows, Linux ARM64 |
| Edge build succeeds | `cross build --target aarch64-unknown-linux-gnu` |
| Benchmark published | Results committed to `benches/results/` |
| CHANGELOG updated | Required |
| Version bumped | Required |

---

## 4. Test Infrastructure

### 4.1 Test Helpers Module

```rust
// src/test_helpers.rs (compiled only in test)
#[cfg(test)]
pub mod test_helpers {
    use crate::*;
    
    /// Spawn a test server with a tiny model
    pub async fn spawn_test_server() -> TestServer {
        let config = FuseConfig {
            server: ServerConfig { port: 0, ..Default::default() }, // Random port
            ..test_config()
        };
        let addr = start_server(config).await.unwrap();
        TestServer { addr, client: reqwest::Client::new() }
    }
    
    /// Spawn with a specific model loaded
    pub async fn spawn_test_server_with_model(model: &str) -> TestServer {
        let server = spawn_test_server().await;
        // Load tiny test model (embedded in binary for tests)
        server.load_test_model(model).await;
        server
    }
    
    pub struct TestServer {
        pub addr: SocketAddr,
        pub client: reqwest::Client,
    }
    
    impl TestServer {
        pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
            self.client.get(format!("http://{}{}", self.addr, path))
        }
        pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
            self.client.post(format!("http://{}{}", self.addr, path))
        }
    }
    
    /// Create a minimal config for testing
    pub fn test_config() -> FuseConfig {
        FuseConfig {
            inference: InferenceConfig {
                max_loaded_models: 1,
                continuous_batching: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    
    /// Create a tiny model for testing (generates deterministic output)
    pub fn create_test_model() -> TempDir {
        let dir = TempDir::new().unwrap();
        // Write a minimal GGUF with random weights
        write_tiny_gguf(dir.path().join("model.gguf"));
        dir
    }
}
```

### 4.2 Mock Channel Servers

```rust
// tests/mocks/telegram_mock.rs
pub struct MockTelegramServer {
    addr: SocketAddr,
    updates_tx: mpsc::Sender<serde_json::Value>,
    sent_messages: Arc<Mutex<Vec<TelegramMessage>>>,
}

impl MockTelegramServer {
    pub async fn start() -> Self {
        let (tx, rx) = mpsc::channel(100);
        let sent = Arc::new(Mutex::new(Vec::new()));
        
        let app = Router::new()
            .route("/bot:token/getUpdates", get(handle_get_updates))
            .route("/bot:token/sendMessage", post(handle_send_message));
        
        let addr = start_mock_server(app).await;
        Self { addr, updates_tx: tx, sent_messages: sent }
    }
    
    pub async fn send_update(&self, update: serde_json::Value) {
        self.updates_tx.send(update).await.unwrap();
    }
    
    pub async fn last_sent_message(&self) -> TelegramMessage {
        // Wait for message with timeout
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let msgs = self.sent_messages.lock();
                if !msgs.is_empty() {
                    return msgs.last().unwrap().clone();
                }
                drop(msgs);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.unwrap()
    }
}
```

---

## 5. Coverage Requirements

| Module | Minimum Coverage | Rationale |
|--------|-----------------|-----------|
| `inference/` | 90% | Core business logic, safety-critical |
| `quantization/` | 90% | Data integrity critical |
| `api/routes/` | 85% | API contract compliance |
| `channels/` | 80% | External integration (harder to test) |
| `devices/` | 75% | External hardware (mock-heavy) |
| `config/` | 90% | Parsing must be bulletproof |
| `security/` | 95% | Security code must be thorough |
| `model/` | 85% | Model lifecycle |
| `rag/` | 80% | Search quality |
| `workflow/` | 85% | Execution correctness |
| `ui/` | 60% | UI testing is less valuable |
| **Overall** | **>85%** | CI gate |

---

*End of TDD Strategy*
