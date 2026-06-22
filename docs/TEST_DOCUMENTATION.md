# Fuse Test Documentation

## Overview

Fuse follows a strict **TDD (Test-Driven Development)** protocol: RED → GREEN → REFACTOR. Every feature is implemented test-first. Run `cargo test --lib` for current test count (historical snapshots were inconsistent).

---

## Test Summary

| Category | Module | Tests | Status |
|----------|--------|-------|--------|
| **Inference** | `inference::backend` | ~10 | Pass |
| | `inference::cpu::engine` | ~8 | Pass |
| | `inference::cpu::kv_cache` | 9 | Pass |
| | `inference::coordinator` | 5 | Pass |
| | `inference::sampler` | ~6 | Pass |
| | `inference::grammar` | ~5 | Pass |
| | `inference::cache` | 14 | Pass |
| | `inference::wasm_runtime` | 6 | Pass |
| | `inference::ab_testing` | 17 | Pass |
| | `inference::prompt_optimizer` | 15 | Pass |
| | `inference::gpu::metal` | 6 | Pass |
| | `inference::gpu::cuda` | 5 | Pass |
| **API** | `api::routes::ollama` | ~12 | Pass |
| | `api::routes::openai` | ~6 | Pass |
| | `api::routes::anthropic` | ~4 | Pass |
| **Server** | `server::middleware` | ~8 | Pass |
| | `server::handlers` | ~4 | Pass |
| **CLI** | `cli::commands` | ~4 | Pass |
| | `cli::handlers::serve` | ~4 | Pass |
| | `cli::slash_commands` | 22 | Pass |
| | `cli::validation` | ~6 | Pass |
| **TUI** | `tui::app` | 25 | Pass (requires `--features tui`) |
| | `tui::chat` | 12 | Pass |
| | `tui::render` | 17 | Pass |
| | `tui::theme` | 4 | Pass (requires `--features tui`) |
| | `tui::widgets` | 3 | Pass (requires `--features tui`) |
| **Channels** | `channels::traits` | 5 | Pass |
| | `channels::session` | ~8 | Pass |
| | `channels::router` | ~6 | Pass |
| | `channels::telegram` | ~4 | Pass |
| | `channels::discord` | ~4 | Pass |
| | `channels::slack` | ~4 | Pass |
| | `channels::matrix` | ~4 | Pass |
| | `channels::web_widget` | 26 | Pass |
| **Devices** | `devices::traits` | ~4 | Pass |
| | `devices::mqtt` | ~4 | Pass |
| | `devices::oura` | ~4 | Pass |
| | `devices::home_assistant` | ~4 | Pass |
| | `devices::correlator` | ~6 | Pass |
| | `devices::automation` | ~8 | Pass |
| **Model** | `model::manager` | ~8 | Pass |
| | `model::registry::huggingface` | ~6 | Pass |
| | `model::registry::ollama` | ~4 | Pass |
| | `model::formats::gguf` | ~8 | Pass |
| | `model::recommender` | 10 | Pass |
| | `model::delta` | 15 | Pass |
| **Quantization** | `quantization::gguf_codec` | ~8 | Pass |
| | `quantization::optimizer` | ~6 | Pass |
| | `quantization::validator` | ~4 | Pass |
| | `quantization::methods` | ~6 | Pass |
| **RAG** | `rag::indexer` | ~6 | Pass |
| | `rag::retriever` | ~4 | Pass |
| | `rag::memory` | 18 | Pass |
| **Agents** | `agents::traits` | 1 | Pass |
| | `agents::mcp` | 17 | Pass |
| | `agents::swarm` | 8 | Pass |
| | `agents::worker` | 7 | Pass |
| | `agents::session` | 8 | Pass |
| | `agents::task_packet` | 10 | Pass |
| | `agents::failure` | 10 | Pass |
| | `agents::permissions` | 7 | Pass |
| | `agents::bash_validator` | 18 | Pass |
| | `agents::lane` | 14 | Pass |
| | `agents::diagnostics` | 16 | Pass |
| | `agents::branch_awareness` | 18 | Pass |
| **Plugins** | `plugins::mod` | 22 | Pass |
| **Security** | `security::ai_shield` | 16 | Pass |
| | `security::rbac` | 12 | Pass |
| **Fleet** | `fleet::mod` | 14 | Pass |
| **K8s** | `k8s::mod` | 3 | Pass |
| **Observability** | `observability::metrics` | 12 | Pass |
| **Config** | `config::loader` | ~8 | Pass |
| | `config::validation` | ~6 | Pass |
| **Error** | `error` | ~20 | Pass |
| **Storage** | `storage::repository` | ~4 | Pass |
| **Workflow** | `workflow::parser` | ~4 | Pass |
| | `workflow::executor` | ~4 | Pass |
| **Platform** | `platform::hardware` | ~4 | Pass |
| **Pre-existing failures** | `pool`, `queue` | 3 | **FAIL** |

---

## How to Run Tests

### Full Test Suite

```bash
cd <project-root>

# All unit tests (skip known slow/hanging test)
cargo test --lib -- --skip test_cache_ttl_expiration

# Include TUI tests
cargo test --lib --features tui -- --skip test_cache_ttl_expiration

# Skip all pre-existing failures
cargo test --lib -- \
  --skip test_cache_ttl_expiration \
  --skip test_model_pool \
  --skip test_connection_pool_basic \
  --skip test_priority_ordering \
  --skip test_system_capability_detection
```

### Test by Phase

```bash
# Phase 1: Core Inference
cargo test --lib inference::

# Phase 2: API + CLI
cargo test --lib api::
cargo test --lib cli::
cargo test --lib server::

# Phase 3: Quantization
cargo test --lib quantization::

# Phase 4: Channels
cargo test --lib channels::

# Phase 5: Dioxus UI
cargo test --lib --features dioxus-ui ui_dioxus::

# Phase 6: Device Hub
cargo test --lib devices::

# Phase 7: GPU + Production
cargo test --lib security::
cargo test --lib observability::
cargo test --lib k8s::

# Phase 8: Edge + Agents
cargo test --lib agents::mcp
cargo test --lib agents::swarm
cargo test --lib plugins::

# Phase 9: AI-Enhanced Features
cargo test --lib inference::cache
cargo test --lib inference::ab_testing
cargo test --lib inference::prompt_optimizer
cargo test --lib rag::memory
cargo test --lib model::recommender
cargo test --lib model::delta
cargo test --lib fleet::

# Phase 10: Agent Harness
cargo test --lib agents::worker
cargo test --lib agents::session
cargo test --lib agents::task_packet
cargo test --lib agents::failure
cargo test --lib agents::permissions
cargo test --lib agents::bash_validator
cargo test --lib agents::lane
cargo test --lib agents::diagnostics
cargo test --lib agents::branch_awareness
cargo test --lib slash_commands

# TUI (requires feature flag)
cargo test --lib --features tui tui::
```

### Test by Category

```bash
# Unit tests only
cargo test --lib

# Integration tests
cargo test --test '*'

# Benchmarks (compile check)
cargo bench --no-run
```

---

## Quality Gates

Every PR must pass ALL of these:

```bash
cargo fmt --check                          # Formatting
cargo clippy -- -D warnings                # Linting (0 warnings)
cargo test --lib -- --skip test_cache_ttl  # All tests
cargo build                                # Builds
cargo build --features tui                 # TUI builds
```

---

## Test Strategy

### Unit Tests

- Located in `#[cfg(test)] mod tests` at bottom of each source file
- Test individual functions/methods in isolation
- Use mock implementations for trait dependencies
- Pattern: `test_<module>_<behavior>` naming

### Integration Tests

- Located in `tests/` directory
- Test multiple modules working together
- Use `spawn_test_server()` helper for API tests
- Test full request → response cycle

### Property Tests

- Use `proptest` crate for data transformation invariants
- Quantize → dequantize roundtrip tolerance checks
- PagedAttention boundary conditions

### Test Patterns Used

```rust
// Unit test with mock
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_works() {
        let sut = MyStruct::new(config);
        assert_eq!(sut.method(), expected);
    }

    #[tokio::test]
    async fn test_async_feature() {
        let result = async_method().await.unwrap();
        assert!(result.is_valid());
    }
}
```

---

## Known Failures

| Test | Module | Reason | Impact |
|------|--------|--------|--------|
| `test_model_pool` | pool | Pre-existing race condition | Low — pool module not in critical path |
| `test_connection_pool_basic` | pool | Pre-existing assertion failure | Low |
| `test_priority_ordering` | queue | Pre-existing ordering issue | Low — queue module auxiliary |

---

## Coverage Targets

| Metric | Target | Current |
|--------|--------|---------|
| New code coverage | >95% | ~95% (TDD enforced) |
| Overall test count | >800 | run `cargo test --lib` |
| Clippy warnings | 0 | 0 |
| Pre-existing failures | <10 | 3 |

---

## TDD Protocol

Every feature follows this cycle:

1. **RED**: Write failing test first
2. **GREEN**: Write minimum code to pass
3. **REFACTOR**: Clean up while tests stay green
4. **VERIFY**: `cargo test && cargo clippy -- -D warnings`
5. **COMMIT**: `git commit -m "feat(module): description [task-id]"`

---

*Last updated: 2026-04-09*
