pub mod embeddings;
pub mod indexer;
pub mod memory;
pub mod retriever;

pub use embeddings::{Embedding, EmbeddingGenerator};
pub use indexer::{IndexConfig, RepositoryIndexer};
pub use memory::{ConversationMemory, MemoryConfig, MemoryEntry, MemorySearchResult};
pub use retriever::{ContextRetriever, RetrievalResult};

use crate::error::Result;
use std::path::Path;

/// RAG Service for repository-aware context retrieval
pub struct RAGService {
    indexer: RepositoryIndexer,
    retriever: ContextRetriever,
}

impl RAGService {
    pub fn new(index_path: impl AsRef<Path>) -> Result<Self> {
        let indexer = RepositoryIndexer::new(index_path.as_ref())?;
        let retriever = ContextRetriever::new(index_path.as_ref())?;

        Ok(Self { indexer, retriever })
    }

    /// Index a repository
    pub async fn index_repository(&mut self, repo_path: impl AsRef<Path>) -> Result<()> {
        self.indexer.index_directory(repo_path).await
    }

    /// Query for relevant context
    pub async fn query(&self, query: &str, k: usize) -> Result<Vec<RetrievalResult>> {
        self.retriever.retrieve(query, k).await
    }

    /// Update index incrementally
    pub async fn update_index(&mut self, changed_files: Vec<impl AsRef<Path>>) -> Result<()> {
        self.indexer.update_files(changed_files).await
    }
}
