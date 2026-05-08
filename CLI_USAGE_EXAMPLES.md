# Fuse CLI Usage Examples

This document provides comprehensive examples of all Fuse CLI commands.

## Table of Contents

- [Configuration Management](#configuration-management)
- [Model Management](#model-management)
- [Model Operations](#model-operations)
- [Layer Management](#layer-management)
- [Model Analysis](#model-analysis)
- [Remote Endpoints](#remote-endpoints)
- [Workflows](#workflows)
- [User Interface](#user-interface)
- [History Management](#history-management)
- [MCP Server](#mcp-server)
- [Feature Flags](#feature-flags)

## Configuration Management

### Initialize Configuration

```bash
# Interactive setup
fuse init

# Use defaults (non-interactive)
fuse init -y

# Copy from existing file
fuse init --from-file config.toml
```

### View Configuration

```bash
# Show current configuration
fuse config

# Show configuration file path
fuse config --path

# Validate configuration
fuse config --validate
```

## Model Management

### Pull Models

```bash
# Pull from default source (Hugging Face)
fuse pull gpt2

# Pull from specific source
fuse pull llama-3 --source huggingface

# Pull specific format (GGUF, Safetensors, ONNX) and model
fuse pull meta-llama/llama-3.1:latest --format GGUF

# Resume an interrupted download safely
# (Detects 416/200 HTTP responses to prevent corruption)
fuse pull gpt2 --resume
```

### List Models

```bash
# List all models
fuse list

# List with detailed information
fuse list --verbose

# Filter by source
fuse list --source huggingface
```

### Run Models

```bash
# Run model with default port
fuse run gpt2

# Run on specific port
fuse run gpt2 --port 8080
```

### Update Models

```bash
# Update to latest version
fuse update gpt2
```

### Remove Models

```bash
# Remove with confirmation
fuse rm gpt2

# Remove without confirmation
fuse rm gpt2 -y
```

## Model Operations

### Inspect Models

```bash
# Inspect model architecture
fuse inspect gpt2

# Export inspection to JSON
fuse inspect gpt2 --json
```

### Quantize Models

```bash
# Quantize with GGUF method
fuse quantize gpt2 --method gguf --format q4_0

# Quantize with custom output name
fuse quantize gpt2 --method gguf --format q4_0 --output gpt2-q4

# Quantize with GPTQ
fuse quantize llama-3 --method gptq

# Quantize with AWQ
fuse quantize mistral --method awq
```

### Scan for Vulnerabilities

```bash
# Enable vulnerability scanning first
fuse features enable vulnerability-scanning

# Scan local model
fuse scan gpt2

# Scan remote model
fuse scan https://example.com/model.bin --remote

# Generate JSON report
fuse scan gpt2 --format json --output report.json

# Generate CycloneDX SBOM
fuse scan gpt2 --format cyclonedx --output sbom.json
```

## Layer Management

### Inspect Layers

```bash
# Inspect layers (default format)
fuse layer inspect gpt2

# Inspect with detailed information
fuse layer inspect gpt2 --wide
```

### Remove Layers

```bash
# Remove specific layer
fuse layer remove gpt2 layer-5
```

### Add Layers

```bash
# Add geo-restriction layer
fuse layer add gpt2 --layer-type geo-restriction --config geo-config.json

# Add content filter layer
fuse layer add gpt2 --layer-type content-filter --config filter-config.json

# Add custom layer
fuse layer add gpt2 --layer-type custom --config custom-layer.json
```

## Model Analysis

### Check Compatibility

```bash
# Check compatibility between two models
fuse comp-check gpt2 llama-3

# Check compatibility for multiple models
fuse comp-check gpt2 llama-3 mistral
```

### Merge Models

```bash
# Merge with average strategy (default)
fuse merge gpt2 llama-3 --output merged-model

# Merge with weighted strategy
fuse merge gpt2 llama-3 --output merged-model --strategy weighted --weights 0.6,0.4

# Merge with SLERP strategy
fuse merge gpt2 llama-3 --output merged-model --strategy slerp
```

## Remote Endpoints

### Add Remote Endpoint

```bash
# Add endpoint without authentication
fuse remote add aws-endpoint https://api.example.com/models

# Add endpoint with API key
fuse remote add gcp-endpoint https://api.gcp.com/models --api-key YOUR_API_KEY
```

### List Remote Endpoints

```bash
fuse remote list
```

### Remove Remote Endpoint

```bash
fuse remote remove aws-endpoint
```

## Workflows

### Run Workflow

```bash
# Enable agentic coding first
fuse features enable agentic-coding

# Run workflow
fuse workflow run .fuse/specs/fuse.md

# Run with verbose output
fuse workflow run .fuse/specs/fuse.md --verbose
```

### List Workflows

```bash
fuse workflow list
```

### Validate Workflow

```bash
fuse workflow validate .fuse/specs/fuse.md
```

## User Interface

### Start Web UI

```bash
# Start with default settings
fuse ui

# Start on specific port
fuse ui --port 3000

# Start on specific host and port
fuse ui --host 0.0.0.0 --port 3000

# Start and open browser automatically
fuse ui --open
```

## History Management

### View History

```bash
# View all history
fuse history

# View limited number of messages
fuse history --limit 10

# Filter by model
fuse history --model gpt2
```

### Clear History

```bash
# Clear all history (with confirmation)
fuse history --clear
```

## MCP Server

### Start MCP Server

```bash
# Enable MCP server first
fuse features enable mcp-server

# Start with default port
fuse mcp start

# Start on specific port
fuse mcp start --port 9000

# Start with custom config
fuse mcp start --config mcp-config.toml
```

### Check MCP Status

```bash
fuse mcp status
```

### Stop MCP Server

```bash
fuse mcp stop
```

## Feature Flags

### List Features

```bash
fuse features list
```

### Enable Features

```bash
# Enable agentic coding
fuse features enable agentic-coding

# Enable thinking visualization
fuse features enable thinking-visualization

# Enable generative UI
fuse features enable generative-ui

# Enable MCP server
fuse features enable mcp-server

# Enable vulnerability scanning
fuse features enable vulnerability-scanning
```

### Disable Features

```bash
# Disable a feature
fuse features disable agentic-coding
```

## Global Options

All commands support these global options:

```bash
# Use custom config file
fuse --config /path/to/config.toml <command>

# Set log level
fuse --log-level debug <command>

# Combine options
fuse --config custom.toml --log-level trace pull gpt2
```

## Help and Version

```bash
# Show general help
fuse --help

# Show command-specific help
fuse pull --help
fuse layer --help
fuse quantize --help

# Show version
fuse --version
```

## Advanced Examples

### Complete Model Setup Workflow

```bash
# 1. Initialize configuration
fuse init -y

# 2. Enable features
fuse features enable vulnerability-scanning
fuse features enable agentic-coding

# 3. Pull a model
fuse pull meta-llama/llama-3.1

# 4. Scan for vulnerabilities
fuse scan meta-llama/llama-3.1

# 5. Quantize for optimization
fuse quantize meta-llama/llama-3.1 --method gguf --format q4_0 --output llama-3.1-q4

# 6. Run the quantized model
fuse run llama-3.1-q4 --port 8080
```

### Model Merging Workflow

```bash
# 1. Check compatibility
fuse comp-check gpt2 llama-3

# 2. Merge models
fuse merge gpt2 llama-3 --output hybrid-model --strategy slerp

# 3. Inspect the merged model
fuse layer inspect hybrid-model --wide

# 4. Test the merged model
fuse run hybrid-model
```

### Development Workflow with Agentic Coding

```bash
# 1. Enable agentic coding
fuse features enable agentic-coding

# 2. Pull a code-focused model
fuse pull qwen-coder

# 3. Run workflow
fuse workflow run .fuse/specs/fuse.md --verbose

# 4. View history
fuse history --model qwen-coder
```

## Error Handling

The CLI provides clear error messages with remediation suggestions:

```bash
# Invalid model name
$ fuse pull "model with spaces"
Error: Validation error: Model name contains invalid characters. 
Only alphanumeric, hyphens, underscores, forward slashes, dots, and colons are allowed

# Feature not enabled
$ fuse scan gpt2
Error: Feature not enabled: Vulnerability scanning is not enabled. 
Enable it with: fuse features enable vulnerability-scanning

# Invalid quantization method
$ fuse quantize gpt2 --method invalid
Error: Validation error: Invalid quantization method: invalid. 
Valid methods: gguf, gptq, awq, ggml

# Insufficient models for merge
$ fuse merge gpt2 --output merged
Error: Validation error: At least 2 models are required for merging
```

## Tips and Best Practices

1. **Always validate configuration**: Run `fuse config --validate` after making changes
2. **Use verbose mode for debugging**: Add `--log-level debug` to any command
3. **Check compatibility before merging**: Use `fuse comp-check` before `fuse merge`
4. **Enable features as needed**: Only enable features you plan to use
5. **Scan models for vulnerabilities**: Run `fuse scan` on models from untrusted sources
6. **Use quantization for optimization**: Quantize large models to reduce memory usage
7. **Resume interrupted downloads**: Use `--resume` flag if a download fails
8. **Backup before layer operations**: Layer modifications are destructive

## Configuration File Examples

### Minimal Configuration (config.toml)

```toml
[general]
models_dir = "~/.fuse/models"
cache_dir = "~/.fuse/cache"
log_level = "info"

[feature_flags]
agentic_coding = false
thinking_visualization = false
generative_ui = false
mcp_server = false
vulnerability_scanning = false

[server]
host = "127.0.0.1"
port = 8080
```

### Full Configuration with All Features

```toml
[general]
models_dir = "~/.fuse/models"
cache_dir = "~/.fuse/cache"
log_level = "debug"

[feature_flags]
agentic_coding = true
thinking_visualization = true
generative_ui = true
mcp_server = true
vulnerability_scanning = true

[server]
host = "0.0.0.0"
port = 8080
max_connections = 100

[server.rate_limit]
requests_per_minute = 60

[[registries.sources]]
name = "huggingface"
url = "https://huggingface.co"
auth_required = false

[[registries.sources]]
name = "custom"
url = "https://models.example.com"
auth_required = true

[inference]
default_max_tokens = 2048
default_temperature = 0.7
context_window = 4096
```

## Next Steps

- Read the [Design Document](design.md) for architecture details
- Check [Requirements](requirements.md) for feature specifications
- Review [Tasks](tasks.md) for implementation progress
- Explore the [API Documentation](API.md) for programmatic access
