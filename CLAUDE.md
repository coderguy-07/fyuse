# CLAUDE.md — Fuse AI System Manager | Autonomous Development Prompt

## Identity

You are building **Fuse** — the world's most powerful open-source AI system manager. A single Rust binary that replaces Ollama + vLLM + OpenClaw + MimiClaw + LangChain. It runs on any device from Raspberry Pi to data center, with or without GPU.

**You operate in TDD-first, production-grade, autonomous mode.**

---

## Project State

- **Language**: Rust 2021 edition, MSRV 1.75+
- **UI**: Dioxus (desktop + web + mobile from one codebase)
- **Runtime**: tokio async + rayon for CPU-bound work
- **API**: axum (Ollama + OpenAI + Anthropic compatible)
- **Storage**: redb (embedded, pure Rust, ACID)
- **Config**: TOML with env var expansion, hot-reload
- **Testing**: cargo test + criterion benchmarks + proptest
- **CI**: GitHub Actions for Mac (ARM+Intel), Linux, Windows

---

## Core Architecture Rules

### 1. EVERYTHING IS A TRAIT

Every major component has a trait interface. This enables testing, swapping backends, and plugin extensibility.

```rust
// Pattern: Define trait → Implement → Test with mock → Wire in config
#[async_trait]
pub trait InferenceBackend: Send + Sync { ... }
pub trait Channel: Send + Sync { ... }
pub trait DeviceConnector: Send + Sync { ... }
pub trait QuantizationMethod: Send + Sync { ... }
pub trait StorageBackend: Send + Sync { ... }
pub trait Plugin: Send + Sync { ... }
```

### 2. CONFIG-DRIVEN EVERYTHING

Every feature is toggleable via `fuse.toml`. No hardcoded behavior. Use `#[cfg(feature = "...")]` for compile-time and `config.feature.enabled` for runtime.

```rust
// Pattern: Config struct → Default → Validate → Hot-reload
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InferenceConfig {
    pub default_model: String,
    pub max_loaded_models: usize,
    pub continuous_batching: bool,
    pub paged_attention: bool,
    // ... all configurable
}
```

### 3. CPU-FIRST, GPU-OPTIONAL

Every inference code path MUST work on CPU. GPU is an accelerator behind feature flags.

```rust
// CORRECT: CPU path is primary
fn matmul(a: &Tensor, b: &Tensor) -> Tensor {
    #[cfg(feature = "cuda")]
    if cuda_available() { return cuda_matmul(a, b); }
    
    #[cfg(feature = "metal")]  
    if metal_available() { return metal_matmul(a, b); }
    
    cpu_simd_matmul(a, b)  // Always available
}
```

### 4. MODULAR MODULE STRUCTURE

```
src/
├── main.rs                     # Entry point only
├── lib.rs                      # Re-exports
├── cli/                        # CLI (clap) — presentation only
├── api/                        # REST/WS API (axum) — presentation only
│   ├── routes/
│   │   ├── ollama.rs           # Ollama-compatible endpoints
│   │   ├── openai.rs           # OpenAI-compatible endpoints
│   │   ├── anthropic.rs        # Anthropic-compatible endpoints
│   │   └── admin.rs            # Admin/metrics endpoints
│   └── middleware/
├── inference/                  # Inference engine — core business logic
│   ├── backend.rs              # InferenceBackend trait
│   ├── coordinator.rs          # Request routing + continuous batching
│   ├── cpu/                    # CPU backend (candle + custom SIMD)
│   │   ├── engine.rs
│   │   ├── kv_cache.rs         # PagedAttention for CPU
│   │   └── simd/               # Platform-specific SIMD kernels
│   ├── gpu/                    # GPU backends (feature-gated)
│   └── remote/                 # Remote API proxy
├── quantization/               # Adaptive Quantization Engine
│   ├── engine.rs               # AQE coordinator
│   ├── profiler.rs             # Hardware + model profiling
│   ├── methods/                # TurboQuant, GGUF, AWQ, GPTQ
│   └── validator.rs            # Quality validation
├── model/                      # Model management
│   ├── manager.rs              # Lifecycle: pull, load, unload, remove
│   ├── registry/               # HuggingFace, Ollama, custom registries
│   ├── formats/                # GGUF, SafeTensors, ONNX readers
│   ├── merging.rs              # SLERP, TIES merging
│   └── resource_manager.rs     # Memory budgets, LRU, idle management
├── channels/                   # Multi-channel bridge (NEW)
│   ├── mod.rs                  # Channel trait
│   ├── telegram.rs             # Telegram Bot API
│   ├── discord.rs              # Discord gateway
│   ├── slack.rs                # Slack Events API
│   ├── matrix.rs               # Matrix protocol
│   ├── web_widget.rs           # Embeddable WASM widget
│   └── session.rs              # Per-user session management
├── devices/                    # Device Hub (NEW)
│   ├── mod.rs                  # DeviceConnector trait
│   ├── oura.rs                 # Oura Ring API
│   ├── apple_health.rs         # Apple HealthKit bridge
│   ├── home_assistant.rs       # Home Assistant integration
│   ├── mqtt.rs                 # Generic MQTT sensor gateway
│   └── correlator.rs           # AI-powered data correlation
├── rag/                        # RAG system
│   ├── indexer.rs              # Document/code indexing
│   ├── chunker.rs              # Semantic + syntactic chunking
│   ├── store.rs                # Vector store (redb)
│   └── retriever.rs            # Hybrid search + re-ranking
├── workflow/                   # Workflow engine
│   ├── parser.rs               # fuse.md DSL parser
│   ├── executor.rs             # DAG executor
│   └── scheduler.rs            # Cron + webhook triggers
├── agents/                     # Agent framework (NEW)
│   ├── mod.rs                  # Agent trait
│   ├── swarm.rs                # Multi-agent orchestration
│   ├── tools.rs                # Built-in tools
│   └── mcp.rs                  # MCP server + client
├── security/                   # AI Shield Gateway
│   ├── ai_shield.rs            # Middleware: prompt injection, PII
│   ├── rbac.rs                 # Role-based access control
│   ├── audit.rs                # Audit logging
│   └── sbom.rs                 # Model SBOM generation
├── config/                     # Configuration system
│   ├── loader.rs               # TOML loading with env expansion
│   ├── watcher.rs              # Hot-reload via file watching
│   └── validation.rs           # Config validation
├── platform/                   # Platform abstraction
│   ├── hardware.rs             # CPU/GPU/RAM detection
│   ├── simd.rs                 # SIMD capability detection
│   └── os.rs                   # OS-specific utilities
├── observability/              # Observability stack
│   ├── metrics.rs              # Prometheus metrics
│   ├── tracing.rs              # OpenTelemetry
│   └── logging.rs              # Structured JSON logging
├── storage/                    # Data persistence
│   ├── db.rs                   # redb database
│   ├── models.rs               # Model metadata store
│   └── sessions.rs             # Session persistence
└── ui/                         # Dioxus UI (NEW — replaces Yew)
    ├── app.rs                  # Root Dioxus component
    ├── pages/
    │   ├── chat.rs             # Chat interface
    │   ├── models.rs           # Model management
    │   ├── dashboard.rs        # System dashboard
    │   ├── channels.rs         # Channel management
    │   └── devices.rs          # Device hub dashboard
    └── components/             # Reusable components
```

### 5. ASYNC RULES

```rust
// RULE: Never block tokio threads with CPU work
// CORRECT:
let result = tokio::task::spawn_blocking(move || {
    inference_engine.forward(&tokens)  // CPU-bound
}).await?;

// WRONG:
let result = inference_engine.forward(&tokens).await;  // blocks tokio!
```

### 6. ERROR HANDLING

```rust
// Use thiserror for library errors
#[derive(Debug, thiserror::Error)]
pub enum FuseError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Inference failed: {0}")]
    InferenceFailed(String),
    #[error("Channel error: {channel}: {message}")]
    ChannelError { channel: String, message: String },
    // ... specific, actionable variants
}

// Use anyhow ONLY in main.rs and CLI handlers
// Never use unwrap() or expect() in library code
// Never use panic!() except in truly impossible states
```

---

## TDD Development Protocol

### The Iron Rule: **RED → GREEN → REFACTOR**

Every feature MUST follow this cycle:

```
1. WRITE FAILING TEST that defines the expected behavior
2. WRITE MINIMUM CODE to make the test pass
3. REFACTOR while keeping tests green
4. BENCHMARK if performance-critical (criterion)
5. PROPERTY TEST if data transformation (proptest)
```

### Test Organization

```
src/
├── inference/
│   ├── cpu/
│   │   ├── engine.rs
│   │   └── engine.rs         # Unit tests at bottom of file
├── ...

tests/                         # Integration tests
├── api/
│   ├── ollama_compat.rs       # Ollama API compatibility suite
│   ├── openai_compat.rs       # OpenAI API compatibility suite
│   └── streaming.rs           # WebSocket/SSE streaming tests
├── inference/
│   ├── cpu_inference.rs       # End-to-end CPU inference
│   ├── quantization.rs        # Quantize → load → infer round-trip
│   └── multi_model.rs         # Multi-model serving tests
├── channels/
│   ├── telegram_mock.rs       # Mock Telegram API tests
│   └── session.rs             # Session persistence tests
└── e2e/
    ├── pull_and_run.rs        # `fuse pull && fuse run` end-to-end
    └── serve_and_query.rs     # `fuse serve` + API query

benches/                       # Performance benchmarks
├── inference_throughput.rs     # tok/s benchmarks
├── quantization_speed.rs      # Quantization time benchmarks
├── api_latency.rs             # API response time
└── memory_usage.rs            # Memory consumption
```

### Test Patterns

```rust
// Unit test: test the module in isolation with mocks
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    
    #[test]
    fn test_model_router_selects_cpu_when_no_gpu() {
        let hw = HardwareProfile { gpu: None, ..default() };
        let router = ModelRouter::new(hw);
        assert_eq!(router.select_backend("model"), BackendType::CpuSimd);
    }
    
    #[tokio::test]
    async fn test_inference_streams_tokens() {
        let engine = MockInferenceBackend::new();
        engine.expect_stream().returning(|_, _| {
            Box::pin(stream::iter(vec![Ok(Token::new("hello")), Ok(Token::new(" world"))]))
        });
        let tokens: Vec<_> = engine.stream(&handle, request).collect().await;
        assert_eq!(tokens.len(), 2);
    }
}

// Integration test: test multiple modules working together
#[tokio::test]
async fn test_ollama_api_generate() {
    let app = spawn_test_server().await;
    let resp = app.post("/api/generate")
        .json(&json!({"model": "test-model", "prompt": "Hi"}))
        .send().await;
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await;
    assert!(body["response"].as_str().unwrap().len() > 0);
}

// Property test: verify invariants
proptest! {
    #[test]
    fn quantize_dequantize_roundtrip(weights in prop::collection::vec(-1.0f32..1.0, 1..1000)) {
        let quantized = quantize_q4_0(&weights);
        let dequantized = dequantize_q4_0(&quantized);
        for (a, b) in weights.iter().zip(dequantized.iter()) {
            prop_assert!((a - b).abs() < 0.5);  // Q4_0 tolerance
        }
    }
}

// Benchmark: track performance
fn bench_inference_7b(c: &mut Criterion) {
    c.bench_function("7B_Q4_K_M_generate_100_tokens", |b| {
        b.iter(|| engine.generate(black_box(&prompt), 100))
    });
}
```

### Quality Gates (CI Must Pass)

```yaml
# Every PR must pass ALL of these:
quality_gates:
  - cargo fmt --check                    # Formatting
  - cargo clippy -- -D warnings          # Linting
  - cargo test                           # All tests
  - cargo test --features cuda           # GPU tests (if available)
  - cargo audit                          # Security audit
  - cargo bench --no-run                 # Benchmarks compile
  - cargo build --target wasm32-wasi     # WASM builds
  - coverage >= 85%                      # Code coverage
  - binary_size < 15MB                   # Size check
  - no_unsafe_without_comment            # Safety check
```

---

## Autonomous Development Playbook

When Claude is given a task, follow this exact protocol:

### Phase 1: Understand
1. Read the relevant PRD section (docs/feature-requirements/14-PRD-v2-complete.md)
2. Read existing code in the target module
3. Identify the trait interface (or design one)
4. Check for existing tests

### Phase 2: Test First
1. Write failing integration test for the feature
2. Write unit tests for each function/method
3. Write property tests for data transformations
4. Run tests — confirm they fail (RED)

### Phase 3: Implement
1. Define the trait (if new module)
2. Implement the struct
3. Wire into config system
4. Run tests — make them pass (GREEN)

### Phase 4: Refactor
1. Remove duplication
2. Ensure error handling is complete
3. Add structured logging (`tracing::info!`, `tracing::error!`)
4. Run tests — still green

### Phase 5: Integrate
1. Wire into CLI commands (if user-facing)
2. Wire into API routes (if API-facing)
3. Wire into config loader
4. Write integration test for full flow
5. Run ALL tests

### Phase 6: Document
1. Add rustdoc comments to public API
2. Update TOML config example
3. Update CLI help text

---

## Coding Standards

### Rust Style
- `cargo fmt` (defaults, no custom rustfmt.toml)
- `cargo clippy -- -D warnings` (treat all warnings as errors)
- No `unwrap()` or `expect()` in library code — use `?` operator
- No `panic!()` except truly impossible states (document why)
- Minimize `unsafe` — only in SIMD kernels in `inference/cpu/simd/`
- Every `unsafe` block has a `// SAFETY:` comment
- Prefer `Arc<T>` over `Rc<T>` (we're always multi-threaded)
- Prefer `DashMap` over `RwLock<HashMap>` for concurrent maps
- Prefer `parking_lot` over `std::sync` for mutexes

### Naming
- Types: `PascalCase` — `ModelRouter`, `InferenceBackend`
- Functions: `snake_case` — `load_model`, `auto_quantize`
- Constants: `SCREAMING_SNAKE` — `MAX_CONTEXT_LENGTH`
- Feature flags: `kebab-case` — `continuous-batching`
- Config keys: `snake_case` — `max_loaded_models`
- Test functions: `test_<module>_<behavior>` — `test_router_selects_cpu`

### Dependencies
- Prefer pure-Rust crates
- Pin major versions
- Run `cargo audit` after adding
- Check license (Apache-2.0, MIT, BSD, ISC acceptable)
- No crates with `unsafe` in hot paths unless audited

### Performance
- Never allocate in inference hot loops
- Memory-map model files (`memmap2`)
- Zero-copy deserialization where possible (`zerocopy`)
- Profile with `cargo flamegraph` before optimizing
- Benchmark with criterion before and after changes

---

## Feature Flag Map

```toml
[features]
default = ["cpu-inference", "cli", "api-server", "gguf"]

# Inference
cpu-inference = ["candle-core", "candle-transformers"]
continuous-batching = ["cpu-inference"]
paged-attention = ["cpu-inference"]
speculative-decoding = ["cpu-inference"]

# GPU (compile-time)
cuda = ["candle-core/cuda", "cudarc"]
metal = ["candle-core/metal"]
vulkan = ["vulkano", "ash"]

# Quantization
gguf = ["cpu-inference"]
turboquant = ["gguf"]
awq = ["gguf"]
gptq = ["gguf"]

# API
api-server = ["axum", "tower", "tower-http"]
ollama-compat = ["api-server"]
openai-compat = ["api-server"]
anthropic-compat = ["api-server"]

# Channels
channels = ["api-server"]
telegram = ["channels", "teloxide"]
discord = ["channels", "serenity"]
slack = ["channels"]
matrix = ["channels", "matrix-sdk"]

# Devices
device-hub = []
oura = ["device-hub", "reqwest"]
apple-health = ["device-hub"]
home-assistant = ["device-hub", "reqwest"]
mqtt-devices = ["device-hub", "rumqttc"]

# UI
dioxus-ui = ["dioxus", "dioxus-web"]
tui = ["ratatui", "crossterm"]

# RAG
rag = ["cpu-inference"]

# Agents
agents = ["api-server"]
mcp = ["agents"]

# Production
kubernetes = ["kube", "k8s-openapi"]
observability = ["opentelemetry", "tracing-opentelemetry", "opentelemetry-otlp"]
ai-shield = ["api-server"]

# Edge
edge = ["cpu-inference", "cli"]  # Minimal
wasm-runtime = ["wasmtime"]
```

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `Cargo.toml` | Dependencies + feature flags |
| `fuse.toml` | Runtime configuration |
| `src/main.rs` | Entry point |
| `src/lib.rs` | Library root + re-exports |
| `src/inference/backend.rs` | InferenceBackend trait (most important) |
| `src/inference/coordinator.rs` | Request routing + batching |
| `src/channels/mod.rs` | Channel trait |
| `src/devices/mod.rs` | DeviceConnector trait |
| `src/quantization/engine.rs` | AQE coordinator |
| `src/config/loader.rs` | Config loading + validation |
| `docs/feature-requirements/14-PRD-v2-complete.md` | Full PRD |
| `docs/feature-requirements/15-dev-strategy.md` | Development strategy |
| `docs/feature-requirements/16-tdd-strategy.md` | TDD guide |
| `docs/feature-requirements/17-autopilot-tasks.md` | Task manifest |

## Performance Targets

| Metric | Target | Gate |
|--------|--------|------|
| 7B Q4_K_M on M2 | >10 tok/s | CI benchmark |
| 3B Q4_K_M on M2 | >25 tok/s | CI benchmark |
| API p99 (excl inference) | <10ms | Load test |
| Binary size (default) | <15MB | CI check |
| Memory overhead | <50MB | CI check |
| Test coverage | >85% | CI gate |
| Clippy warnings | 0 | CI gate |

## Quick Commands

```bash
cargo build                          # Dev build
cargo build --release                # Release
cargo test                           # All tests
cargo test --lib                     # Unit tests only
cargo test --test api                # Integration tests
cargo bench                          # Benchmarks
cargo fmt && cargo clippy -- -D warnings  # Lint
cargo build --features edge --no-default-features  # Edge build
cargo build --target wasm32-unknown-unknown --features dioxus-ui  # WASM
```
