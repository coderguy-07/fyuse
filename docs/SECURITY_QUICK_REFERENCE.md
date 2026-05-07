# Security Quick Reference

## 🚀 Quick Setup

```bash
./scripts/setup_hooks.sh
```

## ✅ Safe Practices

### Configuration
```toml
# ✅ DO THIS
api_key = "${FUSE_API_KEY}"

# ❌ NOT THIS
api_key = "sk-abc123"
```

### Environment Variables
```bash
# Set in ~/.bashrc or ~/.zshrc
export FUSE_API_KEY="your-key"
export HF_TOKEN="your-token"
```

## 🔍 Quick Checks

```bash
# Check all files
pre-commit run --all-files

# Check for secrets
detect-secrets scan

# Validate config
python scripts/validate_config.py config.toml

# Security audit
cargo audit
```

## 🚫 What's Blocked

- API keys and tokens
- Passwords and secrets
- Private keys (*.key, *.pem)
- Model files (*.bin, *.gguf)
- Database files (*.db, *.sqlite)
- Config files with credentials
- Large files (>1MB)

## 🆘 If Hooks Fail

1. **Read the error message** - It tells you what's wrong
2. **Remove sensitive data** - Use environment variables
3. **Run hooks again** - `pre-commit run --all-files`
4. **Commit** - `git commit -m "message"`

## 📞 Need Help?

- **Docs**: [SECURITY_SETUP.md](./SECURITY_SETUP.md)
- **Policy**: [SECURITY.md](../SECURITY.md)
- **Issues**: GitHub Issues
- **Security**: security@fuse-project.io

## 🔑 Key Commands

| Command | Purpose |
|---------|---------|
| `./scripts/setup_hooks.sh` | Initial setup |
| `pre-commit run --all-files` | Check all files |
| `git commit --no-verify` | Skip hooks (not recommended) |
| `cargo audit` | Security audit |
| `detect-secrets scan` | Find secrets |

## 💡 Remember

- **Never** commit real credentials
- **Always** use environment variables
- **Check** before you commit
- **Rotate** credentials if exposed
