# Fuse API Documentation

## Version: 1.0.0
## Base URL: `/api/v1`

---

## Table of Contents

1. [Authentication](#1-authentication)
2. [Ollama-Compatible API](#2-ollama-compatible-api)
3. [Extended API](#3-extended-api)
4. [WebSocket API](#4-websocket-api)
5. [Error Handling](#5-error-handling)
6. [Rate Limiting](#6-rate-limiting)

---

## 1. Authentication

### 1.1 API Key Authentication

```http
GET /api/v1/models
Authorization: Bearer {api_key}
```

### 1.2 OAuth 2.0

```http
POST /oauth/token
Content-Type: application/x-www-form-urlencoded

grant_type=client_credentials&
client_id={client_id}&
client_secret={client_secret}&
scope=models:read inference:write
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "scope": "models:read inference:write"
}
```

---

## 2. Ollama-Compatible API

Fuse implements the Ollama API for drop-in compatibility.

### 2.1 Generate Completion

```http
POST /api/generate
Content-Type: application/json

{
  "model": "llama3",
  "prompt": "Why is the sky blue?",
  "stream": false,
  "options": {
    "temperature": 0.7,
    "num_predict": 128
  }
}
```

**Response:**
```json
{
  "model": "llama3",
  "created_at": "2024-01-15T10:30:00Z",
  "response": "The sky appears blue due to Rayleigh scattering...",
  "done": true,
  "context": [1, 2, 3, ...],
  "total_duration": 4523000000,
  "load_duration": 1234000000,
  "prompt_eval_count": 7,
  "prompt_eval_duration": 234000000,
  "eval_count": 28,
  "eval_duration": 3055000000
}
```

### 2.2 Chat Completion

```http
POST /api/chat
Content-Type: application/json

{
  "model": "llama3",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant."
    },
    {
      "role": "user",
      "content": "Hello!"
    }
  ],
  "stream": true,
  "options": {
    "temperature": 0.8
  }
}
```

**Streaming Response:**
```json
{"model":"llama3","created_at":"2024-01-15T10:30:00Z","message":{"role":"assistant","content":"Hello"},"done":false}
{"model":"llama3","created_at":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":" there"},"done":false}
{"model":"llama3","created_at":"2024-01-15T10:30:02Z","message":{"role":"assistant","content":"!"},"done":true,"total_duration":2345000000}
```

### 2.3 List Models

```http
GET /api/tags
```

**Response:**
```json
{
  "models": [
    {
      "name": "llama3",
      "model": "llama3:latest",
      "modified_at": "2024-01-10T08:00:00Z",
      "size": 4661222336,
      "digest": "sha256:a2e2c2b2...",
      "details": {
        "format": "gguf",
        "family": "llama",
        "families": ["llama"],
        "parameter_size": "8B",
        "quantization_level": "Q4_0"
      }
    }
  ]
}
```

### 2.4 Show Model Info

```http
POST /api/show
Content-Type: application/json

{
  "name": "llama3"
}
```

**Response:**
```json
{
  "license": "LLAMA 3 COMMUNITY LICENSE",
  "modelfile": "FROM llama3...",
  "parameters": "8B",
  "template": "{{ .System }}...",
  "details": {
    "format": "gguf",
    "family": "llama",
    "parameter_size": "8B",
    "quantization_level": "Q4_0"
  },
  "model_info": {
    "general.architecture": "llama",
    "llama.context_length": 8192,
    "llama.embedding_length": 4096
  }
}
```

### 2.5 Pull Model

```http
POST /api/pull
Content-Type: application/json

{
  "name": "llama3",
  "insecure": false,
  "stream": true
}
```

**Streaming Response:**
```json
{"status":"pulling manifest"}
{"status":"pulling 6a0746a1...","completed":3243254592,"total":4661222336}
{"status":"verifying sha256 digest"}
{"status":"writing manifest"}
{"status":"success"}
```

### 2.6 Create Model

```http
POST /api/create
Content-Type: application/json

{
  "name": "mario",
  "modelfile": "FROM llama3\nSYSTEM You are Mario from Super Mario Bros."
}
```

### 2.7 Delete Model

```http
DELETE /api/delete
Content-Type: application/json

{
  "name": "mario"
}
```

### 2.8 Generate Embeddings

```http
POST /api/embeddings
Content-Type: application/json

{
  "model": "llama3",
  "prompt": "Here is an article about llamas..."
}
```

**Response:**
```json
{
  "embedding": [0.1, 0.2, 0.3, ...]
}
```

---

## 3. Extended API

### 3.1 Batch Inference

```http
POST /api/v1/batch
Content-Type: application/json

{
  "model": "llama3",
  "requests": [
    {
      "id": "req-1",
      "prompt": "What is Rust?"
    },
    {
      "id": "req-2",
      "prompt": "Explain borrowing in Rust"
    }
  ],
  "options": {
    "temperature": 0.7,
    "max_tokens": 256
  },
  "callback_url": "https://example.com/webhook"
}
```

**Response:**
```json
{
  "batch_id": "batch-abc123",
  "status": "queued",
  "estimated_completion": "2024-01-15T10:35:00Z",
  "request_count": 2
}
```

### 3.2 Get Batch Status

```http
GET /api/v1/batch/{batch_id}
```

**Response:**
```json
{
  "batch_id": "batch-abc123",
  "status": "completed",
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:32:15Z",
  "results": [
    {
      "id": "req-1",
      "status": "success",
      "response": "Rust is a systems programming language...",
      "tokens": 156
    },
    {
      "id": "req-2",
      "status": "success",
      "response": "Borrowing is Rust's way of memory management...",
      "tokens": 203
    }
  ],
  "total_tokens": 359,
  "total_duration_ms": 135000
}
```

### 3.3 Model Management

#### List Models (Extended)

```http
GET /api/v1/models
```

**Response:**
```json
{
  "models": [
    {
      "id": "llama3-8b",
      "name": "llama3",
      "version": "8b-instruct-q4_0",
      "source": "huggingface",
      "size_bytes": 4661222336,
      "quantization": {
        "method": "gguf",
        "format": "Q4_0"
      },
      "status": "ready",
      "loaded_at": "2024-01-15T10:00:00Z",
      "metadata": {
        "context_length": 8192,
        "vocab_size": 128256,
        "parameters": "8B"
      }
    }
  ],
  "total": 1,
  "page": 1,
  "per_page": 20
}
```

#### Pull Model (Extended)

```http
POST /api/v1/models/pull
Content-Type: application/json

{
  "source": "huggingface",
  "repository": "meta-llama/Meta-Llama-3-8B-Instruct",
  "quantization": {
    "enabled": true,
    "method": "gguf",
    "format": "Q4_K_M"
  }
}
```

**Response:**
```json
{
  "job_id": "pull-xyz789",
  "status": "pending",
  "model_id": "llama3-8b-q4km",
  "estimated_size": 4661222336
}
```

#### Delete Model

```http
DELETE /api/v1/models/{model_id}
```

#### Model Quantization

```http
POST /api/v1/models/{model_id}/quantize
Content-Type: application/json

{
  "method": "gguf",
  "format": "Q4_K_M",
  "output_name": "llama3-8b-q4km"
}
```

**Response:**
```json
{
  "job_id": "quant-abc123",
  "status": "processing",
  "progress": 0,
  "output_model_id": "llama3-8b-q4km"
}
```

### 3.4 Workflow Management

#### Run Workflow

```http
POST /api/v1/workflows/run
Content-Type: application/json

{
  "workflow": {
    "name": "code-review",
    "steps": [
      {
        "id": "discover",
        "action": "discover_files",
        "config": {
          "pattern": "*.rs"
        }
      },
      {
        "id": "analyze",
        "action": "analyze_code",
        "depends_on": ["discover"],
        "config": {
          "model": "llama3"
        }
      }
    ]
  },
  "inputs": {
    "repository": "/path/to/repo"
  }
}
```

**Response:**
```json
{
  "execution_id": "exec-123",
  "status": "running",
  "started_at": "2024-01-15T10:30:00Z"
}
```

#### Get Workflow Status

```http
GET /api/v1/workflows/{execution_id}
```

**Response:**
```json
{
  "execution_id": "exec-123",
  "status": "completed",
  "workflow_name": "code-review",
  "started_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:32:00Z",
  "steps": [
    {
      "id": "discover",
      "status": "completed",
      "started_at": "2024-01-15T10:30:00Z",
      "completed_at": "2024-01-15T10:30:05Z",
      "output": {
        "files": ["src/main.rs", "src/lib.rs"]
      }
    },
    {
      "id": "analyze",
      "status": "completed",
      "started_at": "2024-01-15T10:30:05Z",
      "completed_at": "2024-01-15T10:32:00Z",
      "output": {
        "findings": [...]
      }
    }
  ]
}
```

### 3.5 Queue Management

#### Get Queue Status

```http
GET /api/v1/queue/status
```

**Response:**
```json
{
  "queue_depth": 15,
  "active_jobs": 4,
  "completed_jobs": 1024,
  "failed_jobs": 3,
  "average_wait_time_ms": 1250,
  "average_processing_time_ms": 3420,
  "models": {
    "llama3": {
      "queued": 10,
      "active": 2
    }
  }
}
```

#### Flush Queue

```http
POST /api/v1/queue/flush
Content-Type: application/json

{
  "status": ["pending", "failed"],
  "model": "llama3"
}
```

### 3.6 Metrics

```http
GET /api/v1/metrics
```

**Response:**
```json
{
  "system": {
    "cpu_percent": 45.2,
    "memory_used_bytes": 17179869184,
    "memory_total_bytes": 34359738368,
    "gpu": [
      {
        "id": "gpu-0",
        "name": "NVIDIA A100",
        "utilization_percent": 78.5,
        "memory_used_bytes": 34359738368,
        "memory_total_bytes": 42949672960,
        "temperature_celsius": 65
      }
    ]
  },
  "inference": {
    "total_requests": 15420,
    "requests_per_second": 12.5,
    "average_latency_ms": 125,
    "p99_latency_ms": 450,
    "errors_per_second": 0.02
  },
  "models": {
    "llama3": {
      "loaded": true,
      "requests_total": 8420,
      "average_latency_ms": 98
    }
  }
}
```

---

## 4. WebSocket API

### 4.1 Real-time Streaming

```javascript
const ws = new WebSocket('wss://api.fuse.ai/ws/stream');

// Authenticate
ws.onopen = () => {
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'Bearer {api_key}'
  }));
};

// Start inference stream
ws.send(JSON.stringify({
  type: 'inference.start',
  request: {
    model: 'llama3',
    prompt: 'Tell me a story',
    stream: true
  }
}));

// Receive streaming tokens
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  switch (message.type) {
    case 'token':
      console.log('Token:', message.content);
      break;
    case 'error':
      console.error('Error:', message.error);
      break;
    case 'complete':
      console.log('Complete:', message.stats);
      break;
  }
};
```

### 4.2 Message Types

#### Client to Server

| Type | Description | Payload |
|------|-------------|---------|
| `auth` | Authenticate connection | `{ token: string }` |
| `inference.start` | Start inference | `{ model, prompt, options }` |
| `inference.cancel` | Cancel inference | `{ request_id }` |
| `ping` | Keep-alive | `{}` |

#### Server to Client

| Type | Description | Payload |
|------|-------------|---------|
| `token` | Streaming token | `{ content, request_id }` |
| `error` | Error message | `{ error, code }` |
| `complete` | Inference complete | `{ stats, request_id }` |
| `pong` | Keep-alive response | `{}` |

---

## 5. Error Handling

### 5.1 Error Response Format

```json
{
  "error": {
    "code": "MODEL_NOT_FOUND",
    "message": "Model 'llama3' not found",
    "details": {
      "model_id": "llama3"
    },
    "request_id": "req-abc123",
    "documentation_url": "https://docs.fuse.ai/errors/MODEL_NOT_FOUND"
  }
}
```

### 5.2 Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `UNAUTHORIZED` | 401 | Invalid or missing authentication |
| `FORBIDDEN` | 403 | Insufficient permissions |
| `MODEL_NOT_FOUND` | 404 | Requested model not found |
| `MODEL_NOT_LOADED` | 409 | Model exists but not loaded |
| `INSUFFICIENT_RESOURCES` | 503 | Not enough resources (GPU/CPU/memory) |
| `RATE_LIMIT_EXCEEDED` | 429 | Rate limit exceeded |
| `INVALID_REQUEST` | 400 | Invalid request parameters |
| `INFERENCE_ERROR` | 500 | Error during inference |
| `TIMEOUT` | 504 | Request timeout |

---

## 6. Rate Limiting

### 6.1 Headers

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Request limit per window |
| `X-RateLimit-Remaining` | Remaining requests in window |
| `X-RateLimit-Reset` | Unix timestamp when limit resets |
| `X-RateLimit-Retry-After` | Seconds until retry (on 429) |

### 6.2 Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/api/generate` | 60 | 1 minute |
| `/api/chat` | 60 | 1 minute |
| `/api/embeddings` | 120 | 1 minute |
| `/api/v1/batch` | 10 | 1 minute |
| All other | 120 | 1 minute |

---

## 7. OpenAPI Specification

```yaml
openapi: 3.0.3
info:
  title: Fuse API
  version: 1.0.0
  description: AI Model Management and Inference API

servers:
  - url: https://api.fuse.ai/api/v1
  - url: http://localhost:8080/api/v1

security:
  - BearerAuth: []
  - ApiKeyAuth: []

paths:
  /models:
    get:
      summary: List all models
      operationId: listModels
      responses:
        '200':
          description: List of models
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ModelList'

  /generate:
    post:
      summary: Generate completion
      operationId: generate
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/GenerateRequest'
      responses:
        '200':
          description: Generated completion
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/GenerateResponse'

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key

  schemas:
    ModelList:
      type: object
      properties:
        models:
          type: array
          items:
            $ref: '#/components/schemas/Model'

    Model:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        status:
          type: string
          enum: [pending, loading, ready, error]

    GenerateRequest:
      type: object
      required:
        - model
        - prompt
      properties:
        model:
          type: string
        prompt:
          type: string
        stream:
          type: boolean
          default: false
        options:
          type: object
          properties:
            temperature:
              type: number
              minimum: 0
              maximum: 2
            num_predict:
              type: integer

    GenerateResponse:
      type: object
      properties:
        model:
          type: string
        response:
          type: string
        done:
          type: boolean
        total_duration:
          type: integer
```

---

*End of API Documentation*
