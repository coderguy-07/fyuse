#!/bin/bash
# Setup pre-commit hooks for Fuse project

set -e

echo "🔧 Setting up pre-commit hooks for Fuse..."
echo ""

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is required but not installed."
    echo "Please install Python 3 and try again."
    exit 1
fi

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    echo "📦 Installing pre-commit..."
    pip3 install pre-commit
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is required but not installed."
    echo "Please install Rust from https://rustup.rs/ and try again."
    exit 1
fi

# Install Rust tools
echo "📦 Installing Rust development tools..."

# cargo-audit for security auditing
if ! command -v cargo-audit &> /dev/null; then
    echo "  - Installing cargo-audit..."
    cargo install cargo-audit
fi

# cargo-outdated for dependency checking
if ! command -v cargo-outdated &> /dev/null; then
    echo "  - Installing cargo-outdated..."
    cargo install cargo-outdated
fi

# Make scripts executable
echo "🔐 Setting script permissions..."
chmod +x scripts/*.py
chmod +x scripts/*.sh

# Initialize pre-commit
echo "🎣 Installing pre-commit hooks..."
pre-commit install
pre-commit install --hook-type commit-msg
pre-commit install --hook-type pre-push

# Create secrets baseline
echo "🔍 Creating secrets baseline..."
if command -v detect-secrets &> /dev/null; then
    detect-secrets scan > .secrets.baseline
else
    echo "  ⚠️  detect-secrets not installed, skipping baseline creation"
    echo "  Install with: pip install detect-secrets"
fi

# Create example config files
echo "📝 Creating example configuration files..."

cat > config.example.toml << 'EOF'
# Fuse Configuration Example
# Copy this file to config.toml and customize

[general]
models_dir = "~/.fuse/models"
cache_dir = "~/.fuse/cache"
log_level = "info"

[feature_flags]
agentic_coding = false
thinking_visualization = false
generative_ui = true
mcp_server = false
vulnerability_scanning = true

[server]
host = "127.0.0.1"
port = 8080
max_connections = 100

[server.rate_limit]
requests_per_minute = 60

[auth]
# Use environment variables for sensitive data
api_key = "${FUSE_API_KEY}"
secret = "${FUSE_SECRET}"

[inference]
default_max_tokens = 2048
default_temperature = 0.7
context_window = 4096

[ui]
enabled = true
theme = "auto"
history_retention_days = 90
max_conversations = 1000

[ui.context]
max_input_tokens = 4096
max_output_tokens = 2048
show_token_count = true
EOF

# Create .markdownlint.json
cat > .markdownlint.json << 'EOF'
{
  "default": true,
  "MD013": false,
  "MD033": false,
  "MD041": false
}
EOF

# Create LICENSE_HEADER
cat > LICENSE_HEADER << 'EOF'
Copyright (c) 2024 Fuse Contributors
SPDX-License-Identifier: MIT
EOF

# Create .secrets.baseline if it doesn't exist
if [ ! -f .secrets.baseline ]; then
    echo "{}" > .secrets.baseline
fi

# Run initial check
echo ""
echo "✅ Pre-commit hooks installed successfully!"
echo ""
echo "Running initial checks..."
pre-commit run --all-files || true

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✨ Setup complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Pre-commit hooks are now active and will run automatically on:"
echo "  • git commit  - Basic checks, formatting, linting"
echo "  • git push    - Extended checks, tests, security scans"
echo ""
echo "Manual commands:"
echo "  • pre-commit run --all-files    - Run all hooks on all files"
echo "  • pre-commit run <hook-id>      - Run specific hook"
echo "  • pre-commit autoupdate         - Update hook versions"
echo ""
echo "Security features enabled:"
echo "  ✓ Sensitive data detection"
echo "  ✓ Credential scanning"
echo "  ✓ Private key detection"
echo "  ✓ Configuration validation"
echo "  ✓ File permission checks"
echo ""
echo "To bypass hooks (not recommended):"
echo "  git commit --no-verify"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
