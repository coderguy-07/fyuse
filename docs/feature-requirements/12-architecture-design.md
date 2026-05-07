# Fuse Architecture Design Document

## Version: 2.0.0
## Date: 2026-04-04
## Status: Final

---

## 1. Design Philosophy

### Core Principles
1. **CPU-First, GPU-Optional**: Every code path optimized for CPU. GPU is an accelerator, not a requirement.
2. **Single Binary**: One statically-linked binary. No Python, no Node.js, no runtime dependencies.
3. **Zero-Copy Everything**: Memory-mapped model files, zero-copy I/O, minimal allocations in hot paths.
4. **Compile-Time Feature Selection**: Feature flags gate GPU backends, UI, enterprise features — edge binary stays small.
5. **Backend Agnostic**: Trait-based abstractions allow swapping inference backends without API changes.

---

## 2. System Architecture

### 2.1 Layer Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                            │
│                                                                      │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐  ┌──────────────┐  │
│  │   CLI    │  │   REST API   │  │  WebSocket │  │   Web UI     │  │
│  │  (clap)  │  │   (axum)     │  │  (axum-ws) │  │ (Yew/WASM)  │  │
│  └────┬─────┘  └──────┬───────┘  └─────┬──────┘  └──────┬───────┘  │
│       └───────────────┬┼───────────────┬┘                │          │
├───────────────────────┼┼───────────────┼─────────────────┼──────────┤
│                       ││               │   Service Layer  │          │
│  ┌────────────────────▼▼───────────────▼─────────────────▼───────┐  │
│  │                    Request Router                              │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐   │  │
│  │  │ Auth/AuthZ  │  │ Rate Limiter │  │ AI Shield Gateway   │   │  │
│  │  └─────────────┘  └──────────────┘  └─────────────────────┘   │  │
│  └───────────────────────────┬───────────────────────────────────┘  │
│                              │                                       │
│  ┌───────────────────────────▼───────────────────────────────────┐  │
│  │                  Orchestration Engine                          │  │
│  │                                                                │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │  │
│  │  │ Model Router │  │ Batch Queue  │  │ Workflow Engine     │   │  │
│  │  │ (smart       │  │ (priority    │  │ (DAG executor)      │   │  │
│  │  │  dispatch)   │  │  scheduler)  │  │                     │   │  │
│  │  └──────┬───────┘  └──────┬───────┘  └─────────┬──────────┘   │  │
│  │         └─────────────────┼─────────────────────┘              │  │
│  └───────────────────────────┼───────────────────────────────────┘  │
│                              │                                       │
├──────────────────────────────┼───────────────────────────────────────┤
│                              │     Inference Layer                    │
│  ┌───────────────────────────▼───────────────────────────────────┐  │
│  │              Inference Coordinator                             │  │
│  │                                                                │  │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐   │  │
│  │  │  CPU Backend   │  │  GPU Backend   │  │ Remote Backend │   │  │
│  │  │                │  │                │  │                │   │  │
│  │  │ ┌────────────┐ │  │ ┌────────────┐ │  │ ┌────────────┐ │   │  │
│  │  │ │ candle-cpu │ │  │ │ candle-cuda│ │  │ │ OpenAI API │ │   │  │
│  │  │ ├────────────┤ │  │ ├────────────┤ │  │ ├────────────┤ │   │  │
│  │  │ │ SIMD       │ │  │ │ Metal      │ │  │ │ Ollama API │ │   │  │
│  │  │ │ Kernels    │ │  │ ├────────────┤ │  │ ├────────────┤ │   │  │
│  │  │ │ (AVX/NEON/ │ │  │ │ Vulkan     │ │  │ │ Anthropic  │ │   │  │
│  │  │ │  AMX/SVE)  │ │  │ │ Compute    │ │  │ │ API        │ │   │  │
│  │  │ └────────────┘ │  │ └────────────┘ │  │ └────────────┘ │   │  │
│  │  └────────────────┘  └────────────────┘  └────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│                         Quantization Layer                            │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │          Adaptive Quantization Engine (AQE)                    │  │
│  │                                                                │  │
│  │  ┌────────────┐  ┌─────────────────────────────────────────┐  │  │
│  │  │ Hardware   │  │ Quantization Backends                    │  │  │
│  │  │ Profiler   │  │                                          │  │  │
│  │  │            │  │  ┌────────┐ ┌──────┐ ┌─────┐ ┌───────┐ │  │  │
│  │  │ CPU caps   │──│  │Turbo   │ │ GGUF │ │ AWQ │ │ GPTQ  │ │  │  │
│  │  │ RAM size   │  │  │Quant   │ │      │ │     │ │       │ │  │  │
│  │  │ GPU avail  │  │  └────────┘ └──────┘ └─────┘ └───────┘ │  │  │
│  │  │ SIMD level │  └─────────────────────────────────────────┘  │  │
│  │  └────────────┘                                                │  │
│  │                                                                │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │ Quality Validator                                        │  │  │
│  │  │ Perplexity | MMLU | HumanEval | Custom Benchmarks       │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│                         Data Layer                                    │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────┐  │
│  │ Model Store  │  │ Vector Store │  │ Config Store │  │ Audit  │  │
│  │ (mmap GGUF)  │  │ (redb)       │  │ (TOML)       │  │ Log    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────┘  │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│                         Platform Layer                                │
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────┐ │
│  │ Security │  │ Observe  │  │ Resource │  │ Plugin   │  │ K8s  │ │
│  │ (TLS,    │  │ (OTel,   │  │ Manager  │  │ System   │  │ Op   │ │
│  │  RBAC)   │  │  Prom)   │  │ (pools)  │  │ (dylib)  │  │      │ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 3. Core Component Design

### 3.1 Inference Backend Trait

The key abstraction that enables CPU-first with optional GPU:

```rust
/// Core trait all inference backends must implement
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Get backend name and capabilities
    fn info(&self) -> BackendInfo;
    
    /// Load a model into this backend
    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle>;
    
    /// Unload a model, freeing resources
    async fn unload_model(&self, handle: &ModelHandle) -> Result<()>;
    
    /// Run inference (single request)
    async fn infer(&self, handle: &ModelHandle, request: InferenceRequest) -> Result<InferenceResponse>;
    
    /// Stream inference tokens
    fn stream(&self, handle: &ModelHandle, request: InferenceRequest) 
        -> Pin<Box<dyn Stream<Item = Result<Token>> + Send>>;
    
    /// Generate embeddings
    async fn embed(&self, handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    
    /// Get current resource usage
    fn resource_usage(&self) -> ResourceUsage;
}

pub struct BackendInfo {
    pub name: String,
    pub backend_type: BackendType,       // CPU, CUDA, Metal, Vulkan, Remote
    pub supported_formats: Vec<ModelFormat>,
    pub max_context_length: usize,
    pub supports_batching: bool,
    pub supports_kv_cache: bool,
    pub simd_capabilities: SimdCaps,     // AVX2, AVX512, NEON, AMX, SVE
}

pub enum BackendType {
    CpuSimd,      // Optimized CPU with SIMD
    CudaGpu,      // NVIDIA CUDA
    MetalGpu,     // Apple Metal  
    VulkanGpu,    // Cross-platform Vulkan
    RemoteApi,    // Proxy to remote API
    WasmEdge,     // WebAssembly edge runtime
}
```

### 3.2 Adaptive Quantization Engine (AQE)

```rust
pub struct AdaptiveQuantizer {
    hardware_profile: HardwareProfile,
    quality_threshold: f64,           // Minimum acceptable quality (0.0-1.0)
    target_memory: Option<usize>,     // Target memory budget in bytes
}

impl AdaptiveQuantizer {
    /// Automatically select the best quantization strategy
    pub async fn auto_quantize(
        &self,
        model_path: &Path,
        config: AutoQuantConfig,
    ) -> Result<QuantizedModel> {
        // Step 1: Profile the model
        let model_profile = self.profile_model(model_path).await?;
        
        // Step 2: Profile hardware capabilities
        let hw_caps = self.detect_hardware();
        
        // Step 3: Select strategy per layer group
        let strategy = self.select_strategy(&model_profile, &hw_caps, &config);
        
        // Step 4: Apply quantization
        let quantized = self.apply_quantization(model_path, &strategy).await?;
        
        // Step 5: Validate quality
        let quality = self.validate_quality(&quantized, &config.calibration_data).await?;
        
        if quality.score < self.quality_threshold {
            // Step 6: Auto-adjust (increase precision on worst layers)
            return self.adjust_and_retry(quantized, quality, &config).await;
        }
        
        Ok(quantized)
    }
    
    fn select_strategy(
        &self,
        model: &ModelProfile,
        hw: &HardwareProfile,
        config: &AutoQuantConfig,
    ) -> QuantizationStrategy {
        let mut layer_strategies = Vec::new();
        
        for layer in &model.layers {
            let method = match (layer.sensitivity, hw.simd_caps, config.target_quality) {
                // High sensitivity layers get conservative quantization
                (Sensitivity::Critical, _, _) => LayerQuant::Q8_0,
                
                // If hardware supports TurboQuant-friendly SIMD
                (Sensitivity::High, SimdCaps::Avx512 | SimdCaps::Amx, _) => 
                    LayerQuant::TurboQuant { bits: 4 },
                (Sensitivity::Medium, SimdCaps::Avx512 | SimdCaps::Amx, _) => 
                    LayerQuant::TurboQuant { bits: 3 },
                (Sensitivity::Low, SimdCaps::Avx512 | SimdCaps::Amx, _) => 
                    LayerQuant::TurboQuant { bits: 2 },
                    
                // Fallback to GGUF for simpler SIMD
                (Sensitivity::High, _, _) => LayerQuant::Q5_K_M,
                (Sensitivity::Medium, _, _) => LayerQuant::Q4_K_M,
                (Sensitivity::Low, _, _) => LayerQuant::Q4_0,
            };
            
            layer_strategies.push((layer.name.clone(), method));
        }
        
        QuantizationStrategy { layers: layer_strategies }
    }
}
```

### 3.3 Hardware Detection & SIMD Dispatch

```rust
pub struct HardwareProfile {
    pub cpu_arch: CpuArch,              // x86_64, aarch64, riscv64
    pub simd_caps: SimdCaps,            // AVX2, AVX-512, NEON, AMX, SVE
    pub cpu_cores: usize,
    pub ram_total: usize,
    pub ram_available: usize,
    pub gpu: Option<GpuInfo>,
    pub memory_bandwidth: f64,          // GB/s (estimated)
}

#[derive(Debug, Clone, Copy)]
pub enum SimdCaps {
    None,
    Sse42,       // x86 SSE4.2
    Avx2,        // x86 AVX2 (256-bit)
    Avx512,      // x86 AVX-512 (512-bit)
    Amx,         // Intel AMX (matrix)
    Neon,        // ARM NEON (128-bit)
    Sve,         // ARM SVE (scalable)
    Sve2,        // ARM SVE2
    Rvv,         // RISC-V Vector
}

/// Runtime SIMD dispatch for matrix operations
/// Compiles specialized kernels for each SIMD level
pub fn matmul_quantized(
    a: &QuantizedTensor,
    b: &QuantizedTensor,
    caps: SimdCaps,
) -> Tensor {
    match caps {
        SimdCaps::Avx512 | SimdCaps::Amx => matmul_avx512(a, b),
        SimdCaps::Avx2 => matmul_avx2(a, b),
        SimdCaps::Neon | SimdCaps::Sve => matmul_neon(a, b),
        _ => matmul_scalar(a, b),  // Fallback: works everywhere
    }
}
```

### 3.4 Model Router (Smart Dispatch)

```rust
/// Routes inference requests to the optimal backend/model
pub struct ModelRouter {
    backends: Vec<Arc<dyn InferenceBackend>>,
    loaded_models: DashMap<ModelId, (ModelHandle, Arc<dyn InferenceBackend>)>,
    resource_manager: Arc<ResourceManager>,
    config: RouterConfig,
}

impl ModelRouter {
    pub async fn route(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        // 1. Find or load the requested model
        let (handle, backend) = self.ensure_model_loaded(&request.model).await?;
        
        // 2. Check if we should use a different backend (e.g., GPU available now)
        let backend = self.select_optimal_backend(&handle, &request).await.unwrap_or(backend);
        
        // 3. Execute inference
        backend.infer(&handle, request).await
    }
    
    async fn select_optimal_backend(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> Option<Arc<dyn InferenceBackend>> {
        // Prefer GPU if available and model fits in VRAM
        // Prefer CPU if GPU is busy or model is CPU-optimized (TurboQuant)
        // Prefer Remote if local resources are exhausted
        let gpu_backend = self.backends.iter()
            .find(|b| matches!(b.info().backend_type, BackendType::CudaGpu | BackendType::MetalGpu));
        
        if let Some(gpu) = gpu_backend {
            let usage = gpu.resource_usage();
            if usage.memory_available > handle.model_size() {
                return Some(gpu.clone());
            }
        }
        
        None // Stick with current backend
    }
}
```

### 3.5 Resource Manager

```rust
/// Manages model lifecycle and resource allocation
pub struct ResourceManager {
    loaded_models: Arc<RwLock<Vec<LoadedModel>>>,
    config: ResourceConfig,
    metrics: Arc<ResourceMetrics>,
}

pub struct LoadedModel {
    pub id: ModelId,
    pub state: ModelState,
    pub backend: Arc<dyn InferenceBackend>,
    pub handle: ModelHandle,
    pub last_used: Instant,
    pub memory_bytes: usize,
    pub request_count: AtomicU64,
}

pub enum ModelState {
    Active,           // Currently processing requests
    Idle,             // Loaded but no active requests
    Compressed,       // Memory-compressed (KV-cache evicted)
    CpuOffloaded,     // Moved from GPU to CPU RAM
    Unloaded,         // Removed from memory
}

impl ResourceManager {
    /// Background task that optimizes resource usage
    pub async fn optimization_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            
            let mut models = self.loaded_models.write().await;
            let now = Instant::now();
            
            for model in models.iter_mut() {
                let idle_duration = now - model.last_used;
                
                match model.state {
                    ModelState::Active => continue,
                    ModelState::Idle if idle_duration > self.config.compress_after => {
                        model.compress().await;
                        model.state = ModelState::Compressed;
                    }
                    ModelState::Compressed if idle_duration > self.config.offload_after => {
                        model.offload_to_cpu().await;
                        model.state = ModelState::CpuOffloaded;
                    }
                    ModelState::CpuOffloaded if idle_duration > self.config.unload_after => {
                        model.unload().await;
                        model.state = ModelState::Unloaded;
                    }
                    _ => {}
                }
            }
            
            // Evict if over memory budget (LRU)
            self.enforce_memory_budget(&mut models).await;
        }
    }
}
```

---

## 4. Cross-Platform Strategy

### 4.1 Compilation Targets

```toml
# .cargo/config.toml - Build profiles per platform

[target.x86_64-unknown-linux-gnu]
# Linux x86_64: Full features, static linking with musl for portability
rustflags = ["-C", "target-feature=+avx2"]

[target.aarch64-unknown-linux-gnu]
# Linux ARM64: Servers, Raspberry Pi
rustflags = ["-C", "target-feature=+neon"]

[target.aarch64-apple-darwin]
# macOS Apple Silicon: Metal GPU, AMX
rustflags = ["-C", "target-feature=+neon"]

[target.x86_64-pc-windows-msvc]
# Windows: DirectML for GPU
rustflags = ["-C", "target-feature=+avx2"]

[target.wasm32-wasi]
# WASM: Edge/browser deployment
rustflags = []
```

### 4.2 Feature Flags

```toml
# Cargo.toml feature flags
[features]
default = ["cpu-inference", "cli", "api-server"]

# Inference backends
cpu-inference = ["candle-core", "candle-transformers"]
cuda = ["candle-core/cuda", "cudarc"]
metal = ["candle-core/metal", "metal-rs"]
vulkan = ["vulkano", "ash"]

# Quantization methods
quantization = ["cpu-inference"]
turboquant = ["quantization"]
gptq = ["quantization"]
awq = ["quantization"]

# API and networking
api-server = ["axum", "tower", "tower-http"]
openai-compat = ["api-server"]
ollama-compat = ["api-server"]

# UI
web-ui = ["yew", "wasm-bindgen", "web-sys"]
tui = ["ratatui", "crossterm"]

# Production
kubernetes = ["kube", "k8s-openapi"]
observability = ["opentelemetry", "tracing-opentelemetry"]
ai-shield = ["api-server"]

# Edge
edge = ["cpu-inference"]  # Minimal build for constrained devices
wasm-runtime = ["wasmtime"]
```

### 4.3 Binary Size Targets

| Build Profile | Features | Binary Size | Target |
|---------------|----------|-------------|--------|
| Edge Minimal | cpu-inference, cli | ~5 MB | IoT, RPi |
| Developer | default | ~15 MB | Desktop |
| Server | default, observability, ai-shield | ~25 MB | Production |
| Full | all features | ~40 MB | Enterprise |

---

## 5. Data Flow Diagrams

### 5.1 Inference Request Flow

```
Client Request
       │
       ▼
┌──────────────┐     ┌──────────────┐
│  API Server  │────▶│ Auth/AuthZ   │
│  (axum)      │     │              │
└──────┬───────┘     └──────────────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│  AI Shield   │────▶│ Prompt       │
│  Gateway     │     │ Injection    │
│  (optional)  │     │ Detection    │
└──────┬───────┘     └──────────────┘
       │
       ▼
┌──────────────┐
│ Model Router │
│              │──── Which model? Which backend?
└──────┬───────┘
       │
       ├─────────────────────────┐
       │                         │
       ▼                         ▼
┌──────────────┐         ┌──────────────┐
│ CPU Backend  │         │ GPU Backend  │
│              │         │              │
│ 1. Tokenize  │         │ 1. Tokenize  │
│ 2. KV-cache  │         │ 2. KV-cache  │
│ 3. Forward   │         │ 3. Forward   │
│    (SIMD)    │         │    (CUDA/    │
│ 4. Sample    │         │     Metal)   │
│ 5. Decode    │         │ 4. Sample    │
└──────┬───────┘         │ 5. Decode    │
       │                 └──────┬───────┘
       │                        │
       └──────────┬─────────────┘
                  │
                  ▼
           ┌──────────────┐
           │ Response      │
           │ Streaming     │
           └──────┬───────┘
                  │
                  ▼
           ┌──────────────┐
           │ Metrics &    │
           │ Logging      │
           └──────────────┘
```

### 5.2 Auto-Quantization Flow

```
fuse pull model --quantize auto
       │
       ▼
┌──────────────────┐
│ Download Model   │
│ (full precision) │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Profile Model    │
│ - Layer count    │
│ - Weight ranges  │
│ - Sensitivity    │
│   analysis       │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Detect Hardware  │
│ - CPU (SIMD)     │
│ - RAM available  │
│ - GPU (optional) │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Select Strategy  │
│                  │
│ Memory budget:   │
│   8GB available  │
│ CPU: AVX-512     │
│                  │
│ Decision:        │
│ embed -> Q8_0    │
│ attn  -> TQ-4bit │
│ ffn   -> TQ-2bit │
│ out   -> Q6_K    │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Apply Per-Layer  │
│ Quantization     │
│ (parallel with   │
│  rayon)          │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Quality Check    │
│ - Perplexity     │
│ - Sample outputs │
│                  │
│ Score: 96.2%     │
│ (target: 95%)    │
│ ✅ PASS          │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Save as GGUF     │
│ with metadata    │
│                  │
│ Original: 14 GB  │
│ Quantized: 3.1GB │
│ Speedup: 3.2x    │
└──────────────────┘
```

---

## 6. Module Structure

```
fuse/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Entry point
│   ├── lib.rs                     # Library root, re-exports
│   │
│   ├── cli/                       # CLI layer
│   │   ├── mod.rs
│   │   ├── commands.rs            # Command definitions (clap)
│   │   ├── handlers.rs            # Command handlers
│   │   ├── output.rs              # Pretty printing, progress bars
│   │   └── completions.rs         # Shell completions
│   │
│   ├── api/                       # REST API layer
│   │   ├── mod.rs
│   │   ├── server.rs              # axum server setup
│   │   ├── routes/
│   │   │   ├── ollama.rs          # Ollama-compatible endpoints
│   │   │   ├── openai.rs          # OpenAI-compatible endpoints
│   │   │   ├── models.rs          # Model management endpoints
│   │   │   ├── admin.rs           # Admin/metrics endpoints
│   │   │   └── websocket.rs       # WebSocket streaming
│   │   ├── middleware/
│   │   │   ├── auth.rs            # Authentication
│   │   │   ├── rate_limit.rs      # Rate limiting
│   │   │   └── ai_shield.rs       # AI Shield Gateway
│   │   └── dto.rs                 # Request/Response types
│   │
│   ├── inference/                 # Inference engine
│   │   ├── mod.rs
│   │   ├── backend.rs             # InferenceBackend trait
│   │   ├── cpu/
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs          # CPU inference engine
│   │   │   ├── simd/
│   │   │   │   ├── avx2.rs        # AVX2 kernels
│   │   │   │   ├── avx512.rs      # AVX-512 kernels
│   │   │   │   ├── neon.rs        # ARM NEON kernels
│   │   │   │   └── scalar.rs      # Fallback scalar
│   │   │   └── kv_cache.rs        # KV-cache management
│   │   ├── gpu/
│   │   │   ├── mod.rs
│   │   │   ├── cuda.rs            # CUDA backend
│   │   │   ├── metal.rs           # Metal backend
│   │   │   └── vulkan.rs          # Vulkan backend
│   │   ├── remote/
│   │   │   ├── mod.rs
│   │   │   ├── openai.rs          # OpenAI API proxy
│   │   │   ├── anthropic.rs       # Anthropic API proxy
│   │   │   └── ollama.rs          # Ollama API proxy
│   │   ├── router.rs              # Model router / smart dispatch
│   │   ├── tokenizer.rs           # Tokenizer management
│   │   └── sampler.rs             # Token sampling strategies
│   │
│   ├── quantization/              # Adaptive Quantization Engine
│   │   ├── mod.rs
│   │   ├── engine.rs              # AQE coordinator
│   │   ├── profiler.rs            # Model & hardware profiling
│   │   ├── methods/
│   │   │   ├── turboquant.rs      # TurboQuant implementation
│   │   │   ├── gguf.rs            # GGUF quantization (Q4_K_M, etc.)
│   │   │   ├── gptq.rs            # GPTQ quantization
│   │   │   ├── awq.rs             # AWQ quantization
│   │   │   └── aqlm.rs            # AQLM quantization
│   │   ├── validator.rs           # Quality validation (perplexity, etc.)
│   │   └── optimizer.rs           # Per-layer strategy optimizer
│   │
│   ├── model/                     # Model management
│   │   ├── mod.rs
│   │   ├── manager.rs             # Model lifecycle management
│   │   ├── registry/
│   │   │   ├── huggingface.rs     # HuggingFace Hub integration
│   │   │   ├── ollama.rs          # Ollama registry
│   │   │   └── custom.rs          # Custom registry support
│   │   ├── formats/
│   │   │   ├── gguf.rs            # GGUF format reader/writer
│   │   │   ├── safetensors.rs     # SafeTensors reader
│   │   │   └── onnx.rs            # ONNX format support
│   │   ├── merging.rs             # Model merging (SLERP, TIES)
│   │   ├── layers.rs              # Layer manipulation
│   │   ├── metadata.rs            # Model metadata
│   │   └── resource_manager.rs    # Resource optimization
│   │
│   ├── rag/                       # RAG system
│   │   ├── mod.rs
│   │   ├── indexer.rs             # Document/code indexing
│   │   ├── chunker.rs             # Chunking strategies
│   │   ├── embedder.rs            # Embedding generation
│   │   ├── store.rs               # Vector store (redb-based)
│   │   └── retriever.rs           # Retrieval + re-ranking
│   │
│   ├── workflow/                  # Workflow engine
│   │   ├── mod.rs
│   │   ├── parser.rs              # fuse.md parser
│   │   ├── executor.rs            # DAG executor
│   │   ├── scheduler.rs           # Task scheduling
│   │   └── state.rs               # Workflow state management
│   │
│   ├── security/                  # Security subsystem
│   │   ├── mod.rs
│   │   ├── ai_shield.rs           # AI Shield Gateway
│   │   ├── prompt_guard.rs        # Prompt injection detection
│   │   ├── pii_filter.rs          # PII detection/redaction
│   │   ├── sbom.rs                # Model SBOM generation
│   │   └── audit.rs               # Audit logging
│   │
│   ├── config/                    # Configuration
│   │   ├── mod.rs
│   │   ├── loader.rs              # Config loading (TOML)
│   │   ├── watcher.rs             # Hot-reload file watcher
│   │   └── directory.rs           # Directory management
│   │
│   ├── platform/                  # Platform abstractions
│   │   ├── mod.rs
│   │   ├── hardware.rs            # Hardware detection
│   │   ├── simd_detect.rs         # SIMD capability detection
│   │   └── os.rs                  # OS-specific utilities
│   │
│   ├── observability/             # Observability
│   │   ├── mod.rs
│   │   ├── metrics.rs             # Prometheus metrics
│   │   ├── tracing.rs             # OpenTelemetry tracing
│   │   └── logging.rs             # Structured logging
│   │
│   └── ui/                        # Web UI (Yew/WASM)
│       ├── mod.rs
│       ├── app.rs                 # Root component
│       ├── pages/
│       │   ├── chat.rs            # Chat interface
│       │   ├── models.rs          # Model management
│       │   ├── dashboard.rs       # System dashboard
│       │   └── settings.rs        # Settings page
│       └── components/            # Reusable UI components
│
├── k8s/                           # Kubernetes manifests
│   ├── operator/                  # K8s operator
│   ├── helm/                      # Helm chart
│   └── examples/                  # Deployment examples
│
├── plugins/                       # Plugin SDK
│   ├── sdk/                       # Plugin development SDK
│   └── examples/                  # Example plugins
│
├── benches/                       # Benchmarks
│   ├── inference.rs
│   ├── quantization.rs
│   └── api_throughput.rs
│
└── tests/                         # Integration tests
    ├── api/
    ├── inference/
    ├── quantization/
    └── e2e/
```

---

## 7. Key Technical Decisions

### 7.1 Why Candle as Primary Backend

| Factor | Candle | Burn | tch-rs | Custom |
|--------|--------|------|--------|--------|
| Rust-native | Yes | Yes | No (PyTorch FFI) | Yes |
| GGUF support | Yes | No | No | Manual |
| CUDA support | Yes | Yes | Yes | Manual |
| Metal support | Yes | Yes | No | Manual |
| Maturity | High (HuggingFace) | Medium | High | N/A |
| Community | Large | Growing | Large | N/A |
| Binary size | Small | Small | Large (libTorch) | Smallest |

**Also consider**:
- **mistral.rs**: Rust-native inference engine supporting GGUF, GPTQ, ISQ with CPU/CUDA/Metal. Most feature-complete Rust option alongside candle.
- **tract** (Sonos): Pure Rust ONNX/NNEF engine, production-proven at Sonos, excellent for embedded/edge.
- **ort**: ONNX Runtime Rust bindings — battle-tested C++ engine, supports CoreML/DirectML/XNNPACK execution providers.

**Decision**: Start with candle. Add custom SIMD kernels for TurboQuant. Keep the backend trait so we can add burn, mistral.rs, tract, or ort backends later.

### 7.2 Why redb for Local Storage

- **Embedded**: No separate database process (SQLite alternative in pure Rust)
- **ACID**: Full ACID transactions
- **Performance**: Faster than SQLite for key-value patterns
- **Zero dependencies**: Pure Rust, no C bindings
- **Small binary impact**: ~200KB added

### 7.3 Why TOML for Configuration

- **Human-readable**: Easy for developers to edit
- **Typed**: Maps cleanly to Rust structs via serde
- **Rust ecosystem standard**: Cargo uses TOML; familiar to Rust devs
- **Comments**: Supports inline documentation

### 7.4 Async Architecture

```
┌──────────────────────────────────────────────┐
│ tokio runtime (multi-threaded)               │
│                                               │
│  ┌─────────────────┐  ┌──────────────────┐   │
│  │ API tasks        │  │ Background tasks │   │
│  │ (request/resp)   │  │ (optimization,   │   │
│  │                  │  │  health checks)  │   │
│  └─────────────────┘  └──────────────────┘   │
│                                               │
│  ┌──────────────────────────────────────┐     │
│  │ Blocking thread pool (rayon)         │     │
│  │ - Model inference (CPU-bound)        │     │
│  │ - Quantization (CPU-bound)           │     │
│  │ - Embedding generation               │     │
│  └──────────────────────────────────────┘     │
└──────────────────────────────────────────────┘
```

**Key rule**: Never run inference on the tokio async threads. Always dispatch CPU-bound work to `tokio::task::spawn_blocking` or a dedicated rayon thread pool.

---

## 8. Security Architecture

### 8.1 Defense in Depth

```
Internet/Network
       │
       ▼
┌──────────────┐
│ TLS 1.3      │ ← Encryption in transit
├──────────────┤
│ Rate Limiter │ ← DoS protection
├──────────────┤
│ Auth (JWT/   │ ← Identity verification
│  API Key)    │
├──────────────┤
│ AI Shield    │ ← Prompt injection, PII detection
│ Gateway      │
├──────────────┤
│ RBAC         │ ← Authorization (who can do what)
├──────────────┤
│ Input Valid  │ ← Request sanitization
├──────────────┤
│ Model        │ ← Model-level security
│ Sandbox      │   (output filtering, guardrails)
├──────────────┤
│ Audit Log    │ ← Immutable audit trail
└──────────────┘
```

---

## 9. Deployment Topologies

### 9.1 Single Binary (Developer)
```
┌───────────────────┐
│ fuse (single proc) │
│ CLI + API + UI     │
│ CPU inference      │
└───────────────────┘
```

### 9.2 Docker (Team)
```
┌─────────────────────┐
│ Docker container     │
│ fuse serve           │
│ API + GPU (optional) │
│ Volume: models/      │
└─────────────────────┘
```

### 9.3 Kubernetes (Enterprise)
```
┌──────────────────────────────────────────┐
│ Kubernetes Cluster                        │
│                                           │
│  ┌──────────┐  ┌──────────┐             │
│  │ Fuse     │  │ Fuse     │  ... (HPA)  │
│  │ Pod 1    │  │ Pod 2    │             │
│  └────┬─────┘  └────┬─────┘             │
│       └──────┬───────┘                   │
│              ▼                            │
│  ┌────────────────────┐                  │
│  │ Shared Model PVC   │                  │
│  └────────────────────┘                  │
│                                           │
│  ┌────────────────────┐                  │
│  │ Fuse K8s Operator  │                  │
│  │ (CRD management)   │                  │
│  └────────────────────┘                  │
└──────────────────────────────────────────┘
```

### 9.4 Edge Mesh (IoT)
```
┌────────────┐  ┌────────────┐  ┌────────────┐
│ Edge Node 1│  │ Edge Node 2│  │ Edge Node 3│
│ RPi 5      │  │ Jetson     │  │ Phone      │
│ fuse (3B)  │◄─┤ fuse (7B)  │─►│ fuse (1.5B)│
│            │  │            │  │            │
└────────────┘  └────────────┘  └────────────┘
      │               │               │
      └───────────────┼───────────────┘
                      │
              ┌───────▼───────┐
              │ Fuse Mesh     │
              │ (model routing│
              │  & sharding)  │
              └───────────────┘
```

---

*End of Architecture Design Document*
