# Fuse Configuration Guide

## Configuration Files

Fuse supports both TOML and YAML configuration formats. You can use either format based on your preference.

### Location

Configuration files are stored in the `.fuse` directory in your home folder:

- **Active Config**: `~/.fuse/config.toml` (or `config.yaml`)
- **Example Configs**: 
  - `~/.fuse/config.toml.example` - TOML format reference
  - `~/.fuse/config.yaml.example` - YAML format reference

### First Run

On first run, Fuse will automatically:
1. Create the `~/.fuse` directory
2. Generate a default `config.toml` file
3. Copy example configuration files for reference

### Configuration Methods

You can configure Fuse in three ways:

#### 1. Edit Configuration File Directly

Edit the active configuration file:
```bash
# Edit TOML config
vim ~/.fuse/config.toml

# Or edit YAML config (rename first)
mv ~/.fuse/config.toml ~/.fuse/config.yaml
vim ~/.fuse/config.yaml
```

#### 2. Use CLI Commands

Manage feature flags via CLI:
```bash
# List all features
fuse features list

# Enable a feature
fuse features enable agentic-coding

# Disable a feature
fuse features disable agentic-coding
```

View current configuration:
```bash
# Show current config
fuse config

# Show config file path
fuse config --path
```

#### 3. Use Custom Config File

Specify a custom config file for a single command:
```bash
# Use custom TOML config
fuse --config /path/to/custom.toml pull llama2

# Use custom YAML config
fuse --config /path/to/custom.yaml run llama2
```

## Configuration Options

### Core Settings

```toml
# Directory where downloaded models are stored
models_dir = "~/.fuse/models"

# Directory for temporary files and cache
cache_dir = "~/.fuse/cache"

# Logging level: trace, debug, info, warn, error
log_level = "info"
```

### Feature Flags

Enable or disable optional features:

```toml
[feature_flags]
agentic_coding = false              # Automated workflow execution
thinking_visualization = false       # Display model thinking stages
generative_ui = false               # Interactive UI with feedback
mcp_server = false                  # Model Context Protocol support
vulnerability_scanning = false       # Security vulnerability scanning
```

**Available Features:**
- `agentic-coding` - Automated workflow execution with fix-compile-test loops
- `thinking-visualization` - Display model thinking and planning stages in real-time
- `generative-ui` - Interactive UI with real-time action feedback
- `mcp-server` - Model Context Protocol server support
- `vulnerability-scanning` - Scan models for security vulnerabilities

### Server Configuration

```toml
[server]
host = "127.0.0.1"                  # Server bind address
port = 8080                         # Server port
max_connections = 100               # Max concurrent connections

[server.rate_limit]
requests_per_minute = 60            # Rate limit per client

# Optional TLS configuration
# [server.tls]
# cert_path = "~/.fuse/certs/cert.pem"
# key_path = "~/.fuse/certs/key.pem"
```

### Model Registries

Add custom model registries:

```toml
[[registries]]
name = "huggingface"
url = "https://huggingface.co"
auth_required = false

[[registries]]
name = "custom-registry"
url = "https://my-registry.example.com"
auth_required = true
```

### Inference Settings

```toml
[inference]
default_max_tokens = 2048           # Max tokens to generate
default_temperature = 0.7           # Generation temperature (0.0-1.0)
context_window = 4096               # Context window size
```

## Example Configurations

### Development Configuration

```toml
log_level = "debug"

[feature_flags]
agentic_coding = true
thinking_visualization = true
generative_ui = true

[server]
host = "0.0.0.0"
port = 8080
```

### Production Configuration

```toml
log_level = "warn"

[feature_flags]
vulnerability_scanning = true

[server]
host = "0.0.0.0"
port = 443
max_connections = 1000

[server.rate_limit]
requests_per_minute = 120

[server.tls]
cert_path = "/etc/fuse/certs/cert.pem"
key_path = "/etc/fuse/certs/key.pem"
```

### Minimal Configuration

```toml
models_dir = "~/.fuse/models"
cache_dir = "~/.fuse/cache"
log_level = "info"
```

## YAML Format

If you prefer YAML, here's the equivalent configuration:

```yaml
models_dir: "~/.fuse/models"
cache_dir: "~/.fuse/cache"
log_level: "info"

feature_flags:
  agentic_coding: false
  thinking_visualization: false
  generative_ui: false
  mcp_server: false
  vulnerability_scanning: false

server:
  host: "127.0.0.1"
  port: 8080
  max_connections: 100
  rate_limit:
    requests_per_minute: 60

inference:
  default_max_tokens: 2048
  default_temperature: 0.7
  context_window: 4096
```

## Troubleshooting

### Configuration Not Loading

Check the config file path:
```bash
fuse config --path
```

### Invalid Configuration

Fuse validates configuration on load. Common issues:
- Invalid log level (must be: trace, debug, info, warn, error)
- Invalid paths
- Malformed TOML/YAML syntax

### Reset to Defaults

Delete the config file and restart:
```bash
rm ~/.fuse/config.toml
fuse config  # Will create new default config
```

### View Example Configs

Example configurations are always available:
```bash
cat ~/.fuse/config.toml.example
cat ~/.fuse/config.yaml.example
```

## Environment Variables

Override log level with environment variable:
```bash
RUST_LOG=debug fuse pull llama2
```
