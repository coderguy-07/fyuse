use crate::error::Result;
use crate::rag::RAGService;
use std::path::Path;
use tracing::{error, info};

pub async fn handle_learn(path: &Path, verbose: bool, _force: bool) -> Result<()> {
    info!("Starting repository indexing for: {}", path.display());

    if !path.exists() {
        error!("Path does not exist: {}", path.display());
        return Err(crate::error::FuseError::ValidationError(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    // Create RAG service
    let index_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".fuse")
        .join("index");

    let mut rag_service = RAGService::new(&index_path)?;

    if verbose {
        println!("📚 Indexing repository: {}", path.display());
        println!("📁 Index location: {}", index_path.display());
    }

    // Index the repository
    rag_service.index_repository(path).await?;

    if verbose {
        println!("✅ Repository indexed successfully!");
        println!("💡 You can now use this context in your conversations");
    }

    info!("Repository indexing complete");
    Ok(())
}
