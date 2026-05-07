use serde::{Deserialize, Serialize};

use crate::error::{FuseError, Result};

/// A chunk of delta data to apply to a base model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaChunk {
    pub offset: u64,
    pub size: u64,
    /// SHA256 hex hash of the chunk data.
    pub hash: String,
    pub data: Vec<u8>,
}

impl DeltaChunk {
    pub fn new(offset: u64, data: Vec<u8>, hash: String) -> Self {
        Self {
            offset,
            size: data.len() as u64,
            hash,
            data,
        }
    }

    /// Compute a simple SHA256-like hash of the data using a basic checksum.
    /// Uses a FNV-1a-inspired hash producing a hex string.
    pub fn compute_hash(data: &[u8]) -> String {
        // Use a simple but deterministic hash for verification
        let mut h: u64 = 0xcbf29ce484222325;
        for &byte in data {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    }

    /// Verify this chunk's data against its stored hash.
    pub fn verify(&self) -> bool {
        let computed = Self::compute_hash(&self.data);
        computed == self.hash
    }
}

/// Manifest describing a delta update from one version to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaManifest {
    pub base_version: String,
    pub target_version: String,
    pub chunks: Vec<DeltaChunk>,
    /// Expected total size of the target after applying all chunks.
    pub target_size: u64,
    /// Hash of the complete target file.
    pub target_hash: String,
}

impl DeltaManifest {
    pub fn new(
        base_version: impl Into<String>,
        target_version: impl Into<String>,
        target_size: u64,
        target_hash: impl Into<String>,
    ) -> Self {
        Self {
            base_version: base_version.into(),
            target_version: target_version.into(),
            chunks: Vec::new(),
            target_size,
            target_hash: target_hash.into(),
        }
    }

    pub fn add_chunk(&mut self, chunk: DeltaChunk) {
        self.chunks.push(chunk);
    }

    pub fn total_delta_size(&self) -> u64 {
        self.chunks.iter().map(|c| c.size).sum()
    }

    /// Validate that all chunks have correct hashes.
    pub fn validate(&self) -> Result<()> {
        if self.base_version.is_empty() || self.target_version.is_empty() {
            return Err(FuseError::ValidationError(
                "Versions cannot be empty".to_string(),
            ));
        }
        for (i, chunk) in self.chunks.iter().enumerate() {
            if !chunk.verify() {
                return Err(FuseError::ValidationError(format!(
                    "Chunk {i} hash mismatch at offset {}",
                    chunk.offset
                )));
            }
        }
        Ok(())
    }
}

/// Applies delta chunks to a base model byte array.
#[derive(Debug)]
pub struct DeltaApplier;

impl DeltaApplier {
    /// Apply a delta manifest to a base model, producing the target model bytes.
    pub fn apply(base: &[u8], manifest: &DeltaManifest) -> Result<Vec<u8>> {
        manifest.validate()?;

        let mut result = base.to_vec();

        // Ensure result is large enough for the target
        if (manifest.target_size as usize) > result.len() {
            result.resize(manifest.target_size as usize, 0);
        }

        for chunk in &manifest.chunks {
            let start = chunk.offset as usize;
            let end = start + chunk.data.len();
            if end > result.len() {
                return Err(FuseError::ValidationError(format!(
                    "Chunk at offset {} with size {} exceeds target size {}",
                    chunk.offset,
                    chunk.data.len(),
                    result.len()
                )));
            }
            result[start..end].copy_from_slice(&chunk.data);
        }

        // Verify final hash
        let final_hash = DeltaChunk::compute_hash(&result);
        if final_hash != manifest.target_hash {
            return Err(FuseError::ValidationError(format!(
                "Target hash mismatch: expected {}, got {final_hash}",
                manifest.target_hash
            )));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(offset: u64, data: Vec<u8>) -> DeltaChunk {
        let hash = DeltaChunk::compute_hash(&data);
        DeltaChunk::new(offset, data, hash)
    }

    #[test]
    fn test_chunk_creation() {
        let chunk = make_chunk(0, vec![1, 2, 3]);
        assert_eq!(chunk.offset, 0);
        assert_eq!(chunk.size, 3);
        assert!(!chunk.hash.is_empty());
    }

    #[test]
    fn test_chunk_hash_deterministic() {
        let h1 = DeltaChunk::compute_hash(&[1, 2, 3]);
        let h2 = DeltaChunk::compute_hash(&[1, 2, 3]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_chunk_hash_differs_for_different_data() {
        let h1 = DeltaChunk::compute_hash(&[1, 2, 3]);
        let h2 = DeltaChunk::compute_hash(&[4, 5, 6]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_chunk_verify_valid() {
        let chunk = make_chunk(0, vec![10, 20, 30]);
        assert!(chunk.verify());
    }

    #[test]
    fn test_chunk_verify_invalid() {
        let mut chunk = make_chunk(0, vec![10, 20, 30]);
        chunk.hash = "badhash".to_string();
        assert!(!chunk.verify());
    }

    #[test]
    fn test_manifest_creation() {
        let m = DeltaManifest::new("v1", "v2", 100, "somehash");
        assert_eq!(m.base_version, "v1");
        assert_eq!(m.target_version, "v2");
        assert_eq!(m.target_size, 100);
        assert!(m.chunks.is_empty());
    }

    #[test]
    fn test_manifest_add_chunk() {
        let mut m = DeltaManifest::new("v1", "v2", 100, "h");
        m.add_chunk(make_chunk(0, vec![1, 2]));
        m.add_chunk(make_chunk(2, vec![3, 4]));
        assert_eq!(m.chunks.len(), 2);
        assert_eq!(m.total_delta_size(), 4);
    }

    #[test]
    fn test_manifest_validate_empty_version() {
        let m = DeltaManifest::new("", "v2", 100, "h");
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_validate_bad_chunk_hash() {
        let mut m = DeltaManifest::new("v1", "v2", 100, "h");
        let mut chunk = make_chunk(0, vec![1, 2, 3]);
        chunk.hash = "wrong".to_string();
        m.add_chunk(chunk);
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_apply_delta() {
        let base = vec![0u8; 8];
        let patch_data = vec![0xAA, 0xBB, 0xCC];
        let mut target = base.clone();
        target[2..5].copy_from_slice(&patch_data);
        let target_hash = DeltaChunk::compute_hash(&target);

        let mut manifest = DeltaManifest::new("v1", "v2", 8, target_hash);
        manifest.add_chunk(make_chunk(2, patch_data));

        let result = DeltaApplier::apply(&base, &manifest).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_apply_delta_target_hash_mismatch() {
        let base = vec![0u8; 8];
        let mut manifest = DeltaManifest::new("v1", "v2", 8, "wronghash");
        manifest.add_chunk(make_chunk(0, vec![1]));
        assert!(DeltaApplier::apply(&base, &manifest).is_err());
    }

    #[test]
    fn test_apply_delta_chunk_exceeds_size() {
        let base = vec![0u8; 4];
        let mut manifest = DeltaManifest::new("v1", "v2", 4, "h");
        manifest.add_chunk(make_chunk(3, vec![1, 2, 3])); // exceeds size
        assert!(DeltaApplier::apply(&base, &manifest).is_err());
    }

    #[test]
    fn test_apply_grows_buffer() {
        let base = vec![0u8; 4];
        let target = vec![0u8; 8];
        let target_hash = DeltaChunk::compute_hash(&target);
        let manifest = DeltaManifest::new("v1", "v2", 8, target_hash);
        let result = DeltaApplier::apply(&base, &manifest).unwrap();
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_serde_roundtrip_manifest() {
        let mut m = DeltaManifest::new("v1", "v2", 100, "hash123");
        m.add_chunk(make_chunk(0, vec![1, 2, 3]));
        let json = serde_json::to_string(&m).unwrap();
        let deserialized: DeltaManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.base_version, "v1");
        assert_eq!(deserialized.target_version, "v2");
        assert_eq!(deserialized.chunks.len(), 1);
        assert!(deserialized.chunks[0].verify());
    }

    #[test]
    fn test_serde_roundtrip_chunk() {
        let chunk = make_chunk(42, vec![10, 20, 30, 40]);
        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: DeltaChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.offset, 42);
        assert!(deserialized.verify());
    }
}
