use crate::error::Result;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

pub async fn discover_workflow_file(base_dir: &Path) -> Result<Option<PathBuf>> {
    // Priority order for workflow file discovery
    let search_paths = vec![
        base_dir.join(".fuse/specs/fuse.md"),
        base_dir.join(".cursor/specs/cursor.md"),
        base_dir.join(".cursor/claude.md"),
        base_dir.join(".kiro/specs/workflow.md"),
        base_dir.join(".windsurf/specs/windsurf.md"),
    ];

    for path in search_paths {
        debug!("Checking for workflow file: {}", path.display());
        if path.exists() {
            info!("Found workflow file: {}", path.display());
            return Ok(Some(path));
        }
    }

    info!("No workflow file found, will create default");
    Ok(None)
}

pub async fn initialize_fuse_structure(base_dir: &Path) -> Result<PathBuf> {
    let fuse_dir = base_dir.join(".fuse");
    let specs_dir = fuse_dir.join("specs");
    let vibe_dir = fuse_dir.join("vibe");

    fs::create_dir_all(&specs_dir).await?;
    fs::create_dir_all(&vibe_dir).await?;

    let fuse_md_path = specs_dir.join("fuse.md");
    if !fuse_md_path.exists() {
        create_default_fuse_md(&fuse_md_path).await?;
    }

    Ok(fuse_md_path)
}

async fn create_default_fuse_md(path: &Path) -> Result<()> {
    let default_content = r#"# Fuse Workflow

## Overview
This file defines automated workflows for the Fuse AI platform.

## Workflow: Fix-Compile-Test

### Steps

#### 1. Analyze Error
- Read compilation or test error
- Identify root cause
- Plan fix strategy

#### 2. Apply Fix
- Modify source files based on analysis
- Validate syntax
- Ensure code quality

#### 3. Compile
- Run `cargo build`
- If error, goto step 1 (max 5 iterations)
- If success, proceed to step 4

#### 4. Test
- Run `cargo test`
- If failure, goto step 1 (max 3 iterations)
- If success, proceed to step 5

#### 5. Complete
- Generate summary of changes
- Update vibe log
- Report success

## Configuration

- Max iterations: 10
- Timeout: 1 hour
- Auto-commit: false
"#;

    fs::write(path, default_content).await?;
    info!("Created default fuse.md at {}", path.display());
    Ok(())
}
