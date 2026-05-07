# Fuse Development Strategy

## Modular, Reusable, Config-Driven Architecture

### Version: 2.0.0 | Date: 2026-04-04

---

## 1. Core Design Patterns

### 1.1 The Fuse Trait Pattern

Every major subsystem is defined by a trait. This enables:
- **Testability**: Mock any component in unit tests
- **Extensibility**: Add new implementations without changing callers
- **Plugin support**: Third-party code implements Fuse traits
- **Config-driven**: Runtime selects implementation based on config

```rust
// The 7 core traits that define Fuse:

/// 1. Inference — run AI models
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    fn info(&self) -> BackendInfo;
    async fn load_model(&self, path: &Path, config: &ModelConfig) -> Result<ModelHandle>;
    async fn unload_model(&self, handle: &ModelHandle) -> Result<()>;
    async fn infer(&self, handle: &ModelHandle, req: InferenceRequest) -> Result<InferenceResponse>;
    fn stream(&self, handle: &ModelHandle, req: InferenceRequest)
        -> Pin<Box<dyn Stream<Item = Result<Token>> + Send>>;
    async fn embed(&self, handle: &ModelHandle, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn resource_usage(&self) -> ResourceUsage;
}

/// 2. Quantization — compress models
#[async_trait]
pub trait Quantizer: Send + Sync {
    fn name(&self) -> &str;
    fn supported_bits(&self) -> &[u8];
    async fn quantize(&self, model: &Path, config: QuantConfig) -> Result<QuantizedModel>;
    async fn validate(&self, original: &Path, quantized: &Path) -> Result<QualityReport>;
}

/// 3. Channel — communication surfaces
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&mut self, config: &ChannelConfig) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn send_message(&self, session_id: &str, message: &Message) -> Result<()>;
    fn incoming(&self) -> Pin<Box<dyn Stream<Item = IncomingMessage> + Send>>;
}

/// 4. Device — hardware/wearable/IoT connectors
#[async_trait]
pub trait DeviceConnector: Send + Sync {
    fn name(&self) -> &str;
    fn device_type(&self) -> DeviceType;
    async fn connect(&mut self, config: &DeviceConfig) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn read_data(&self, query: &DataQuery) -> Result<DeviceData>;
    fn subscribe(&self) -> Pin<Box<dyn Stream<Item = DeviceEvent> + Send>>;
}

/// 5. Storage — data persistence
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>>;
}

/// 6. Plugin — extensibility
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&mut self, ctx: &PluginContext) -> Result<()>;
    fn on_unload(&mut self) -> Result<()>;
}

/// 7. Skill — reusable capabilities (OpenClaw-inspired)
#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn triggers(&self) -> &[SkillTrigger];
    async fn execute(&self, ctx: &SkillContext, input: &SkillInput) -> Result<SkillOutput>;
}
```

### 1.2 The Config-Driven Pattern

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  fuse.toml   │────▶│ ConfigLoader │────▶│ Validated   │
│  (user edits)│     │ + env expand │     │ FuseConfig  │
└─────────────┘     │ + validate   │     └──────┬──────┘
                    └──────────────┘            │
                                                ▼
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  FileWatcher │────▶│ HotReload    │────▶│ Components  │
│  (notify)    │     │ Coordinator  │     │ .reload()   │
└─────────────┘     └──────────────┘     └─────────────┘
```

Every component implements `Configurable`:

```rust
pub trait Configurable {
    type Config: DeserializeOwned + Validate;
    fn reload(&mut self, config: Self::Config) -> Result<()>;
}
```

### 1.3 The Message Bus Pattern

All components communicate through an async message bus, not direct calls:

```rust
pub enum FuseEvent {
    // Inference events
    InferenceRequest { id: Uuid, model: String, request: InferenceRequest },
    InferenceResponse { id: Uuid, response: InferenceResponse },
    TokenGenerated { id: Uuid, token: Token },
    
    // Channel events
    ChannelMessage { channel: String, session: String, message: Message },
    ChannelResponse { channel: String, session: String, message: Message },
    
    // Device events
    DeviceData { device: String, data: DeviceData },
    DeviceAlert { device: String, alert: Alert },
    
    // System events
    ModelLoaded { model: String },
    ModelUnloaded { model: String },
    ConfigReloaded,
    ShutdownRequested,
}

// Components subscribe to events they care about
pub struct EventBus {
    sender: broadcast::Sender<FuseEvent>,
}
```

### 1.4 The Registry Pattern

Dynamic registration of implementations:

```rust
pub struct InferenceRegistry {
    backends: HashMap<String, Box<dyn InferenceBackend>>,
}

impl InferenceRegistry {
    pub fn register(&mut self, name: &str, backend: Box<dyn InferenceBackend>) { ... }
    pub fn get(&self, name: &str) -> Option<&dyn InferenceBackend> { ... }
    
    /// Build from config — this is how config drives behavior
    pub fn from_config(config: &InferenceConfig) -> Result<Self> {
        let mut registry = Self::new();
        
        // Always register CPU
        registry.register("cpu", Box::new(CpuBackend::new(&config)?));
        
        // Conditionally register GPU
        #[cfg(feature = "cuda")]
        if config.gpu_mode != GpuMode::Cpu {
            if let Ok(cuda) = CudaBackend::new(&config) {
                registry.register("cuda", Box::new(cuda));
            }
        }
        
        // Register remote if configured
        if !config.remote_endpoints.is_empty() {
            registry.register("remote", Box::new(RemoteBackend::new(&config)?));
        }
        
        Ok(registry)
    }
}
```

---

## 2. Dioxus UI Architecture

### Why Dioxus (not Yew)

| Factor | Dioxus | Yew |
|--------|--------|-----|
| Multi-target | Web + Desktop + Mobile + TUI | Web only |
| React-like | Yes (hooks, JSX-like RSX) | Yes but more divergent |
| Server-side | Fullstack support | No |
| Hot reload | Built-in | Limited |
| Bundle size | Smaller | Larger |
| Maintainer | Active, well-funded | Community |

### Dioxus Component Architecture

```
ui/
├── app.rs                    # Root: Router + Theme + Auth context
├── layouts/
│   ├── main_layout.rs        # Sidebar + content area
│   └── minimal_layout.rs     # For embedded widget
├── pages/
│   ├── chat.rs               # Chat interface
│   │   ├── ChatPage          # Page-level state
│   │   ├── MessageList        # Virtual scroll message list
│   │   ├── InputArea          # Multi-line input + file attach
│   │   └── ModelSelector      # Model dropdown
│   ├── models.rs             # Model management
│   │   ├── ModelsPage
│   │   ├── ModelCard          # Model info card
│   │   ├── PullDialog         # Model download dialog
│   │   └── QuantizeDialog     # Quantization options
│   ├── dashboard.rs          # System dashboard
│   │   ├── DashboardPage
│   │   ├── ResourceChart      # CPU/RAM/GPU charts
│   │   ├── ModelStatus        # Loaded models table
│   │   └── QueueStatus        # Request queue
│   ├── channels.rs           # Channel management
│   └── devices.rs            # Device hub
├── components/               # Reusable
│   ├── markdown.rs           # Markdown renderer
│   ├── code_block.rs         # Syntax highlighted code
│   ├── progress_bar.rs       # Progress indicator
│   ├── toast.rs              # Notification toasts
│   └── chart.rs              # Real-time charts
└── hooks/                    # Custom hooks
    ├── use_websocket.rs      # WebSocket connection
    ├── use_api.rs            # REST API client
    ├── use_theme.rs          # Theme management
    └── use_streaming.rs      # SSE streaming
```

### WASM Deployment Strategy

```
Target 1: Web App (fuse serve --ui)
├── Full Dioxus app compiled to WASM
├── Served by Fuse's axum server
├── Communicates via WebSocket to Fuse API
└── Progressive Web App (installable)

Target 2: Desktop App (fuse desktop)
├── Dioxus desktop (webview-based)
├── Direct Rust calls (no network needed)
└── Native file dialogs, system tray

Target 3: Embeddable Widget
├── Minimal Dioxus component → WASM
├── Single <script> tag embedding
├── Connects to remote Fuse instance
└── <50KB gzipped

Target 4: TUI (fuse tui)
├── Dioxus TUI renderer (or ratatui)
├── For SSH / headless servers
└── Full chat + model management
```

---

## 3. Kubernetes-Native Design

### Custom Resource Definitions

```yaml
# FuseModel CRD — declare desired model state
apiVersion: fuse.ai/v1alpha1
kind: FuseModel
metadata:
  name: deepseek-r1-7b
  namespace: fuse-system
spec:
  source: "huggingface://deepseek-ai/deepseek-r1:7b"
  quantization:
    method: auto
    quality_threshold: 0.95
  replicas: 2
  resources:
    memory: "8Gi"
    cpu: "4"
  autoscale:
    min: 1
    max: 5
    metric: queue_depth
    target: 10

---
# FuseChannel CRD — declare channel
apiVersion: fuse.ai/v1alpha1
kind: FuseChannel
metadata:
  name: telegram-bot
spec:
  type: telegram
  model: deepseek-r1-7b
  secretRef:
    name: telegram-credentials
  config:
    max_history: 50
    system_prompt: "You are a helpful assistant."

---
# FuseGateway CRD — AI Shield configuration
apiVersion: fuse.ai/v1alpha1
kind: FuseGateway
metadata:
  name: ai-shield
spec:
  rules:
    - name: prompt-injection
      action: block
      sensitivity: high
    - name: pii-detection
      action: redact
      patterns: [email, phone, ssn]
```

### Operator Architecture

```
┌─────────────────────────────────────┐
│ Fuse K8s Operator                    │
│                                      │
│  ┌──────────────┐                    │
│  │ Model        │─── Watch FuseModel │
│  │ Controller   │    → Pull, quantize│
│  │              │    → Create Pods    │
│  └──────────────┘                    │
│                                      │
│  ┌──────────────┐                    │
│  │ Channel      │─── Watch FuseChannel
│  │ Controller   │    → Start bots    │
│  └──────────────┘                    │
│                                      │
│  ┌──────────────┐                    │
│  │ Gateway      │─── Watch FuseGateway
│  │ Controller   │    → Config shield │
│  └──────────────┘                    │
│                                      │
│  ┌──────────────┐                    │
│  │ Autoscaler   │─── Monitor metrics │
│  │              │    → Scale pods    │
│  └──────────────┘                    │
└─────────────────────────────────────┘
```

---

## 4. Memory Management Strategy

### 4.1 Zero-Copy Model Loading

```rust
// Models are memory-mapped, not read into heap
use memmap2::MmapOptions;

pub struct MappedModel {
    mmap: memmap2::Mmap,
    metadata: ModelMetadata,
}

impl MappedModel {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        // SAFETY: File is read-only, we hold the file handle
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let metadata = parse_gguf_header(&mmap)?;
        Ok(Self { mmap, metadata })
    }
    
    pub fn tensor_data(&self, offset: usize, len: usize) -> &[u8] {
        &self.mmap[offset..offset + len]
    }
}
```

### 4.2 KV-Cache with Paged Attention

```rust
pub struct PagedKvCache {
    pages: Vec<KvPage>,           // Fixed-size pages
    page_table: HashMap<usize, Vec<usize>>,  // sequence → page indices
    free_pages: VecDeque<usize>,  // Free page pool
    page_size: usize,             // Tokens per page (typically 16)
}

impl PagedKvCache {
    /// Allocate pages for a new sequence
    pub fn allocate(&mut self, seq_id: usize, num_tokens: usize) -> Result<()> {
        let pages_needed = (num_tokens + self.page_size - 1) / self.page_size;
        if self.free_pages.len() < pages_needed {
            return Err(FuseError::OutOfMemory("KV-cache full".into()));
        }
        let pages: Vec<usize> = (0..pages_needed)
            .map(|_| self.free_pages.pop_front().unwrap())
            .collect();
        self.page_table.insert(seq_id, pages);
        Ok(())
    }
    
    /// Free pages when sequence completes
    pub fn free(&mut self, seq_id: usize) {
        if let Some(pages) = self.page_table.remove(&seq_id) {
            self.free_pages.extend(pages);
        }
    }
}
```

### 4.3 Resource Budgets

```rust
pub struct ResourceBudget {
    max_memory: usize,          // Hard memory limit
    max_loaded_models: usize,   // Max concurrent models
    max_kv_cache_pages: usize,  // Max KV-cache pages
}

impl ResourceBudget {
    pub fn from_system(hw: &HardwareProfile, config: &ResourceConfig) -> Self {
        let max_memory = config.max_memory
            .unwrap_or_else(|| hw.ram_available * 80 / 100);  // Default: 80% of available
        
        Self {
            max_memory,
            max_loaded_models: config.max_loaded_models.unwrap_or(3),
            max_kv_cache_pages: max_memory / (2 * PAGE_SIZE_BYTES),  // Half for KV
        }
    }
}
```

---

## 5. Channel Bridge Architecture

### 5.1 Message Flow

```
Telegram/Discord/Slack/Web/...
        │
        ▼
┌──────────────────┐
│ Channel Adapter   │  ── Converts platform-specific message to FuseMessage
│ (impl Channel)    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Session Manager   │  ── Manages per-user conversation state
│                   │     Loads history, manages context window
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Channel Router    │  ── Routes to configured model
│                   │     Applies channel-specific system prompt
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ AI Shield        │  ── Optional: prompt injection, PII detection
│ (if enabled)     │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Inference Engine  │  ── Runs inference, streams tokens
│                   │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Channel Adapter   │  ── Converts response back to platform format
│ .send_message()   │     Handles markdown→platform formatting
└──────────────────┘
```

### 5.2 Session Persistence

```rust
pub struct SessionManager {
    store: Arc<dyn StorageBackend>,
    sessions: DashMap<SessionKey, ConversationSession>,
    config: SessionConfig,
}

#[derive(Clone)]
pub struct SessionKey {
    pub channel: String,    // "telegram", "discord", etc.
    pub user_id: String,    // Platform-specific user ID
}

pub struct ConversationSession {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub system_prompt: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}
```

---

## 6. Build & Release Strategy

### Binary Targets

```
fuse                    # Main binary (CLI + server + all features)
fuse-edge               # Minimal binary (CLI + CPU inference only)
fuse-operator           # K8s operator binary
fuse-ui                 # Standalone UI (Dioxus desktop)
fuse-chat.js            # Embeddable WASM widget
```

### CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    steps:
      - cargo fmt --check
      - cargo clippy -- -D warnings
      - cargo test --all-features
      - cargo build --release --target ${{ matrix.target }}

  coverage:
    steps:
      - cargo tarpaulin --out xml
      - assert coverage >= 85%

  benchmark:
    steps:
      - cargo bench
      - compare with main branch
      - fail if regression > 5%

  wasm:
    steps:
      - cargo build --target wasm32-unknown-unknown --features dioxus-ui

  edge:
    steps:
      - cross build --target aarch64-unknown-linux-gnu --features edge --no-default-features
      - assert binary_size < 10MB

  release:
    if: tag
    steps:
      - build all targets
      - create GitHub release with binaries
      - publish to crates.io
      - build and push Docker images
      - update Homebrew formula
```

---

*End of Development Strategy*
