# Intelligent Resource Management

Fuse includes an advanced resource management system that automatically optimizes VRAM, CPU, and GPU usage for idle models, ensuring efficient resource utilization.

## Overview

The Resource Manager monitors all loaded models and automatically:
- **Detects idle models** (no active requests for configurable timeout)
- **Optimizes memory usage** by compressing model data
- **Offloads to CPU** to free GPU/VRAM when idle
- **Auto-unloads** least recently used models when limits are exceeded
- **Tracks resource usage** (memory, CPU, GPU) per model

## Key Features

### 1. Automatic Idle Detection
Models that haven't been used for a configurable period (default: 5 minutes) are automatically marked as idle.

### 2. Memory Optimization
Idle models can be optimized to use ~20-30% less memory through:
- Memory compression
- Reduced precision for cached data
- Cleanup of temporary buffers

### 3. GPU Offloading
When a model is idle, it can be automatically offloaded from GPU to CPU, freeing up:
- **VRAM** (GPU memory)
- **GPU compute resources**
- Typically reduces memory usage by ~30%

### 4. Automatic Unloading
When resource limits are exceeded, the system automatically unloads least recently used models to stay within configured limits.

### 5. Resource Tracking
Real-time monitoring of:
- Memory usage (bytes)
- CPU usage (percentage)
- GPU usage (percentage)
- Active request count
- Last access time

## Configuration

### Resource Policy

Configure resource management in your `config.toml`:

```toml
[resource_management]
# Time before considering a model idle (seconds)
idle_timeout = 300  # 5 minutes

# Maximum total memory usage (bytes)
max_memory_bytes = 8589934592  # 8GB

# Maximum number of models to keep loaded
max_loaded_models = 3

# Enable automatic unloading of idle models
auto_unload_idle = true

# Enable memory optimization for idle models
optimize_idle_memory = true

# Enable GPU offloading for idle models
offload_to_cpu = true
```

### Programmatic Configuration

```rust
use fuse::model::{ResourcePolicy, LocalInferenceEngine};
use std::time::Duration;

let policy = ResourcePolicy {
    idle_timeout: Duration::from_secs(300),
    max_memory_bytes: 8 * 1024 * 1024 * 1024, // 8GB
    max_loaded_models: 3,
    auto_unload_idle: true,
    optimize_idle_memory: true,
    offload_to_cpu: true,
};

let engine = LocalInferenceEngine::with_resource_policy(
    model_repo,
    models_dir,
    policy,
);
```

## Model States

The resource manager tracks models through different states:

### Active
- Model is currently processing requests
- Full resources allocated (GPU/VRAM)
- No optimization applied

### Idle
- No active requests
- Within idle timeout period
- Full resources still allocated
- Candidate for optimization

### Optimized
- Idle timeout exceeded
- Memory optimized (~20% reduction)
- Still on GPU but compressed
- Can be quickly reactivated

### OffloadedToCpu
- Idle timeout exceeded
- Moved from GPU to CPU
- ~30% memory reduction
- VRAM freed for other models
- Slower reactivation (needs GPU reload)

### Unloaded
- Completely removed from memory
- All resources freed
- Requires full reload to use

## State Transitions

```
Active ──┐
         │
         ├──> Idle ──┐
         │           │
         │           ├──> Optimized ──┐
         │           │                │
         │           └──> OffloadedToCpu ──┐
         │                                 │
         └──────────────────────────────> Unloaded
```

## API Usage

### Check Resource Usage

```rust
// Get stats for a specific model
let stats = engine.resource_manager().get_stats("gpt2");
println!("Memory: {} MB", stats.memory_bytes / (1024 * 1024));
println!("CPU: {:.1}%", stats.cpu_percent);
println!("GPU: {:.1}%", stats.gpu_percent);
println!("Active requests: {}", stats.active_requests);

// Get total memory usage
let total_memory = engine.resource_manager().total_memory_usage();
println!("Total memory: {} GB", total_memory / (1024 * 1024 * 1024));

// Get loaded model count
let count = engine.resource_manager().loaded_model_count();
println!("Loaded models: {}", count);
```

### Manual Optimization

```rust
// Trigger manual optimization of idle models
engine.resource_manager().trigger_optimization().await?;

// Get list of idle models
let idle_models = engine.resource_manager().get_idle_models();
println!("Idle models: {:?}", idle_models);

// Optimize specific idle models
let optimized = engine.resource_manager().optimize_idle_models().await?;
println!("Optimized {} models", optimized.len());

// Enforce resource limits
let unloaded = engine.resource_manager().enforce_limits().await?;
println!("Unloaded {} models to free resources", unloaded.len());
```

### Monitoring

```rust
// Check if over resource limits
if engine.resource_manager().is_over_limit() {
    println!("⚠️  Resource limits exceeded!");
    
    // Automatically handled by the system, but you can also
    // manually trigger cleanup
    engine.resource_manager().enforce_limits().await?;
}
```

## Performance Impact

### Memory Savings
- **Idle Optimization**: 15-20% memory reduction
- **CPU Offloading**: 25-35% memory reduction
- **Auto-unloading**: 100% memory freed

### Reactivation Time
- **Active → Idle**: Instant (no change)
- **Idle → Optimized**: ~50-100ms to decompress
- **Optimized → Active**: ~50-100ms
- **OffloadedToCpu → Active**: ~500ms-2s (GPU reload)
- **Unloaded → Active**: ~5-30s (full model load)

### Recommendations

1. **For frequently used models**: Set longer `idle_timeout` (10-15 minutes)
2. **For memory-constrained systems**: Enable `offload_to_cpu` and `auto_unload_idle`
3. **For GPU-heavy workloads**: Enable `offload_to_cpu` to free VRAM
4. **For low-latency requirements**: Disable `offload_to_cpu`, keep `optimize_idle_memory` only

## Monitoring Dashboard

Access real-time resource metrics via API:

```bash
# Get resource stats for all models
curl http://localhost:8080/api/v1/resources

# Get stats for specific model
curl http://localhost:8080/api/v1/resources/gpt2

# Trigger manual optimization
curl -X POST http://localhost:8080/api/v1/resources/optimize
```

## Best Practices

### 1. Set Appropriate Limits
```toml
# For 16GB RAM system
max_memory_bytes = 12884901888  # 12GB (leave 4GB for OS)

# For 8GB VRAM GPU
max_memory_bytes = 6442450944   # 6GB (leave 2GB buffer)
```

### 2. Tune Idle Timeout
```toml
# Development (frequent model switching)
idle_timeout = 600  # 10 minutes

# Production (stable workload)
idle_timeout = 300  # 5 minutes

# High-traffic (keep models hot)
idle_timeout = 1800  # 30 minutes
```

### 3. Balance Model Count
```toml
# Small models (< 2GB each)
max_loaded_models = 5

# Medium models (2-4GB each)
max_loaded_models = 3

# Large models (> 4GB each)
max_loaded_models = 2
```

### 4. Monitor and Adjust
```bash
# Watch resource usage
watch -n 5 'curl -s http://localhost:8080/api/v1/resources | jq'

# Check logs for optimization events
tail -f ~/.fuse/logs/fuse.log | grep -i "resource\|optimize\|offload"
```

## Troubleshooting

### Models Being Unloaded Too Frequently
**Solution**: Increase `idle_timeout` or `max_loaded_models`

### High Memory Usage
**Solution**: 
- Decrease `max_memory_bytes`
- Enable `auto_unload_idle`
- Decrease `max_loaded_models`

### Slow Model Reactivation
**Solution**: 
- Disable `offload_to_cpu`
- Increase `idle_timeout`
- Keep frequently used models in `Active` state

### GPU Memory Exhaustion
**Solution**:
- Enable `offload_to_cpu`
- Decrease `max_loaded_models`
- Set lower `max_memory_bytes`

## Advanced Configuration

### Custom Resource Policy Per Model

```rust
// Different policies for different model types
let small_model_policy = ResourcePolicy {
    idle_timeout: Duration::from_secs(600),  // Keep longer
    offload_to_cpu: false,  // Keep on GPU
    ..Default::default()
};

let large_model_policy = ResourcePolicy {
    idle_timeout: Duration::from_secs(180),  // Aggressive
    offload_to_cpu: true,  // Free VRAM quickly
    auto_unload_idle: true,
    ..Default::default()
};
```

### Integration with Monitoring Systems

```rust
// Export metrics to Prometheus
use prometheus::{register_gauge, Gauge};

let memory_gauge = register_gauge!(
    "fuse_model_memory_bytes",
    "Memory usage per model"
).unwrap();

// Update metrics periodically
tokio::spawn(async move {
    loop {
        let total = engine.resource_manager().total_memory_usage();
        memory_gauge.set(total as f64);
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
});
```

## Future Enhancements

- [ ] Predictive preloading based on usage patterns
- [ ] Dynamic policy adjustment based on system load
- [ ] Multi-GPU load balancing
- [ ] Distributed resource management across nodes
- [ ] ML-based optimization strategies
- [ ] Integration with Kubernetes resource limits

---

For more information, see:
- [Configuration Guide](CONFIGURATION.md)
- [Performance Tuning](PERFORMANCE.md)
- [API Reference](API.md)
