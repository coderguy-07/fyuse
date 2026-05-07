# Fuse Kubernetes & Scalability Guide

## Version: 1.0.0
## Status: Draft

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Kubernetes Deployment](#2-kubernetes-deployment)
3. [GPU Sharing & Scheduling](#3-gpu-sharing--scheduling)
4. [Scaling Strategies](#4-scaling-strategies)
5. [Observability](#5-observability)
6. [Multi-Tenancy](#6-multi-tenancy)

---

## 1. Architecture Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLIENT LAYER                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ CLI      │  │ Web UI   │  │ IDE Ext  │  │ API      │  │ MCP      │       │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘       │
└───────┼─────────────┼─────────────┼─────────────┼─────────────┼─────────────┘
        │             │             │             │             │
        └─────────────┴─────────────┴─────────────┴─────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              GATEWAY LAYER                                   │
│                         (Ingress Controller)                                 │
│                    SSL Termination, Rate Limiting                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CONTROL PLANE                                   │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                    Fuse Operator (Kubernetes Operator)                   │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │ │
│  │  │ Model        │  │ Inference    │  │ Resource     │  │ Workflow    │ │ │
│  │  │ Controller   │  │ Controller   │  │ Controller   │  │ Controller  │ │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────┘ │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              DATA PLANE                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                    Model Pods (StatefulSets)                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │ │
│  │  │ Model A     │  │ Model B     │  │ Model C     │  │ Model D     │    │ │
│  │  │ (GPU)       │  │ (GPU)       │  │ (GPU)       │  │ (CPU)       │    │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              STORAGE LAYER                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Model Cache  │  │ Chat History │  │ Config Store │  │ Metrics TSDB │     │
│  │ (PV/PVC)     │  │ (PostgreSQL) │  │ (etcd)       │  │ (Prometheus) │     │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Component Responsibilities

| Component | Responsibility | Scaling |
|-----------|---------------|---------|
| API Gateway | Request routing, auth, rate limiting | Horizontal (replicas) |
| Fuse Operator | CRD management, scheduling decisions | Single instance (HA) |
| Model Pods | Model inference execution | Horizontal (GPU nodes) |
| Cache Layer | Model storage, embedding cache | Vertical (memory) |
| Database | Chat history, metadata | Horizontal (sharding) |

---

## 2. Kubernetes Deployment

### 2.1 Custom Resource Definitions (CRDs)

```yaml
# FuseModel - Represents an AI model deployment
apiVersion: fuse.ai/v1
kind: FuseModel
metadata:
  name: llama-3-70b
  namespace: fuse-production
spec:
  model:
    source: huggingface
    repository: meta-llama/Meta-Llama-3-70B-Instruct
    revision: main
    auth:
      secretRef: hf-token
  
  quantization:
    enabled: true
    method: gguf
    format: Q4_K_M
  
  inference:
    maxTokens: 4096
    temperature: 0.7
    contextWindow: 8192
  
  resources:
    gpu:
      count: 2
      memory: "40Gi"
      type: nvidia-a100
    cpu: "8"
    memory: "64Gi"
  
  scaling:
    minReplicas: 1
    maxReplicas: 10
    targetLatency: 100ms
    targetUtilization: 80
  
  scheduling:
    nodeSelector:
      node-type: gpu
    tolerations:
      - key: nvidia.com/gpu
        operator: Exists
        effect: NoSchedule
---
# FuseWorkspace - Multi-tenant workspace
apiVersion: fuse.ai/v1
kind: FuseWorkspace
metadata:
  name: team-alpha
spec:
  models:
    - name: llama-3-70b
      quota:
        requestsPerMinute: 1000
        tokensPerDay: 10000000
  
  resources:
    maxGPUs: 4
    maxMemory: "256Gi"
  
  auth:
    type: oauth
    provider: keycloak
    clientId: fuse-team-alpha
---
# FuseInference - Inference request routing
apiVersion: fuse.ai/v1
kind: FuseInference
metadata:
  name: production-router
spec:
  routing:
    strategy: weighted
    models:
      - name: llama-3-70b
        weight: 70
      - name: claude-3-opus
        weight: 30
        type: remote
        endpoint: https://api.anthropic.com
  
  caching:
    enabled: true
    ttl: 3600
    similarPromptCache: true
  
  fallback:
    enabled: true
    onError:
      - retry
      - fallback-model
      - queue
```

### 2.2 Operator Architecture

```rust
// Fuse Operator - Main reconciliation loop
#[derive(Clone)]
pub struct FuseOperator {
    client: Client,
    model_controller: Arc<ModelController>,
    scheduler: Arc<Scheduler>,
    metrics: Arc<MetricsCollector>,
}

#[async_trait]
impl Controller for FuseOperator {
    type Resource = FuseModel;
    type Error = OperatorError;
    
    async fn reconcile(&self, model: FuseModel) -> Result<Action, Self::Error> {
        let ctx = ReconcileContext::from(&model);
        
        // 1. Ensure model is cached
        self.ensure_model_cached(&model).await?;
        
        // 2. Calculate resource requirements
        let resources = self.calculate_resources(&model).await?;
        
        // 3. Schedule pods on appropriate nodes
        let pods = self.scheduler.schedule(&model, &resources).await?;
        
        // 4. Update service endpoints
        self.update_service(&model, &pods).await?;
        
        // 5. Configure autoscaler
        self.configure_autoscaler(&model).await?;
        
        // 6. Update status
        self.update_status(&model, &pods).await?;
        
        Ok(Action::requeue(Duration::from_secs(30)))
    }
    
    async fn cleanup(&self, model: FuseModel) -> Result<Action, Self::Error> {
        // Graceful shutdown sequence
        self.drain_pods(&model).await?;
        self.release_resources(&model).await?;
        self.cleanup_cache(&model).await?;
        
        Ok(Action::await_change())
    }
}
```

### 2.3 Deployment Manifests

```yaml
# Namespace with resource quotas
apiVersion: v1
kind: Namespace
metadata:
  name: fuse-production
  labels:
    app.kubernetes.io/name: fuse
    app.kubernetes.io/component: platform
---
apiVersion: v1
kind: ResourceQuota
metadata:
  name: fuse-quota
  namespace: fuse-production
spec:
  hard:
    requests.nvidia.com/gpu: 16
    limits.nvidia.com/gpu: 16
    requests.memory: 512Gi
    limits.memory: 512Gi
    requests.cpu: "64"
    limits.cpu: "64"
    persistentvolumeclaims: "10"
---
# Fuse API Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: fuse-api
  namespace: fuse-production
spec:
  replicas: 3
  selector:
    matchLabels:
      app: fuse-api
  template:
    metadata:
      labels:
        app: fuse-api
    spec:
      serviceAccountName: fuse-operator
      containers:
      - name: api
        image: fuse/fuse:latest
        command: ["fuse", "serve", "--mode", "api"]
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        - name: FUSE_CONFIG_PATH
          value: "/config/config.toml"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: fuse-secrets
              key: redis-url
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        volumeMounts:
        - name: config
          mountPath: /config
        - name: models
          mountPath: /models
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: fuse-config
      - name: models
        persistentVolumeClaim:
          claimName: fuse-models-cache
---
# Fuse Operator Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: fuse-operator
  namespace: fuse-system
spec:
  replicas: 1
  selector:
    matchLabels:
      app: fuse-operator
  template:
    metadata:
      labels:
        app: fuse-operator
    spec:
      serviceAccountName: fuse-operator
      containers:
      - name: operator
        image: fuse/operator:latest
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        env:
        - name: WATCH_NAMESPACE
          value: "fuse-production"
---
# RBAC for Fuse Operator
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: fuse-operator
rules:
- apiGroups: ["fuse.ai"]
  resources: ["fusemodels", "fuseworkspaces", "fuseinferences"]
  verbs: ["*"]
- apiGroups: [""]
  resources: ["pods", "services", "configmaps", "persistentvolumeclaims"]
  verbs: ["*"]
- apiGroups: ["apps"]
  resources: ["statefulsets", "deployments"]
  verbs: ["*"]
- apiGroups: ["autoscaling"]
  resources: ["horizontalpodautoscalers"]
  verbs: ["*"]
- apiGroups: ["nvidia.com"]
  resources: ["gpu"]
  verbs: ["get", "list", "watch"]
```

---

## 3. GPU Sharing & Scheduling

### 3.1 GPU Sharing Strategies

| Strategy | Use Case | Implementation |
|----------|----------|----------------|
| Time Slicing | Multiple small models | NVIDIA Time-Slicing |
| MIG (A100/H100) | Memory isolation | NVIDIA MIG |
| MPS | Compute sharing | CUDA MPS |
| vGPU | VM-level sharing | NVIDIA vGPU |

```yaml
# Time Slicing Configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: time-slicing-config
  namespace: nvidia-device-plugin
data:
  any: |-
    version: v1
    sharing:
      timeSlicing:
        renameByDefault: false
        resources:
        - name: nvidia.com/gpu
          replicas: 4  # 4 virtual GPUs per physical GPU
---
# MIG Configuration for A100
apiVersion: v1
kind: ConfigMap
metadata:
  name: mig-config
  namespace: nvidia-device-plugin
data:
  a100-40gb: |-
    version: v1
    sharing:
      mig:
        strategy: mixed
        resources:
        - name: nvidia.com/gpu
          partitions:
            - name: 1g.5gb
              count: 7
            - name: 2g.10gb
              count: 3
            - name: 3g.20gb
              count: 2
```

### 3.2 Custom GPU Scheduler

```rust
pub struct GpuScheduler {
    node_cache: Arc<RwLock<HashMap<String, NodeInfo>>>,
    pod_queue: PriorityQueue<PodSpec>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    name: String,
    gpus: Vec<GpuInfo>,
    memory_available: u64,
    cpu_available: u64,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    id: String,
    model: GpuModel,
    memory_total: u64,
    memory_used: u64,
    mig_enabled: bool,
    mig_instances: Vec<MigInstance>,
}

impl GpuScheduler {
    pub async fn schedule(&self, pod: &PodSpec) -> Result<SchedulingDecision, Error> {
        let requirements = self.extract_gpu_requirements(pod)?;
        let nodes = self.node_cache.read().await;
        
        // Score all feasible nodes
        let scored: Vec<_> = nodes
            .values()
            .filter(|n| self.meets_requirements(n, &requirements))
            .map(|n| (n, self.score_node(n, &requirements)))
            .collect();
        
        // Pick best node
        let best = scored
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .ok_or(Error::NoSuitableNode)?;
        
        Ok(SchedulingDecision {
            node_name: best.0.name.clone(),
            gpus: self.allocate_gpus(best.0, &requirements)?,
        })
    }
    
    fn score_node(&self, node: &NodeInfo, req: &GpuRequirements) -> f64 {
        let mut score = 0.0;
        
        // Prefer nodes with matching GPU model
        if node.gpus.iter().any(|g| g.model == req.model) {
            score += 100.0;
        }
        
        // Prefer nodes with more available GPUs (for bin packing)
        let available_gpus = node.gpus.iter()
            .filter(|g| g.memory_used == 0)
            .count();
        score += available_gpus as f64 * 10.0;
        
        // Prefer nodes with existing model cache
        if self.has_model_cached(node, &req.model_name) {
            score += 50.0;
        }
        
        // Prefer nodes with lower latency to client
        score -= self.network_latency(node) * 5.0;
        
        score
    }
}
```

### 3.3 GPU Metrics Collection

```rust
pub struct GpuMetricsCollector {
    nvml: Nvml,
    metrics_tx: mpsc::Sender<GpuMetrics>,
}

#[derive(Debug)]
pub struct GpuMetrics {
    device_id: String,
    timestamp: DateTime<Utc>,
    utilization: GpuUtilization,
    memory: MemoryInfo,
    temperature: u32,
    power: PowerInfo,
    processes: Vec<ProcessInfo>,
}

impl GpuMetricsCollector {
    pub async fn collect(&self) -> Result<(), Error> {
        let device_count = self.nvml.device_count()?;
        
        for i in 0..device_count {
            let device = self.nvml.device_by_index(i)?;
            
            let metrics = GpuMetrics {
                device_id: device.uuid()?,
                timestamp: Utc::now(),
                utilization: device.utilization_rates()?.into(),
                memory: device.memory_info()?.into(),
                temperature: device.temperature(TemperatureSensor::Gpu)?,
                power: device.power_usage()?.into(),
                processes: device.running_compute_processes()?
                    .into_iter()
                    .map(|p| p.into())
                    .collect(),
            };
            
            self.metrics_tx.send(metrics).await?;
        }
        
        Ok(())
    }
}
```

---

## 4. Scaling Strategies

### 4.1 Horizontal Pod Autoscaler (HPA)

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: fuse-model-llama3
  namespace: fuse-production
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: fuse-model-llama3
  minReplicas: 1
  maxReplicas: 10
  metrics:
  - type: Pods
    pods:
      metric:
        name: fuse_inference_latency_p99
      target:
        type: AverageValue
        averageValue: 100m  # 100ms
  - type: Resource
    resource:
      name: nvidia.com/gpu
      target:
        type: Utilization
        averageUtilization: 80
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Pods
        value: 2
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Pods
        value: 1
        periodSeconds: 120
```

### 4.2 Custom Metrics Adapter

```rust
pub struct FuseMetricsAdapter {
    collector: Arc<MetricsCollector>,
}

#[async_trait]
impl CustomMetricsApi for FuseMetricsAdapter {
    async fn get_metric(&self, metric_name: &str, selector: &LabelSelector) -> Result<MetricValue, Error> {
        match metric_name {
            "fuse_inference_latency_p99" => {
                let latency = self.collector
                    .query_p99_latency(selector)
                    .await?;
                
                Ok(MetricValue {
                    value: (latency.as_millis() as i64).into(),
                    timestamp: Utc::now(),
                })
            }
            "fuse_queue_depth" => {
                let depth = self.collector
                    .query_queue_depth(selector)
                    .await?;
                
                Ok(MetricValue {
                    value: depth.into(),
                    timestamp: Utc::now(),
                })
            }
            "fuse_gpu_memory_utilization" => {
                let utilization = self.collector
                    .query_gpu_memory_utilization(selector)
                    .await?;
                
                Ok(MetricValue {
                    value: utilization.into(),
                    timestamp: Utc::now(),
                })
            }
            _ => Err(Error::UnknownMetric),
        }
    }
}
```

### 4.3 Predictive Scaling

```rust
pub struct PredictiveScaler {
    model: Arc<dyn PredictionModel>,
    metrics: Arc<MetricsHistory>,
}

#[async_trait]
pub trait PredictionModel: Send + Sync {
    async fn predict(&self, history: &MetricsHistory, horizon: Duration) -> Prediction;
}

impl PredictiveScaler {
    pub async fn get_recommended_scale(&self) -> Result<ScaleRecommendation, Error> {
        // Get historical metrics
        let history = self.metrics.get_history(Duration::hours(24)).await?;
        
        // Predict load for next 15 minutes
        let prediction = self.model.predict(&history, Duration::minutes(15)).await;
        
        // Calculate required capacity
        let current_replicas = self.get_current_replicas().await?;
        let required_replicas = self.calculate_required_capacity(&prediction);
        
        // Pre-warm by scaling up 5 minutes early
        let prewarm_time = Duration::minutes(5);
        let prewarm_prediction = self.model.predict(&history, prewarm_time).await;
        
        Ok(ScaleRecommendation {
            current: current_replicas,
            target: required_replicas,
            prewarm_at: if required_replicas > current_replicas {
                Some(Utc::now() + (horizon - prewarm_time))
            } else {
                None
            },
        })
    }
}
```

---

## 5. Observability

### 5.1 Metrics

```yaml
# Prometheus ServiceMonitor
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: fuse-metrics
  namespace: fuse-production
spec:
  selector:
    matchLabels:
      app: fuse-api
  endpoints:
  - port: metrics
    path: /metrics
    interval: 15s
    scrapeTimeout: 10s
```

```rust
// Custom metrics
lazy_static! {
    static ref INFERENCE_LATENCY: HistogramVec = register_histogram_vec!(
        "fuse_inference_latency_seconds",
        "Inference latency in seconds",
        &["model", "status"],
        vec![0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).unwrap();
    
    static ref GPU_UTILIZATION: GaugeVec = register_gauge_vec!(
        "fuse_gpu_utilization_percent",
        "GPU utilization percentage",
        &["device", "model"]
    ).unwrap();
    
    static ref QUEUE_DEPTH: GaugeVec = register_gauge_vec!(
        "fuse_queue_depth",
        "Current queue depth",
        &["model", "priority"]
    ).unwrap();
    
    static ref MODEL_LOAD_TIME: HistogramVec = register_histogram_vec!(
        "fuse_model_load_seconds",
        "Time to load a model",
        &["model", "source"],
        vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
    ).unwrap();
}
```

### 5.2 Distributed Tracing

```rust
pub async fn inference_with_tracing(
    request: InferenceRequest,
) -> Result<InferenceResponse, Error> {
    let span = info_span!(
        "inference",
        model = %request.model_name,
        request_id = %Uuid::new_v4(),
        otel.kind = "server",
    );
    
    async {
        // Record attributes
        span.record("input_tokens", request.input_tokens());
        
        // Load model
        let load_span = info_span!("model_load", otel.kind = "internal").entered();
        let model = load_model(&request.model_name).await?;
        drop(load_span);
        
        // Run inference
        let infer_span = info_span!("model_inference", otel.kind = "internal").entered();
        let output = model.infer(request.input).await?;
        drop(infer_span);
        
        // Record output metrics
        span.record("output_tokens", output.tokens.len());
        
        Ok(output)
    }
    .instrument(span)
    .await
}
```

### 5.3 Logging

```yaml
# Fluent Bit configuration for log aggregation
apiVersion: v1
kind: ConfigMap
metadata:
  name: fluent-bit-config
data:
  fluent-bit.conf: |
    [INPUT]
        Name kubernetes
        Tag kube.*
        Kube_URL https://kubernetes.default.svc:443
    
    [FILTER]
        Name kubernetes
        Match kube.*
        Merge_Log On
        Keep_Log Off
    
    [FILTER]
        Name grep
        Match kube.*
        Regex kubernetes.labels.app fuse
    
    [OUTPUT]
        Name loki
        Match kube.*
        Host loki.monitoring.svc.cluster.local
        Port 3100
        Labels job=fuse
```

---

## 6. Multi-Tenancy

### 6.1 Namespace Isolation

```yaml
# Per-tenant namespace
apiVersion: v1
kind: Namespace
metadata:
  name: fuse-tenant-acme
  labels:
    fuse.ai/tenant: acme
    fuse.ai/tier: enterprise
spec:
  finalizers:
  - kubernetes
---
# Network policies for tenant isolation
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: tenant-isolation
  namespace: fuse-tenant-acme
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          fuse.ai/tenant: acme
    - namespaceSelector:
        matchLabels:
          name: fuse-system
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          fuse.ai/tenant: acme
    - namespaceSelector:
        matchLabels:
          name: fuse-system
```

### 6.2 Resource Quotas per Tenant

```yaml
apiVersion: v1
kind: ResourceQuota
metadata:
  name: fuse-quota-acme
  namespace: fuse-tenant-acme
spec:
  hard:
    requests.nvidia.com/gpu: 8
    limits.nvidia.com/gpu: 8
    requests.memory: 256Gi
    limits.memory: 256Gi
    fuse.ai/models: "10"
    fuse.ai/inferences: "100"
---
apiVersion: v1
kind: LimitRange
metadata:
  name: fuse-limits-acme
  namespace: fuse-tenant-acme
spec:
  limits:
  - max:
      nvidia.com/gpu: 2
      memory: 64Gi
      cpu: "16"
    min:
      memory: 1Gi
      cpu: "100m"
    default:
      memory: 4Gi
      cpu: "1000m"
    type: Container
```

### 6.3 Fair Scheduling

```rust
pub struct FairScheduler {
    tenants: DashMap<String, TenantState>,
    fairness_window: Duration,
}

#[derive(Debug)]
pub struct TenantState {
    name: String,
    quota: ResourceQuota,
    usage: ResourceUsage,
    requests: VecDeque<ScheduleRequest>,
}

impl FairScheduler {
    pub async fn schedule(&self, request: ScheduleRequest) -> Result<Assignment, Error> {
        let tenant = self.tenants
            .get(&request.tenant_id)
            .ok_or(Error::UnknownTenant)?;
        
        // Check quota
        if !tenant.has_quota(&request.resources) {
            return Err(Error::QuotaExceeded);
        }
        
        // Calculate fair share
        let fair_share = self.calculate_fair_share(&request.tenant_id);
        
        if tenant.usage.exceeds_fair_share(&fair_share) {
            // Queue request for later
            tenant.requests.push_back(request);
            return Ok(Assignment::Queued);
        }
        
        // Schedule immediately
        let assignment = self.find_slot(&request).await?;
        tenant.usage.add(&request.resources);
        
        Ok(assignment)
    }
    
    fn calculate_fair_share(&self, tenant_id: &str) -> ResourceShare {
        let total_tenants = self.tenants.len();
        let total_resources = self.get_total_resources();
        
        ResourceShare {
            gpus: total_resources.gpus / total_tenants as u32,
            memory: total_resources.memory / total_tenants as u64,
        }
    }
}
```

---

*End of Kubernetes & Scalability Guide*
