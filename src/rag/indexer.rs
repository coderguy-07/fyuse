use crate::error::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

pub struct IndexConfig {
    pub ignore_patterns: Vec<String>,
    pub max_file_size: usize,
    pub supported_extensions: HashSet<String>,
}

impl Default for IndexConfig {
    fn default() -> Self {
        let mut supported_extensions = HashSet::new();
        supported_extensions.insert("rs".to_string());
        supported_extensions.insert("py".to_string());
        supported_extensions.insert("js".to_string());
        supported_extensions.insert("ts".to_string());
        supported_extensions.insert("java".to_string());
        supported_extensions.insert("cpp".to_string());
        supported_extensions.insert("c".to_string());
        supported_extensions.insert("go".to_string());
        supported_extensions.insert("md".to_string());
        supported_extensions.insert("txt".to_string());

        Self {
            ignore_patterns: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            supported_extensions,
        }
    }
}

pub struct RepositoryIndexer {
    #[allow(dead_code)]
    index_path: PathBuf,
    config: IndexConfig,
}

impl RepositoryIndexer {
    pub fn new(index_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_path)?;

        Ok(Self {
            index_path: index_path.to_path_buf(),
            config: IndexConfig::default(),
        })
    }

    pub async fn index_directory(&self, repo_path: impl AsRef<Path>) -> Result<()> {
        let repo_path = repo_path.as_ref();
        info!("Indexing repository: {}", repo_path.display());

        let files = self.discover_files(repo_path).await?;
        info!("Found {} files to index", files.len());

        for file_path in files {
            if let Err(e) = self.index_file(&file_path).await {
                warn!("Failed to index {}: {}", file_path.display(), e);
            }
        }

        info!("Repository indexing complete");
        Ok(())
    }

    async fn discover_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];

        while let Some(path) = stack.pop() {
            if self.should_ignore(&path) {
                continue;
            }

            let metadata = fs::metadata(&path).await?;

            if metadata.is_dir() {
                let mut entries = fs::read_dir(&path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    stack.push(entry.path());
                }
            } else if metadata.is_file() && self.should_index_file(&path) {
                files.push(path);
            }
        }

        Ok(files)
    }

    fn should_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.config.ignore_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }

        false
    }

    fn should_index_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return self.config.supported_extensions.contains(ext_str);
            }
        }
        false
    }

    async fn index_file(&self, file_path: &Path) -> Result<()> {
        debug!("Indexing file: {}", file_path.display());

        let metadata = fs::metadata(file_path).await?;
        if metadata.len() > self.config.max_file_size as u64 {
            debug!("Skipping large file: {}", file_path.display());
            return Ok(());
        }

        let content = fs::read_to_string(file_path).await?;

        // TODO: Generate embeddings and store in vector database
        // For now, just log that we would index it
        debug!(
            "Would generate embeddings for {} ({} bytes)",
            file_path.display(),
            content.len()
        );

        Ok(())
    }

    pub async fn update_files(&self, files: Vec<impl AsRef<Path>>) -> Result<()> {
        info!("Updating {} files in index", files.len());

        for file_path in files {
            let path = file_path.as_ref();
            if self.should_index_file(path) {
                if let Err(e) = self.index_file(path).await {
                    warn!("Failed to update {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_indexer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = RepositoryIndexer::new(temp_dir.path());
        assert!(indexer.is_ok());
    }

    #[test]
    fn test_should_ignore() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = RepositoryIndexer::new(temp_dir.path()).unwrap();

        assert!(indexer.should_ignore(Path::new("node_modules/package")));
        assert!(indexer.should_ignore(Path::new("target/debug")));
        assert!(!indexer.should_ignore(Path::new("src/main.rs")));
    }

    #[test]
    fn test_should_index_file() {
        let temp_dir = TempDir::new().unwrap();
        let indexer = RepositoryIndexer::new(temp_dir.path()).unwrap();

        assert!(indexer.should_index_file(Path::new("main.rs")));
        assert!(indexer.should_index_file(Path::new("script.py")));
        assert!(!indexer.should_index_file(Path::new("binary.exe")));
        assert!(!indexer.should_index_file(Path::new("image.png")));
    }
}
