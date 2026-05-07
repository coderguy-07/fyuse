# Fuse API Reference

Complete API reference for the Fuse AI System Manager. Fuse exposes three compatibility layers (Ollama, OpenAI, Anthropic), a native REST/streaming API, a WebSocket inference endpoint, and a widget WebSocket protocol.

---

## 1. Overview

| Item | Value |
|------|-------|
| **Default base URL** | `http://localhost:11434` |
| **Content-Type** | `application/json` (all request/response bodies) |
| **Streaming content types** | `application/x-ndjson` (Ollama pull), `text/event-stream` (OpenAI SSE, Anthropic SSE, native SSE) |
| **Max request body** | 10 MB (enforced by middleware) |
| **WebSocket endpoint** | `ws://localhost:11434/ws` |

---

## 2. Authentication

Fuse accepts two authentication methods. Requests without valid credentials receive `401 Unauthorized`.

### API Key (header)

```
X-API-Key: your-api-key
```

### Bearer Token (header)

```
Authorization: Bearer your-token
```

Both methods are checked by the `auth_middleware`. Any non-empty key/token is currently accepted; production deployments should configure database-backed or environment-variable-backed validation.

**curl example:**

```bash
# API key
curl http://localhost:11434/api/tags \
  -H "X-API-Key: my-secret-key"

# Bearer token
curl http://localhost:11434/api/tags \
  -H "Authorization: Bearer my-secret-token"
```

---

## 3. Rate Limiting

Fuse uses a **token-bucket** algorithm. Tokens refill continuously at `requests_per_minute / 60` tokens per second. Each unique client (identified by API key or IP address) gets an independent bucket.

| Behavior | Detail |
|----------|--------|
| **Algorithm** | Token bucket, per-client |
| **Default capacity** | Configured via `server.rate_limit.requests_per_minute` in `fuse.toml` |
| **Client identification** | `X-API-Key` header, then `X-Forwarded-For`, then `X-Real-IP`, then socket IP |
| **Bucket cleanup** | Idle buckets evicted after 300 seconds |

### 429 Response

When rate-limited, the server returns:

```
HTTP/1.1 429 Too Many Requests

Rate limit exceeded. Please try again later.
```

---

## 4. Error Handling

### Error Response Format

All API errors return a JSON body with this structure:

```json
{
  "error_code": "MODEL_NOT_FOUND",
  "message": "Model not found: llama3:8b",
  "details": null,
  "remediation": "Try pulling the model first with: fuse pull llama3:8b",
  "timestamp": "2026-04-08T12:00:00Z"
}
```

### Error Codes

| Error Code | HTTP Status | Description | Remediation |
|------------|-------------|-------------|-------------|
| `MODEL_NOT_FOUND` | 404 | Requested model does not exist locally | Pull the model with `fuse pull <name>` |
| `VALIDATION_ERROR` | 400 | Invalid request parameters | Fix the request body per the API spec |
| `CONFIG_ERROR` | 400 | Invalid configuration | Check your `fuse.toml` for errors |
| `AUTH_ERROR` | 401 | Authentication failed | Verify your API key or bearer token |
| `PERMISSION_DENIED` | 403 | Insufficient permissions | Check file and directory permissions |
| `TIMEOUT` | 408 | Operation timed out | Increase timeout or check connectivity |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests | Wait before retrying |
| `RESOURCE_LIMIT_EXCEEDED` | 429 | Resource quota exceeded | Wait or increase limits |
| `FEATURE_DISABLED` | 501 | Feature not enabled | Enable the feature in `fuse.toml` |
| `RESOURCE_UNAVAILABLE` | 503 | Temporary unavailability | Retry after a short delay |
| `INFERENCE_ERROR` | 500 | Inference engine failure | Check model compatibility and input |
| `DOWNLOAD_ERROR` | 500 | Model download failed | Check internet connection and retry |
| `WORKFLOW_ERROR` | 500 | Workflow execution failed | Review workflow definition |
| `DATABASE_ERROR` | 500 | Database operation failed | Check database integrity |
| `IO_ERROR` | 500 | File system error | Check permissions and disk space |
| `NETWORK_ERROR` | 500 | Network request failed | Check connectivity |
| `SERIALIZATION_ERROR` | 500 | JSON/data serialization error | Check request format |
| `INTERNAL_ERROR` | 500 | Unspecified internal error | Report a bug |
| `LAYER_ERROR` | 500 | Model layer operation failed | Check model file integrity |
| `QUANTIZATION_ERROR` | 500 | Quantization failed | Check model format support |
| `MERGE_ERROR` | 500 | Model merge failed | Check merge configuration |
| `SCAN_ERROR` | 500 | Model scan failed | Check model files |
| `RAG_ERROR` | 500 | RAG pipeline error | Check document index |
| `DEVICE_ERROR` | 500 | Hardware device error | Check device connectivity |
| `CHANNEL_ERROR` | 500 | Channel communication error | Check channel configuration |
| `SESSION_NOT_FOUND` | 500 | Session does not exist | Start a new session |
| `AGENT_ERROR` | 500 | Agent execution error | Check agent configuration |

### Retryable Errors

The following errors are safe to retry with exponential backoff: `NETWORK_ERROR`, `DOWNLOAD_ERROR`, `TIMEOUT`, `RESOURCE_UNAVAILABLE`.

---

## 5. Ollama-Compatible API

Drop-in replacement for the Ollama REST API. Point any Ollama client at Fuse's base URL.

### POST /api/generate

Generate a completion from a prompt.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model name |
| `prompt` | string | yes | Input prompt |
| `stream` | bool | no | Stream response (default `false`) |
| `system` | string | no | System prompt |
| `template` | string | no | Prompt template override |
| `context` | array of int | no | Context from previous response |
| `options` | object | no | Model parameters (temperature, top_p, etc.) |

**curl:**

```bash
curl -X POST http://localhost:11434/api/generate \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "prompt": "Explain quantum computing in one paragraph.",
    "stream": false,
    "system": "You are a physics professor."
  }'
```

**Response (non-streaming):**

```json
{
  "model": "llama3:8b",
  "created_at": "2026-04-08T12:00:00.000Z",
  "response": "Quantum computing harnesses quantum mechanical phenomena...",
  "done": true,
  "context": [1, 2, 3],
  "total_duration": 5043500000,
  "load_duration": 1200000000,
  "prompt_eval_count": 12,
  "eval_count": 87,
  "eval_duration": 3800000000
}
```

**Streaming format (NDJSON):** When `stream: true`, the server sends newline-delimited JSON objects. Each intermediate chunk has `done: false` and a partial `response`. The final chunk has `done: true` with timing statistics.

---

### POST /api/chat

Multi-turn chat completion.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model name |
| `messages` | array | yes | Conversation messages |
| `messages[].role` | string | yes | `"system"`, `"user"`, or `"assistant"` |
| `messages[].content` | string | yes | Message text |
| `messages[].images` | array of string | no | Base64-encoded images |
| `stream` | bool | no | Stream response (default `false`) |
| `options` | object | no | Model parameters |

**curl:**

```bash
curl -X POST http://localhost:11434/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is the capital of France?"}
    ],
    "stream": false
  }'
```

**Response:**

```json
{
  "model": "llama3:8b",
  "created_at": "2026-04-08T12:00:00.000Z",
  "message": {
    "role": "assistant",
    "content": "The capital of France is Paris."
  },
  "done": true,
  "total_duration": 3200000000,
  "eval_count": 12,
  "eval_duration": 2000000000
}
```

---

### GET /api/tags

List all locally available models.

**curl:**

```bash
curl http://localhost:11434/api/tags
```

**Response:**

```json
{
  "models": [
    {
      "name": "llama3:8b",
      "modified_at": "2026-04-08T12:00:00.000Z",
      "size": 4661218816,
      "digest": "sha256:abcdef0123456789...",
      "details": {
        "format": "gguf",
        "family": "llama3",
        "families": null,
        "parameter_size": "8B",
        "quantization_level": "Q4_K_M"
      }
    }
  ]
}
```

---

### POST /api/pull

Pull a model from a registry. Response streams as NDJSON progress lines.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Model name (e.g., `"llama3:8b"`) |
| `insecure` | bool | no | Allow insecure connections (default `false`) |
| `stream` | bool | no | Stream progress (default `false`) |

**curl:**

```bash
curl -X POST http://localhost:11434/api/pull \
  -H "Content-Type: application/json" \
  -d '{"name": "llama3:8b", "stream": true}'
```

**Response (NDJSON stream):**

```
{"status":"pulling manifest"}
{"status":"downloading","digest":"sha256:abc...","total":4661218816,"completed":1048576}
{"status":"downloading","digest":"sha256:abc...","total":4661218816,"completed":4661218816}
{"status":"success"}
```

---

### GET /api/show

Get model metadata.

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Model name |

**curl:**

```bash
curl "http://localhost:11434/api/show?name=llama3:8b"
```

**Response:**

```json
{
  "modelfile": "FROM llama3:8b\n",
  "parameters": "",
  "template": "{{ .Prompt }}",
  "details": {
    "format": "gguf",
    "family": "llama3:8b",
    "families": null,
    "parameter_size": "unknown",
    "quantization_level": "unknown"
  }
}
```

---

### POST /api/embeddings

Generate embeddings for a text prompt.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Embedding model name |
| `prompt` | string | yes | Text to embed |
| `options` | object | no | Model parameters |

**curl:**

```bash
curl -X POST http://localhost:11434/api/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "nomic-embed-text",
    "prompt": "Quantum computing leverages superposition and entanglement."
  }'
```

**Response:**

```json
{
  "embedding": [0.0123, -0.0456, 0.0789, ...]
}
```

---

## 6. OpenAI-Compatible API

Drop-in replacement for the OpenAI API. Use any OpenAI SDK by changing the base URL.

### POST /v1/chat/completions

Chat completion with optional streaming and tool calling.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model name |
| `messages` | array | yes | Conversation messages |
| `messages[].role` | string | yes | `"system"`, `"user"`, `"assistant"`, or `"tool"` |
| `messages[].content` | string\|null | no | Message content |
| `messages[].tool_calls` | array | no | Tool calls (assistant messages) |
| `messages[].tool_call_id` | string | no | ID of the tool call being responded to |
| `messages[].name` | string | no | Name of the tool/function |
| `temperature` | float | no | Sampling temperature (0.0-2.0) |
| `top_p` | float | no | Nucleus sampling threshold |
| `max_tokens` | int | no | Maximum tokens to generate |
| `stream` | bool | no | Stream via SSE (default `false`) |
| `tools` | array | no | Tool/function definitions |
| `tool_choice` | string\|object | no | `"auto"`, `"none"`, or `{"type":"function","function":{"name":"..."}}` |
| `response_format` | object | no | `{"type": "json_object"}` for JSON mode |
| `stop` | string\|array | no | Stop sequences |
| `n` | int | no | Number of choices |
| `presence_penalty` | float | no | Presence penalty (-2.0 to 2.0) |
| `frequency_penalty` | float | no | Frequency penalty (-2.0 to 2.0) |
| `seed` | int | no | Random seed for reproducibility |

**curl (non-streaming):**

```bash
curl -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer my-api-key" \
  -d '{
    "model": "llama3:8b",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "temperature": 0.7,
    "max_tokens": 256
  }'
```

**Response (non-streaming):**

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1712577600,
  "model": "llama3:8b",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "2 + 2 = 4."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 24,
    "completion_tokens": 8,
    "total_tokens": 32
  }
}
```

**curl (streaming):**

```bash
curl -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true
  }'
```

**SSE streaming format:**

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1712577600,"model":"llama3:8b","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1712577600,"model":"llama3:8b","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1712577600,"model":"llama3:8b","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

**curl (with tool calling):**

```bash
curl -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "messages": [{"role": "user", "content": "What is the weather in Paris?"}],
    "tools": [
      {
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get current weather for a location",
          "parameters": {
            "type": "object",
            "properties": {
              "location": {"type": "string", "description": "City name"}
            },
            "required": ["location"]
          }
        }
      }
    ],
    "tool_choice": "auto"
  }'
```

**Tool call response:**

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1712577600,
  "model": "llama3:8b",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "tool_calls": [
          {
            "id": "call_abc123",
            "type": "function",
            "function": {
              "name": "get_weather",
              "arguments": "{\"location\": \"Paris\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ],
  "usage": { "prompt_tokens": 40, "completion_tokens": 16, "total_tokens": 56 }
}
```

---

### POST /v1/embeddings

Generate embeddings.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Embedding model name |
| `input` | string\|array | yes | Text(s) to embed |
| `encoding_format` | string | no | `"float"` (default) or `"base64"` |

**curl:**

```bash
curl -X POST http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "text-embedding-ada-002",
    "input": "Hello world"
  }'
```

**Response:**

```json
{
  "object": "list",
  "data": [
    {
      "object": "embedding",
      "embedding": [0.0023, -0.0091, 0.0154, ...],
      "index": 0
    }
  ],
  "model": "text-embedding-ada-002",
  "usage": {
    "prompt_tokens": 2,
    "total_tokens": 2
  }
}
```

**curl (batch):**

```bash
curl -X POST http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "text-embedding-ada-002",
    "input": ["Hello world", "Goodbye world"]
  }'
```

---

### GET /v1/models

List available models.

**curl:**

```bash
curl http://localhost:11434/v1/models
```

**Response:**

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama3:8b",
      "object": "model",
      "created": 1712577600,
      "owned_by": "fuse"
    }
  ]
}
```

---

## 7. Anthropic-Compatible API

Drop-in replacement for the Anthropic Messages API. Use any Anthropic SDK by changing the base URL.

### POST /v1/messages

Create a message (chat completion).

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model name |
| `messages` | array | yes | Conversation messages |
| `messages[].role` | string | yes | `"user"` or `"assistant"` |
| `messages[].content` | string\|array | yes | Text string or array of content blocks |
| `max_tokens` | int | yes | Maximum tokens to generate |
| `system` | string | no | System prompt |
| `temperature` | float | no | Sampling temperature |
| `top_p` | float | no | Nucleus sampling |
| `top_k` | int | no | Top-K sampling |
| `stream` | bool | no | Stream via SSE (default `false`) |
| `stop_sequences` | array | no | Stop sequences |
| `metadata` | object | no | Request metadata (`user_id`, etc.) |

**Content block types (in message content arrays):**

- `{"type": "text", "text": "..."}` -- text content
- `{"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "..."}}` -- image
- `{"type": "tool_use", "id": "...", "name": "...", "input": {...}}` -- tool use
- `{"type": "tool_result", "tool_use_id": "...", "content": "..."}` -- tool result

**curl (non-streaming):**

```bash
curl -X POST http://localhost:11434/v1/messages \
  -H "Content-Type: application/json" \
  -H "X-API-Key: my-api-key" \
  -d '{
    "model": "claude-3-sonnet",
    "messages": [
      {"role": "user", "content": "Explain recursion in one sentence."}
    ],
    "max_tokens": 1024,
    "system": "You are a computer science tutor."
  }'
```

**Response:**

```json
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "Recursion is a technique where a function calls itself with a simpler version of the original problem until it reaches a base case."
    }
  ],
  "model": "claude-3-sonnet",
  "stop_reason": "end_turn",
  "stop_sequence": null,
  "usage": {
    "input_tokens": 18,
    "output_tokens": 32
  }
}
```

**curl (streaming):**

```bash
curl -X POST http://localhost:11434/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-sonnet",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 1024,
    "stream": true
  }'
```

**SSE streaming format (Anthropic protocol):**

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_abc123","type":"message","role":"assistant","content":[],"model":"claude-3-sonnet","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":0,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello! How can I help you today?"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"input_tokens":0,"output_tokens":0}}

event: message_stop
data: {"type":"message_stop"}
```

**curl (multi-turn):**

```bash
curl -X POST http://localhost:11434/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-sonnet",
    "messages": [
      {"role": "user", "content": "What is 2+2?"},
      {"role": "assistant", "content": "4"},
      {"role": "user", "content": "And what is 3+3?"}
    ],
    "max_tokens": 1024
  }'
```

---

## 8. Native REST API

Fuse also exposes its own native endpoints.

### GET /health

Health check.

**curl:**

```bash
curl http://localhost:11434/health
```

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

### POST /infer

Native inference endpoint with full parameter control.

**Request:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | yes | Model name |
| `prompt` | string | yes | Input prompt (must not be empty) |
| `images` | array | no | Image inputs (`{data, format}`) |
| `max_tokens` | int | no | Maximum tokens |
| `temperature` | float | no | Sampling temperature |
| `top_p` | float | no | Nucleus sampling |
| `top_k` | int | no | Top-K sampling |
| `stop_sequences` | array | no | Stop sequences |
| `frequency_penalty` | float | no | Frequency penalty |
| `presence_penalty` | float | no | Presence penalty |
| `seed` | int | no | Random seed |

**curl:**

```bash
curl -X POST http://localhost:11434/infer \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "prompt": "What is the meaning of life?",
    "max_tokens": 256,
    "temperature": 0.8
  }'
```

**Response:**

```json
{
  "model": "llama3:8b",
  "response": "The meaning of life is...",
  "formatted_response": "The meaning of life is...",
  "prompt_tokens": 8,
  "completion_tokens": 45,
  "total_tokens": 53,
  "inference_time_ms": 1234
}
```

### POST /infer/stream

Same request body as `/infer`, returns SSE token stream.

**curl:**

```bash
curl -X POST http://localhost:11434/infer/stream \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3:8b",
    "prompt": "Tell me a story.",
    "max_tokens": 512
  }'
```

**SSE events:**

```
data: {"text":"Once","id":1,"is_final":false}
data: {"text":" upon","id":2,"is_final":false}
data: {"text":" a","id":3,"is_final":false}
data: {"text":"","id":4,"is_final":true}
```

On error during streaming:

```
event: error
data: {"error":"Model failed to generate tokens"}
```

### GET /models

List all models with detailed info.

**curl:**

```bash
curl http://localhost:11434/models
```

**Response:**

```json
[
  {
    "id": "llama3-8b-q4",
    "name": "llama3:8b",
    "size_bytes": 4661218816,
    "architecture": "llama",
    "parameter_count": 8000000000,
    "source": "huggingface",
    "version": "1.0",
    "loaded": true,
    "memory_usage_bytes": 5200000000,
    "is_busy": false,
    "tags": ["chat", "general"]
  }
]
```

### GET /models/:name

Get details for a specific model.

**curl:**

```bash
curl http://localhost:11434/models/llama3:8b
```

### POST /models/:name/load

Load a model into memory.

**curl:**

```bash
curl -X POST http://localhost:11434/models/llama3:8b/load
```

**Response:**

```json
{
  "model": "llama3:8b",
  "handle_id": "handle-abc123",
  "message": "Model 'llama3:8b' loaded successfully"
}
```

### POST /models/:name/unload

Unload a model from memory.

**curl:**

```bash
curl -X POST http://localhost:11434/models/llama3:8b/unload
```

**Response:**

```json
{
  "model": "llama3:8b",
  "message": "Model 'llama3:8b' unloaded successfully"
}
```

---

## 9. WebSocket API

### Endpoint: /ws

Persistent bidirectional connection for streaming inference.

**Connect:**

```bash
websocat ws://localhost:11434/ws
```

### Client Message Types

**Infer** -- start an inference request:

```json
{
  "type": "infer",
  "model": "llama3:8b",
  "prompt": "What is AI?",
  "images": [],
  "parameters": {
    "max_tokens": 256,
    "temperature": 0.7
  }
}
```

**Cancel** -- cancel the current inference:

```json
{"type": "cancel"}
```

**Ping** -- keepalive:

```json
{"type": "ping"}
```

### Server Message Types

**Token** -- a generated token:

```json
{"type": "token", "text": "Artificial", "id": 1, "is_final": false}
```

**Complete** -- generation finished:

```json
{
  "type": "complete",
  "prompt_tokens": 5,
  "completion_tokens": 128,
  "total_tokens": 133
}
```

**Error** -- an error occurred:

```json
{"type": "error", "message": "Model not found: llama3:8b"}
```

**Pong** -- response to ping:

```json
{"type": "pong"}
```

### Message Flow

```
Client                          Server
  |--- {"type":"ping"} ----------->|
  |<-- {"type":"pong"} ------------|
  |                                |
  |--- {"type":"infer",...} ------>|
  |<-- {"type":"token",...} -------|  (repeated)
  |<-- {"type":"token",...} -------|
  |<-- {"type":"complete",...} ----|
  |                                |
  |--- {"type":"cancel"} -------->|  (optional: abort early)
```

---

## 10. Widget WebSocket API

WebSocket protocol for the embeddable web chat widget. Connects to the same `/ws` endpoint but uses a simplified message format.

### Client Message Types

**Chat** -- send a user message:

```json
{"type": "chat", "text": "Hello, what can you do?"}
```

**Ping** -- keepalive:

```json
{"type": "ping"}
```

### Server Message Types

**Token** -- a streamed response token:

```json
{"type": "token", "text": "I can help you with"}
```

**Done** -- response complete:

```json
{"type": "done", "total_tokens": 42}
```

**Error** -- an error occurred:

```json
{"type": "error", "message": "Session expired"}
```

**Pong** -- keepalive response:

```json
{"type": "pong"}
```

### Embedding the Widget

Add this to any webpage:

```html
<script src="http://localhost:11434/widget.js"
        data-theme="auto"
        data-title="Fuse Chat"></script>
```

Configure in `fuse.toml`:

```toml
[channels.web_widget]
enabled = true
cors_origins = ["*"]
max_sessions = 100
session_timeout_secs = 3600
theme = "auto"
title = "Fuse Chat"
```

---

## 11. SDK Examples

### Python (requests)

```python
import requests

# Ollama-compatible generate
resp = requests.post("http://localhost:11434/api/generate", json={
    "model": "llama3:8b",
    "prompt": "Hello, world!",
    "stream": False,
})
print(resp.json()["response"])

# OpenAI-compatible chat
resp = requests.post("http://localhost:11434/v1/chat/completions", json={
    "model": "llama3:8b",
    "messages": [{"role": "user", "content": "Hello"}],
})
print(resp.json()["choices"][0]["message"]["content"])

# Anthropic-compatible messages
resp = requests.post("http://localhost:11434/v1/messages", json={
    "model": "llama3:8b",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 1024,
})
print(resp.json()["content"][0]["text"])
```

### Python (OpenAI SDK)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:11434/v1",
    api_key="any-key",  # Fuse accepts any non-empty key
)

# Non-streaming
response = client.chat.completions.create(
    model="llama3:8b",
    messages=[{"role": "user", "content": "What is 2+2?"}],
    temperature=0.7,
)
print(response.choices[0].message.content)

# Streaming
stream = client.chat.completions.create(
    model="llama3:8b",
    messages=[{"role": "user", "content": "Tell me a joke."}],
    stream=True,
)
for chunk in stream:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="")

# Embeddings
embedding = client.embeddings.create(
    model="nomic-embed-text",
    input="Hello world",
)
print(f"Dimension: {len(embedding.data[0].embedding)}")
```

### Python (Anthropic SDK)

```python
import anthropic

client = anthropic.Anthropic(
    base_url="http://localhost:11434",
    api_key="any-key",
)

# Non-streaming
message = client.messages.create(
    model="llama3:8b",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello!"}],
)
print(message.content[0].text)

# Streaming
with client.messages.stream(
    model="llama3:8b",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Tell me a story."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

### JavaScript (fetch)

```javascript
// OpenAI-compatible chat
const response = await fetch("http://localhost:11434/v1/chat/completions", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    model: "llama3:8b",
    messages: [{ role: "user", content: "Hello" }],
  }),
});
const data = await response.json();
console.log(data.choices[0].message.content);

// Streaming with SSE
const streamResp = await fetch("http://localhost:11434/v1/chat/completions", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    model: "llama3:8b",
    messages: [{ role: "user", content: "Hello" }],
    stream: true,
  }),
});

const reader = streamResp.body.getReader();
const decoder = new TextDecoder();
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  const text = decoder.decode(value);
  for (const line of text.split("\n")) {
    if (line.startsWith("data: ") && line !== "data: [DONE]") {
      const chunk = JSON.parse(line.slice(6));
      const content = chunk.choices[0]?.delta?.content;
      if (content) process.stdout.write(content);
    }
  }
}
```

### JavaScript (WebSocket)

```javascript
const ws = new WebSocket("ws://localhost:11434/ws");

ws.onopen = () => {
  ws.send(JSON.stringify({
    type: "infer",
    model: "llama3:8b",
    prompt: "What is quantum computing?",
    parameters: { max_tokens: 256 },
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  switch (msg.type) {
    case "token":
      process.stdout.write(msg.text);
      break;
    case "complete":
      console.log(`\n[${msg.total_tokens} tokens]`);
      ws.close();
      break;
    case "error":
      console.error("Error:", msg.message);
      ws.close();
      break;
  }
};
```

### curl Quick Reference

```bash
# Health check
curl http://localhost:11434/health

# List models (Ollama)
curl http://localhost:11434/api/tags

# List models (OpenAI)
curl http://localhost:11434/v1/models

# Generate (Ollama)
curl -X POST http://localhost:11434/api/generate \
  -H "Content-Type: application/json" \
  -d '{"model":"llama3:8b","prompt":"Hi","stream":false}'

# Chat (Ollama)
curl -X POST http://localhost:11434/api/chat \
  -H "Content-Type: application/json" \
  -d '{"model":"llama3:8b","messages":[{"role":"user","content":"Hi"}]}'

# Chat (OpenAI)
curl -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"llama3:8b","messages":[{"role":"user","content":"Hi"}]}'

# Messages (Anthropic)
curl -X POST http://localhost:11434/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"model":"llama3:8b","messages":[{"role":"user","content":"Hi"}],"max_tokens":1024}'

# Pull model
curl -X POST http://localhost:11434/api/pull \
  -H "Content-Type: application/json" \
  -d '{"name":"llama3:8b"}'

# Embeddings (Ollama)
curl -X POST http://localhost:11434/api/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","prompt":"Hello"}'

# Embeddings (OpenAI)
curl -X POST http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","input":"Hello"}'

# Show model info
curl "http://localhost:11434/api/show?name=llama3:8b"

# Load model
curl -X POST http://localhost:11434/models/llama3:8b/load

# Unload model
curl -X POST http://localhost:11434/models/llama3:8b/unload
```
