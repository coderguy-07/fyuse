# Security Setup Guide

This guide explains how to set up and use Fuse's security features to prevent sensitive data from being committed to GitHub.

## Quick Start

```bash
# Run the setup script
./scripts/setup_hooks.sh

# Verify installation
pre-commit run --all-files
```

## What Gets Protected

### 🔐 Credentials and Secrets

The pre-commit hooks detect and block:

- API keys and tokens (AWS, GitHub, Slack, Hugging Face, etc.)
- Passwords and secrets
- Private keys (RSA, DSA, ECDSA, ED25519)
- Database connection strings
- JWT tokens
- OAuth tokens
- SSH keys
- TLS/SSL certificates

### 📁 Sensitive Files

Automatically ignored via `.gitignore`:

- Model files (*.bin, *.gguf, *.safetensors, etc.)
- Database files (*.db, *.sqlite, *.redb)
- Configuration files with credentials
- Log files
- Cache directories
- User data directories

### ⚙️ Configuration Files

Configuration files are validated to ensure:

- No hardcoded credentials
- Use of environment variables
- Placeholder values in examples
- Proper file permissions

## Pre-commit Hooks

### Automatic Checks (on `git commit`)

1. **File Size Check**: Prevents large files (>1MB)
2. **Syntax Validation**: YAML, TOML, JSON
3. **Code Formatting**: `cargo fmt`
4. **Linting**: `cargo clippy`
5. **Secret Detection**: Multiple secret scanners
6. **Private Key Detection**: Blocks private keys
7. **AWS Credential Detection**: Blocks AWS keys
8. **Merge Conflict Detection**: Prevents incomplete merges

### Extended Checks (on `git push`)

1. **Test Suite**: `cargo test`
2. **Security Audit**: `cargo audit`
3. **TODO/FIXME Check**: Warns about unresolved TODOs
4. **Debug Print Check**: Detects debug statements

## Using Environment Variables

### Best Practice

Always use environment variables for sensitive data:

```toml
# config.toml - ✅ GOOD
[auth]
api_key = "${FUSE_API_KEY}"
huggingface_token = "${HF_TOKEN}"
aws_access_key = "${AWS_ACCESS_KEY_ID}"
```

```toml
# config.toml - ❌ BAD
[auth]
api_key = "sk-abc123xyz789"  # This will be blocked!
```

### Setting Environment Variables

#### Linux/macOS

```bash
# Add to ~/.bashrc or ~/.zshrc
export FUSE_API_KEY="your-api-key-here"
export HF_TOKEN="your-huggingface-token"
export AWS_ACCESS_KEY_ID="your-aws-key"
export AWS_SECRET_ACCESS_KEY="your-aws-secret"

# Reload shell
source ~/.bashrc  # or ~/.zshrc
```

#### Windows (PowerShell)

```powershell
# Add to $PROFILE
$env:FUSE_API_KEY = "your-api-key-here"
$env:HF_TOKEN = "your-huggingface-token"

# Or use System Environment Variables
[System.Environment]::SetEnvironmentVariable('FUSE_API_KEY', 'your-key', 'User')
```

### Using .env Files

```bash
# Create .env file (automatically ignored)
cat > .env << EOF
FUSE_API_KEY=your-api-key-here
HF_TOKEN=your-huggingface-token
AWS_ACCESS_KEY_ID=your-aws-key
AWS_SECRET_ACCESS_KEY=your-aws-secret
EOF

# Load in your application
# Rust: use dotenv crate
# Shell: source .env
```

## Configuration Examples

### Example Configuration File

```toml
# config.example.toml - Safe to commit
[general]
models_dir = "~/.fuse/models"
cache_dir = "~/.fuse/cache"
log_level = "info"

[auth]
# Use environment variables for sensitive data
api_key = "${FUSE_API_KEY}"
secret = "${FUSE_SECRET}"

# Or use placeholders
# api_key = "YOUR_API_KEY_HERE"
# secret = "YOUR_SECRET_HERE"

[server]
host = "127.0.0.1"
port = 8080

[registries]
[[registries.sources]]
name = "huggingface"
url = "https://huggingface.co"
auth_token = "${HF_TOKEN}"
```

### Local Configuration

```bash
# Copy example to local config
cp config.example.toml config.toml

# Edit with your values
vim config.toml

# config.toml is in .gitignore and won't be committed
```

## Manual Security Checks

### Check for Secrets

```bash
# Run secret detection
detect-secrets scan

# Scan specific files
detect-secrets scan src/config.rs

# Update baseline
detect-secrets scan --baseline .secrets.baseline
```

### Check for Credentials in Config

```bash
# Validate configuration files
python scripts/validate_config.py config.toml

# Check all config files
python scripts/validate_config.py *.toml *.yaml
```

### Check File Permissions

```bash
# Check permissions
python scripts/check_file_permissions.py config.toml

# Fix permissions
chmod 600 config.toml  # Private files
chmod 644 README.md    # Public files
chmod 755 scripts/*.sh # Executables
```

## Bypassing Hooks (Not Recommended)

### Skip Pre-commit Hooks

```bash
# Skip all hooks (use with caution!)
git commit --no-verify

# Skip specific hook
SKIP=cargo-test git commit
```

### When to Bypass

Only bypass hooks when:

- You're committing example/test data
- You're fixing the hooks themselves
- You have a valid reason and understand the risks

**Never bypass hooks to commit sensitive data!**

## Troubleshooting

### Hook Fails with "Command not found"

```bash
# Install missing tools
pip install pre-commit detect-secrets
cargo install cargo-audit cargo-outdated

# Reinstall hooks
pre-commit uninstall
pre-commit install
```

### False Positives

If a hook incorrectly flags something:

1. **Verify it's actually safe**
2. **Add to exclusion list** in `.pre-commit-config.yaml`
3. **Update baseline** for detect-secrets

```bash
# Update secrets baseline
detect-secrets scan --baseline .secrets.baseline
```

### Slow Pre-commit Hooks

```bash
# Run only fast hooks on commit
pre-commit run --hook-stage commit

# Run slow hooks manually
pre-commit run --hook-stage push --all-files
```

### Clean Up After Accidental Commit

If you accidentally committed sensitive data:

```bash
# 1. Remove from latest commit
git reset HEAD~1
git add -A
git commit -m "Remove sensitive data"

# 2. Remove from history (if already pushed)
# See SECURITY.md for detailed instructions

# 3. Rotate compromised credentials immediately!
```

## Best Practices

### Development Workflow

1. **Before Starting**:
   ```bash
   # Set up environment variables
   export FUSE_API_KEY="your-key"
   
   # Create local config
   cp config.example.toml config.toml
   ```

2. **During Development**:
   ```bash
   # Test hooks before committing
   pre-commit run --all-files
   
   # Commit with descriptive message
   git commit -m "feat: add new feature"
   ```

3. **Before Pushing**:
   ```bash
   # Run full test suite
   cargo test --all-features
   
   # Run security audit
   cargo audit
   
   # Push changes
   git push
   ```

### Code Review Checklist

- [ ] No hardcoded credentials
- [ ] Environment variables used for secrets
- [ ] Configuration files validated
- [ ] Tests don't contain real data
- [ ] Documentation doesn't expose secrets
- [ ] Pre-commit hooks pass
- [ ] Security scan clean

### Regular Maintenance

```bash
# Weekly: Update dependencies
cargo update
cargo audit

# Monthly: Update pre-commit hooks
pre-commit autoupdate

# Quarterly: Rotate credentials
# Update all API keys and tokens
```

## Security Scanning Tools

### Integrated Tools

| Tool | Purpose | Command |
|------|---------|---------|
| detect-secrets | Secret detection | `detect-secrets scan` |
| gitleaks | Credential scanning | `gitleaks detect` |
| cargo-audit | Dependency vulnerabilities | `cargo audit` |
| cargo-clippy | Security linting | `cargo clippy` |
| Trivy | Container/model scanning | `trivy fs .` |

### Running Scans

```bash
# Full security scan
./scripts/security_scan.sh

# Individual scans
detect-secrets scan
gitleaks detect --verbose
cargo audit
trivy fs --security-checks vuln,config .
```

## Additional Resources

- [SECURITY.md](../SECURITY.md) - Security policy and reporting
- [.gitignore](./.gitignore) - Ignored files and patterns
- [.pre-commit-config.yaml](./.pre-commit-config.yaml) - Hook configuration
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CIS Benchmarks](https://www.cisecurity.org/cis-benchmarks/)

## Support

If you have questions or need help:

- **Documentation**: https://docs.fuse-project.io/security
- **Issues**: https://github.com/fuse/fuse/issues
- **Security**: security@fuse-project.io

---

**Remember**: Security is everyone's responsibility. When in doubt, ask!
