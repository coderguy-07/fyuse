# Design Document

## Overview

This design addresses the directory management confusion, CLI verbosity, and missing production-grade features in the Fuse AI model management platform. The solution involves:

1. **Directory Restructuring**: Rename global directory from `~/.fuse` to `~/.fuse_cli` to eliminate confusion with project-level `.fuse` directories
2. **Context Analysis**: Implement `fuse read` command to analyze project structure and documentation
3. **CLI Modernization**: Restructure commands to be more concise and follow modern CLI design patterns
4. **Production Features**: Add shell completions, better help system, and command aliases

## Architecture

### Directory Structure

```
~/.fuse_cli/                    # Global CLI directory (renamed from ~/.fuse)
├── config.toml                 # Global configuration
├── config.toml.example         # Example configuration
├── config.yaml.example         # YAML example
├── models/                     # Downloaded models
├── cache/                      # Cache directory
└── logs/                       # Log files

./.fuse/                        # Project-specific directory
├── config.toml                 # Project config (optional)
├── context.json                # Project context from 'fuse read'
├── specs/                      # Spec-driven development
│   └── fuse.md                 # Workflow definitions
├── report/                     # Generated reports
│   ├── compatibility/
│   ├── scan/
│   ├── inspect/
│   └── validation/
└── vibe/                       # Workflow execution logs
```

### Configuration Priority

1. Project-specific config (`./.fuse/config.toml`) - highest priority
2. Global config (`~/.fuse_cli/config.toml`)
3. Environment variables (`FUSE_*`)
4. Default values - lowest priority

## Components and Interfaces

### 1. Directory Manager

**Purpose**: Manage directory paths and configuration loading with proper priority

**Interface**:
```rust
pub struct DirectoryManager {
    global_dir: PathBuf,
    project_dir: Option<PathBuf>,
}

impl DirectoryManager {
    pub fn new() -> Result<Self>;
    pub fn global_dir(&self) -> &Path;
    pub fn project_dir(&self) -> Option<&Path>;
    pub fn ensure_global_dir(&self) -> Result<()>;
    pub fn ensure_project_dir(&self) -> Result<()>;
    pub fn find_config(&self) -> Result<PathBuf>;
    pub fn migrate_old_directory(&self) -> Result<()>;
}
```

**Responsibilities**:
- Detect and create `~/.fuse_cli` directory
- Detect project-level `./.fuse` directory
- Migrate existing `~/.fuse` to `~/.fuse_cli` if found
- Resolve configuration file with proper priority
- Create directories on-demand

### 2. Context Analyzer

**Purpose**: Implement `fuse read` command to analyze project structure

**Interface**:
```rust
pub struct ContextAnalyzer {
    root_path: PathBuf,
}

pub struct ProjectContext {
    pub name: String,
    pub description: Option<String>,
    pub tech_stack: Vec<String>,
    pub entry_points: Vec<PathBuf>,
    pub dependencies: HashMap<String, String>,
    pub documentation: Vec<DocumentSummary>,
    pub git_info: Option<GitInfo>,
}

impl ContextAnalyzer {
    pub fn new(root_path: PathBuf) -> Self;
    pub async fn analyze(&self) -> Result<ProjectContext>;
    pub fn save_context(&self, context: &ProjectContext) -> Result<()>;
    pub fn load_context(&self) -> Result<Option<ProjectContext>>;
}
```

**Analysis Steps**:
1. Scan for documentation files (README.md, CONTRIBUTING.md, ARCHITECTURE.md, etc.)
2. Parse package manifests (Cargo.toml, package.json, pyproject.toml, go.mod, etc.)
3. Analyze git repository (if present)
4. Identify entry points (main.rs, index.js, __main__.py, etc.)
5. Extract dependencies and tech stack
6. Generate summary and save to `./.fuse/context.json`

### 3. CLI Command Restructuring

**Current Structure** (verbose):
```
fuse pull <model>
fuse list
fuse rm <model>
fuse inspect <model>
fuse quantize <model>
fuse layer inspect <model>
fuse comp-check <models>
fuse merge <models>
fuse scan <model>
```

**New Structure** (concise and grouped):
```
# Model operations
fuse model pull <model>        # or: fuse get <model>
fuse model list                # or: fuse ls
fuse model rm <model>          # or: fuse rm <model>
fuse model info <model>        # or: fuse info <model>
fuse model run <model>         # or: fuse run <model>

# Advanced model operations
fuse model quantize <model>
fuse model merge <models>
fuse model scan <model>

# Layer operations
fuse layer ls <model>          # or: fuse layer inspect <model>
fuse layer rm <model> <layer>
fuse layer add <model> <layer>

# Compatibility
fuse compat check <models>     # or: fuse comp <models>

# Project operations
fuse read                      # NEW: Analyze project context
fuse init                      # Initialize project config

# System operations
fuse status                    # NEW: Show system status
fuse version                   # NEW: Show version info
fuse config                    # Show/edit configuration
```

### 4. Command Alias System

**Interface**:
```rust
pub struct CommandAlias {
    pub short: &'static str,
    pub long: &'static str,
    pub description: &'static str,
}

pub struct AliasResolver;

impl AliasResolver {
    pub fn resolve(input: &str) -> Option<String>;
    pub fn list_aliases() -> Vec<CommandAlias>;
}
```

**Aliases**:
- `fuse get` → `fuse model pull`
- `fuse ls` → `fuse model list`
- `fuse rm` → `fuse model rm`
- `fuse info` → `fuse model info`
- `fuse run` → `fuse model run`
- `fuse comp` → `fuse compat check`

### 5. Shell Completion Generator

**Interface**:
```rust
pub struct CompletionGenerator;

impl CompletionGenerator {
    pub fn generate_bash() -> String;
    pub fn generate_zsh() -> String;
    pub fn generate_fish() -> String;
    pub fn generate_powershell() -> String;
    pub fn install(shell: Shell) -> Result<()>;
}
```

**Usage**:
```bash
# Generate completions
fuse completion bash > /etc/bash_completion.d/fuse
fuse completion zsh > ~/.zsh/completions/_fuse
fuse completion fish > ~/.config/fish/completions/fuse.fish

# Or auto-install
fuse completion install bash
```

### 6. Status Command

**Interface**:
```rust
pub struct SystemStatus {
    pub version: String,
    pub config_path: PathBuf,
    pub global_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
    pub models_count: usize,
    pub cache_size: u64,
    pub active_features: Vec<String>,
}

impl SystemStatus {
    pub fn collect() -> Result<Self>;
    pub fn display(&self, format: OutputFormat);
}
```

**Output Example**:
```
Fuse Status
───────────────────────────────────────
Version:        0.1.0
Config:         ~/.fuse_cli/config.toml
Global Dir:     ~/.fuse_cli
Project Dir:    ./.fuse
Models:         5 installed
Cache Size:     2.3 GB
Active Features:
  ✓ agentic-coding
  ✓ vulnerability-scanning
  ✗ thinking-visualization
```

## Data Models

### ProjectContext

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectContext {
    pub name: String,
    pub description: Option<String>,
    pub tech_stack: Vec<String>,
    pub entry_points: Vec<PathBuf>,
    pub dependencies: HashMap<String, String>,
    pub documentation: Vec<DocumentSummary>,
    pub git_info: Option<GitInfo>,
    pub analyzed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub path: PathBuf,
    pub title: Option<String>,
    pub summary: String,
    pub sections: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitInfo {
    pub remote_url: Option<String>,
    pub current_branch: String,
    pub commit_count: usize,
    pub contributors: Vec<String>,
}
```

### Configuration with Environment Variables

```rust
impl FuseConfig {
    pub fn load_with_priority() -> Result<Self> {
        let dir_manager = DirectoryManager::new()?;
        
        // 1. Try project config
        if let Some(project_dir) = dir_manager.project_dir() {
            let project_config = project_dir.join("config.toml");
            if project_config.exists() {
                return Self::from_file(&project_config);
            }
        }
        
        // 2. Try global config
        let global_config = dir_manager.global_dir().join("config.toml");
        if global_config.exists() {
            let mut config = Self::from_file(&global_config)?;
            config.apply_env_overrides();
            return Ok(config);
        }
        
        // 3. Use defaults with env overrides
        let mut config = Self::default();
        config.apply_env_overrides();
        Ok(config)
    }
    
    fn apply_env_overrides(&mut self) {
        if let Ok(val) = env::var("FUSE_LOG_LEVEL") {
            self.log_level = val;
        }
        if let Ok(val) = env::var("FUSE_MODELS_DIR") {
            self.models_dir = PathBuf::from(val);
        }
        // ... more overrides
    }
}
```

## Error Handling

### Enhanced Error Messages

```rust
pub enum FuseError {
    DirectoryMigrationFailed {
        old_path: PathBuf,
        new_path: PathBuf,
        source: io::Error,
    },
    ContextAnalysisFailed {
        path: PathBuf,
        reason: String,
    },
    InvalidCommand {
        input: String,
        suggestions: Vec<String>,
    },
    // ... existing errors
}

impl Display for FuseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::DirectoryMigrationFailed { old_path, new_path, source } => {
                write!(f, "Failed to migrate {} to {}: {}\n\n", 
                    old_path.display(), new_path.display(), source)?;
                write!(f, "Suggestion: Check file permissions and try:\n")?;
                write!(f, "  mv {} {}", old_path.display(), new_path.display())
            }
            Self::InvalidCommand { input, suggestions } => {
                write!(f, "Unknown command: '{}'\n\n", input)?;
                if !suggestions.is_empty() {
                    write!(f, "Did you mean:\n")?;
                    for suggestion in suggestions {
                        write!(f, "  fuse {}\n", suggestion)?;
                    }
                }
                write!(f, "\nRun 'fuse --help' for usage")
            }
            // ... other errors
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **DirectoryManager Tests**
   - Test global directory creation
   - Test project directory detection
   - Test configuration priority resolution
   - Test migration from old directory

2. **ContextAnalyzer Tests**
   - Test documentation parsing
   - Test dependency extraction
   - Test git info extraction
   - Test context serialization

3. **AliasResolver Tests**
   - Test alias resolution
   - Test invalid alias handling
   - Test alias listing

### Integration Tests

1. **End-to-End Directory Migration**
   - Create old `~/.fuse` directory
   - Run migration
   - Verify new `~/.fuse_cli` structure
   - Verify configuration preserved

2. **Context Analysis Workflow**
   - Create test project structure
   - Run `fuse read`
   - Verify context.json created
   - Verify context accuracy

3. **CLI Command Tests**
   - Test all command aliases
   - Test help system
   - Test error messages
   - Test shell completions

## Migration Strategy

### Automatic Migration

When Fuse detects an existing `~/.fuse` directory:

1. Display migration prompt:
   ```
   Found existing Fuse directory at ~/.fuse
   
   To avoid confusion with project-level .fuse directories,
   we're moving global configuration to ~/.fuse_cli
   
   This will:
   - Move ~/.fuse to ~/.fuse_cli
   - Update all internal references
   - Preserve all your models and configuration
   
   Continue? (Y/n)
   ```

2. Perform migration:
   - Rename `~/.fuse` to `~/.fuse_cli`
   - Update any absolute paths in configuration
   - Create symlink from `~/.fuse` to `~/.fuse_cli` (optional, for compatibility)

3. Display success message:
   ```
   ✓ Migration complete!
   
   Your global Fuse directory is now at: ~/.fuse_cli
   All your models and configuration have been preserved.
   ```

### Manual Migration

Users can also manually migrate:
```bash
fuse migrate --from ~/.fuse --to ~/.fuse_cli
```

## Implementation Notes

### Phase 1: Directory Management
- Implement DirectoryManager
- Add migration logic
- Update all path references in codebase

### Phase 2: Context Analysis
- Implement ContextAnalyzer
- Add `fuse read` command
- Add context storage and retrieval

### Phase 3: CLI Restructuring
- Implement command aliases
- Add new command structure
- Maintain backward compatibility

### Phase 4: Production Features
- Add shell completions
- Implement status command
- Add environment variable support
- Enhance help system

### Backward Compatibility

To maintain compatibility during transition:
- Keep old command structure working with deprecation warnings
- Support both `~/.fuse` and `~/.fuse_cli` with automatic migration
- Provide clear migration path in documentation
