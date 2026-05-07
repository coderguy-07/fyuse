# Phase 10: Agent Harness Design Document

> Inspired by [ultraworkers/claw-code](https://github.com/ultraworkers/claw-code).
> Adapted to Fuse's trait-first, config-driven, TDD architecture.

## Overview

The Agent Harness adds production-grade orchestration to Fuse's agent framework (Phase 8).
It provides: worker lifecycle management, persistent sessions, structured tasks, failure recovery,
sandboxed execution, and validated command execution.

## Architecture

```
src/agents/
├── mod.rs              # Updated: re-exports new modules
├── traits.rs           # Existing: Skill trait
├── mcp.rs              # Existing: MCP server + client
├── swarm.rs            # Existing: Multi-agent orchestration
├── worker.rs           # NEW [10.1]: Worker boot state machine
├── session.rs          # NEW [10.2]: Persistent session management
├── task_packet.rs      # NEW [10.3]: Typed task packet format
├── failure.rs          # NEW [10.4]: Failure taxonomy
├── recovery.rs         # NEW [10.4]: Recovery recipes
├── permissions.rs      # NEW [10.5]: Permission system
├── sandbox.rs          # NEW [10.5]: Sandbox validator
└── bash_validator.rs   # NEW [10.6]: Bash command validation
```

## Design Decisions

### 1. Worker State Machine [10.1]

Uses a Rust enum with `#[non_exhaustive]` for future states.
Transitions validated at compile time where possible, runtime otherwise.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum WorkerState {
    Spawning,
    TrustRequired { prompt: String },
    ReadyForPrompt,
    PromptAccepted { task_id: String },
    Running { task_id: String, started_at: DateTime<Utc> },
    Finished { task_id: String, result: WorkerResult },
    Failed { error: WorkerFailure },
}
```

### 2. Session Store [10.2]

Uses redb (already in deps) for persistence. Sessions are append-only logs
with periodic compaction. Token counting built into message append.

### 3. Permission Tiers [10.5]

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    ReadOnly,        // Read files, web access only
    WorkspaceWrite,  // Add file modifications within workspace
    FullAccess,      // Unrestricted
}
```

### 4. Bash Validation [10.6]

Chain of validators pattern. Each validator returns `Allow`, `Deny(reason)`, or `Warn(reason)`.

```rust
pub trait BashRule: Send + Sync {
    fn name(&self) -> &str;
    fn validate(&self, command: &str, mode: PermissionMode) -> RuleVerdict;
}
```

## Config Integration

```toml
[agents.worker]
state_file = ".fuse/worker-state.json"
trust_timeout_secs = 300

[agents.session]
storage = "redb"          # or "json"
compaction_threshold = 100 # messages before auto-compact
max_sessions = 50

[agents.permissions]
default_mode = "workspace-write"
workspace_root = "."
allowed_paths = []
denied_commands = ["rm -rf /", "mkfs", "dd if="]

[agents.bash_validation]
enabled = true
block_destructive = true
```
