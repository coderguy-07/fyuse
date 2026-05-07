//! Memory-mapped GGUF model loader.
//!
//! Opens GGUF files using mmap for zero-copy access to tensor data.
//! The weights stay on disk until actually accessed, allowing fast "loading"
//! and efficient memory use via the OS page cache.

use crate::error::{FuseError, Result};
use crate::model::formats::gguf::{GgufFile, GgufTensorInfo};
use memmap2::Mmap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// A memory-mapped GGUF model file.
pub struct MmapModel {
    pub gguf: GgufFile,
    pub path: PathBuf,
    mmap: Mmap,
}

impl MmapModel {
    /// Open a GGUF file with memory mapping.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| {
            FuseError::InternalError(format!(
                "Failed to open model file {}: {}",
                path.display(),
                e
            ))
        })?;

        // Parse the GGUF header and metadata
        let mut reader = BufReader::new(&file);
        let gguf = GgufFile::parse(&mut reader)?;

        // Memory-map the entire file
        // SAFETY: The file is opened read-only and we don't modify it.
        // The mmap is valid for the lifetime of this struct.
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| {
                FuseError::InternalError(format!("Failed to mmap model file: {}", e))
            })?
        };

        Ok(Self {
            gguf,
            path: path.to_path_buf(),
            mmap,
        })
    }

    /// Get the raw bytes for a tensor (zero-copy slice into mmap).
    pub fn tensor_data(&self, tensor: &GgufTensorInfo) -> Result<&[u8]> {
        let start = (self.gguf.data_offset + tensor.offset) as usize;
        let size = tensor.size_bytes() as usize;
        let end = start + size;

        if end > self.mmap.len() {
            return Err(FuseError::InternalError(format!(
                "Tensor '{}' extends beyond file: offset={}, size={}, file_size={}",
                tensor.name,
                start,
                size,
                self.mmap.len()
            )));
        }

        Ok(&self.mmap[start..end])
    }

    /// Get tensor data by name.
    pub fn tensor_data_by_name(&self, name: &str) -> Result<&[u8]> {
        let tensor = self
            .gguf
            .tensors
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| FuseError::InternalError(format!("Tensor not found: {}", name)))?;
        self.tensor_data(tensor)
    }

    /// Total file size in bytes.
    pub fn file_size(&self) -> u64 {
        self.mmap.len() as u64
    }

    /// Number of tensors in the model.
    pub fn tensor_count(&self) -> usize {
        self.gguf.tensors.len()
    }
}

// MmapModel is Send+Sync because Mmap is Send+Sync
// and GgufFile is Clone+Send.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::formats::gguf::{GgmlType, GGUF_MAGIC, GGUF_VERSION_3};
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Create a complete test GGUF file with tensor data.
    fn create_test_gguf_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // Header
        file.write_all(&GGUF_MAGIC.to_le_bytes()).unwrap();
        file.write_all(&GGUF_VERSION_3.to_le_bytes()).unwrap();
        file.write_all(&1u64.to_le_bytes()).unwrap(); // 1 tensor
        file.write_all(&1u64.to_le_bytes()).unwrap(); // 1 metadata KV

        // Metadata: architecture = "test"
        let key = "general.architecture";
        file.write_all(&(key.len() as u64).to_le_bytes()).unwrap();
        file.write_all(key.as_bytes()).unwrap();
        file.write_all(&8u32.to_le_bytes()).unwrap(); // string type
        let val = "test";
        file.write_all(&(val.len() as u64).to_le_bytes()).unwrap();
        file.write_all(val.as_bytes()).unwrap();

        // Tensor info: "weights" shape [4], F32, offset 0
        let name = "weights";
        file.write_all(&(name.len() as u64).to_le_bytes()).unwrap();
        file.write_all(name.as_bytes()).unwrap();
        file.write_all(&1u32.to_le_bytes()).unwrap(); // 1 dimension
        file.write_all(&4u64.to_le_bytes()).unwrap(); // dim = 4
        file.write_all(&0u32.to_le_bytes()).unwrap(); // F32
        file.write_all(&0u64.to_le_bytes()).unwrap(); // offset 0

        // Pad to 32-byte alignment
        let pos = file.as_file().metadata().unwrap().len();
        let aligned = (pos + 31) & !31;
        let padding = aligned - pos;
        file.write_all(&vec![0u8; padding as usize]).unwrap();

        // Tensor data: 4 f32 values
        let values: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
        for v in &values {
            file.write_all(&v.to_le_bytes()).unwrap();
        }

        file.flush().unwrap();
        file
    }

    #[test]
    fn test_mmap_open() {
        let file = create_test_gguf_file();
        let model = MmapModel::open(file.path()).unwrap();

        assert_eq!(model.tensor_count(), 1);
        assert!(model.file_size() > 0);
        assert_eq!(model.gguf.architecture(), Some("test"));
    }

    #[test]
    fn test_mmap_tensor_data() {
        let file = create_test_gguf_file();
        let model = MmapModel::open(file.path()).unwrap();

        let data = model.tensor_data_by_name("weights").unwrap();
        // 4 f32 values = 16 bytes
        assert_eq!(data.len(), 16);

        // Verify the actual float values
        let values: Vec<f32> = data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        assert_eq!(values, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_mmap_tensor_not_found() {
        let file = create_test_gguf_file();
        let model = MmapModel::open(file.path()).unwrap();
        assert!(model.tensor_data_by_name("nonexistent").is_err());
    }

    #[test]
    fn test_mmap_open_nonexistent() {
        assert!(MmapModel::open(Path::new("/nonexistent/model.gguf")).is_err());
    }

    #[test]
    fn test_mmap_zero_copy() {
        let file = create_test_gguf_file();
        let model = MmapModel::open(file.path()).unwrap();

        // Get two slices - they should point into the same mmap
        let data1 = model.tensor_data_by_name("weights").unwrap();
        let data2 = model.tensor_data_by_name("weights").unwrap();

        // Same pointer = zero copy
        assert_eq!(data1.as_ptr(), data2.as_ptr());
    }

    #[test]
    fn test_tensor_size_f32() {
        let tensor = GgufTensorInfo {
            name: "test".to_string(),
            dimensions: vec![4],
            ggml_type: GgmlType::F32,
            offset: 0,
        };
        assert_eq!(tensor.size_bytes(), 16); // 4 * 4 bytes
    }
}
