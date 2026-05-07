//! GGUF format parser — read-only parsing of GGUF v3 model files.
//!
//! GGUF (GGML Universal File) format stores quantized model weights with metadata.
//! Spec: https://github.com/ggerganov/ggml/blob/master/docs/gguf.md

use crate::error::{FuseError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek};

/// GGUF magic number: "GGUF" in little-endian.
pub const GGUF_MAGIC: u32 = 0x46475547; // "GGUF"

/// Supported GGUF versions.
pub const GGUF_VERSION_2: u32 = 2;
pub const GGUF_VERSION_3: u32 = 3;

/// GGUF metadata value types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GgufValue {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Array(Vec<GgufValue>),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
}

/// GGUF quantization types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum GgmlType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2K = 10,
    Q3K = 11,
    Q4K = 12,
    Q5K = 13,
    Q6K = 14,
    Q8K = 15,
    Iq2Xxs = 16,
    Iq2Xs = 17,
    Iq3Xxs = 18,
    Iq1S = 19,
    Iq4Nl = 20,
    Iq3S = 21,
    Iq2S = 22,
    Iq4Xs = 23,
    I8 = 24,
    I16 = 25,
    I32 = 26,
    I64 = 27,
    F64 = 28,
    Iq1M = 29,
}

impl GgmlType {
    pub fn from_u32(v: u32) -> Result<Self> {
        match v {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            9 => Ok(Self::Q8_1),
            10 => Ok(Self::Q2K),
            11 => Ok(Self::Q3K),
            12 => Ok(Self::Q4K),
            13 => Ok(Self::Q5K),
            14 => Ok(Self::Q6K),
            15 => Ok(Self::Q8K),
            16 => Ok(Self::Iq2Xxs),
            17 => Ok(Self::Iq2Xs),
            18 => Ok(Self::Iq3Xxs),
            19 => Ok(Self::Iq1S),
            20 => Ok(Self::Iq4Nl),
            21 => Ok(Self::Iq3S),
            22 => Ok(Self::Iq2S),
            23 => Ok(Self::Iq4Xs),
            24 => Ok(Self::I8),
            25 => Ok(Self::I16),
            26 => Ok(Self::I32),
            27 => Ok(Self::I64),
            28 => Ok(Self::F64),
            29 => Ok(Self::Iq1M),
            _ => Err(FuseError::InternalError(format!(
                "Unknown GGML type: {}",
                v
            ))),
        }
    }

    /// Size in bytes per element (for non-quantized types).
    pub fn element_size(&self) -> Option<usize> {
        match self {
            Self::F32 => Some(4),
            Self::F16 => Some(2),
            Self::F64 => Some(8),
            Self::I8 => Some(1),
            Self::I16 => Some(2),
            Self::I32 => Some(4),
            Self::I64 => Some(8),
            _ => None, // Quantized types have block sizes
        }
    }

    /// Block size for quantized types.
    pub fn block_size(&self) -> usize {
        match self {
            Self::F32 | Self::F16 | Self::F64 => 1,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => 1,
            Self::Q4_0 | Self::Q4_1 => 32,
            Self::Q5_0 | Self::Q5_1 => 32,
            Self::Q8_0 | Self::Q8_1 => 32,
            Self::Q2K | Self::Q3K | Self::Q4K | Self::Q5K | Self::Q6K | Self::Q8K => 256,
            _ => 32, // Default for IQ types
        }
    }

    /// Bytes per block for quantized types.
    pub fn bytes_per_block(&self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
            Self::F64 => 8,
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 4,
            Self::I64 => 8,
            Self::Q4_0 => 18, // 32 values: 16 bytes data + 2 bytes scale
            Self::Q4_1 => 20,
            Self::Q5_0 => 22,
            Self::Q5_1 => 24,
            Self::Q8_0 => 34,
            Self::Q8_1 => 36,
            Self::Q2K => 84,
            Self::Q3K => 110,
            Self::Q4K => 144,
            Self::Q5K => 176,
            Self::Q6K => 210,
            Self::Q8K => 292,
            _ => 34, // Approximate for IQ types
        }
    }
}

/// GGUF file header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufHeader {
    pub magic: u32,
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
}

/// Metadata key-value store.
pub type GgufMetadata = HashMap<String, GgufValue>;

/// Tensor descriptor within a GGUF file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufTensorInfo {
    pub name: String,
    pub dimensions: Vec<u64>,
    pub ggml_type: GgmlType,
    pub offset: u64,
}

impl GgufTensorInfo {
    /// Total number of elements in this tensor.
    pub fn n_elements(&self) -> u64 {
        self.dimensions.iter().product()
    }

    /// Estimated size in bytes.
    pub fn size_bytes(&self) -> u64 {
        let n = self.n_elements() as usize;
        let blocks = n.div_ceil(self.ggml_type.block_size());
        (blocks * self.ggml_type.bytes_per_block()) as u64
    }
}

/// Parsed GGUF file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufFile {
    pub header: GgufHeader,
    pub metadata: GgufMetadata,
    pub tensors: Vec<GgufTensorInfo>,
    /// Byte offset where tensor data begins.
    pub data_offset: u64,
}

impl GgufFile {
    /// Parse a GGUF file from a reader.
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        // Read header
        let magic = read_u32(reader)?;
        if magic != GGUF_MAGIC {
            return Err(FuseError::InternalError(format!(
                "Invalid GGUF magic: 0x{:08X}, expected 0x{:08X}",
                magic, GGUF_MAGIC
            )));
        }

        let version = read_u32(reader)?;
        if version != GGUF_VERSION_2 && version != GGUF_VERSION_3 {
            return Err(FuseError::InternalError(format!(
                "Unsupported GGUF version: {}, expected 2 or 3",
                version
            )));
        }

        let tensor_count = read_u64(reader)?;
        let metadata_kv_count = read_u64(reader)?;

        let header = GgufHeader {
            magic,
            version,
            tensor_count,
            metadata_kv_count,
        };

        // Read metadata
        let mut metadata = HashMap::new();
        for _ in 0..metadata_kv_count {
            let key = read_string(reader)?;
            let value = read_value(reader)?;
            metadata.insert(key, value);
        }

        // Read tensor info
        let mut tensors = Vec::with_capacity(tensor_count as usize);
        for _ in 0..tensor_count {
            let name = read_string(reader)?;
            let n_dims = read_u32(reader)?;
            let mut dimensions = Vec::with_capacity(n_dims as usize);
            for _ in 0..n_dims {
                dimensions.push(read_u64(reader)?);
            }
            let ggml_type = GgmlType::from_u32(read_u32(reader)?)?;
            let offset = read_u64(reader)?;
            tensors.push(GgufTensorInfo {
                name,
                dimensions,
                ggml_type,
                offset,
            });
        }

        // Data starts at the current position, aligned to 32 bytes
        let pos = reader.stream_position().map_err(io_err)?;
        let data_offset = (pos + 31) & !31; // Align to 32 bytes

        Ok(GgufFile {
            header,
            metadata,
            tensors,
            data_offset,
        })
    }

    /// Get a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&GgufValue> {
        self.metadata.get(key)
    }

    /// Get the model architecture string.
    pub fn architecture(&self) -> Option<&str> {
        match self.metadata.get("general.architecture") {
            Some(GgufValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get the model name.
    pub fn model_name(&self) -> Option<&str> {
        match self.metadata.get("general.name") {
            Some(GgufValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Total size of all tensor data in bytes.
    pub fn total_tensor_bytes(&self) -> u64 {
        self.tensors.iter().map(|t| t.size_bytes()).sum()
    }

    /// Write a GGUF file. Used for roundtrip testing.
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        write_u32(writer, self.header.magic)?;
        write_u32(writer, self.header.version)?;
        write_u64(writer, self.header.tensor_count)?;
        write_u64(writer, self.header.metadata_kv_count)?;

        // Write metadata (sorted for deterministic output)
        let mut keys: Vec<_> = self.metadata.keys().collect();
        keys.sort();
        for key in keys {
            let value = &self.metadata[key];
            write_string(writer, key)?;
            write_value(writer, value)?;
        }

        // Write tensor info
        for tensor in &self.tensors {
            write_string(writer, &tensor.name)?;
            write_u32(writer, tensor.dimensions.len() as u32)?;
            for &dim in &tensor.dimensions {
                write_u64(writer, dim)?;
            }
            write_u32(writer, tensor.ggml_type as u32)?;
            write_u64(writer, tensor.offset)?;
        }

        Ok(())
    }
}

// ── Reader helpers ──

fn read_u8<R: Read>(r: &mut R) -> Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(buf[0])
}

fn read_i8<R: Read>(r: &mut R) -> Result<i8> {
    Ok(read_u8(r)? as i8)
}

fn read_u16<R: Read>(r: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16<R: Read>(r: &mut R) -> Result<i16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_u32<R: Read>(r: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i32<R: Read>(r: &mut R) -> Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_u64<R: Read>(r: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64<R: Read>(r: &mut R) -> Result<i64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32<R: Read>(r: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64<R: Read>(r: &mut R) -> Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf).map_err(io_err)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_bool<R: Read>(r: &mut R) -> Result<bool> {
    Ok(read_u8(r)? != 0)
}

fn read_string<R: Read>(r: &mut R) -> Result<String> {
    let len = read_u64(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(io_err)?;
    String::from_utf8(buf).map_err(|e| FuseError::InternalError(format!("Invalid UTF-8: {}", e)))
}

fn read_value<R: Read>(r: &mut R) -> Result<GgufValue> {
    let type_id = read_u32(r)?;
    match type_id {
        0 => Ok(GgufValue::Uint8(read_u8(r)?)),
        1 => Ok(GgufValue::Int8(read_i8(r)?)),
        2 => Ok(GgufValue::Uint16(read_u16(r)?)),
        3 => Ok(GgufValue::Int16(read_i16(r)?)),
        4 => Ok(GgufValue::Uint32(read_u32(r)?)),
        5 => Ok(GgufValue::Int32(read_i32(r)?)),
        6 => Ok(GgufValue::Float32(read_f32(r)?)),
        7 => Ok(GgufValue::Bool(read_bool(r)?)),
        8 => Ok(GgufValue::String(read_string(r)?)),
        9 => {
            let elem_type = read_u32(r)?;
            let len = read_u64(r)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                let val = read_array_element(r, elem_type)?;
                arr.push(val);
            }
            Ok(GgufValue::Array(arr))
        }
        10 => Ok(GgufValue::Uint64(read_u64(r)?)),
        11 => Ok(GgufValue::Int64(read_i64(r)?)),
        12 => Ok(GgufValue::Float64(read_f64(r)?)),
        _ => Err(FuseError::InternalError(format!(
            "Unknown GGUF value type: {}",
            type_id
        ))),
    }
}

fn read_array_element<R: Read>(r: &mut R, type_id: u32) -> Result<GgufValue> {
    match type_id {
        0 => Ok(GgufValue::Uint8(read_u8(r)?)),
        1 => Ok(GgufValue::Int8(read_i8(r)?)),
        2 => Ok(GgufValue::Uint16(read_u16(r)?)),
        3 => Ok(GgufValue::Int16(read_i16(r)?)),
        4 => Ok(GgufValue::Uint32(read_u32(r)?)),
        5 => Ok(GgufValue::Int32(read_i32(r)?)),
        6 => Ok(GgufValue::Float32(read_f32(r)?)),
        7 => Ok(GgufValue::Bool(read_bool(r)?)),
        8 => Ok(GgufValue::String(read_string(r)?)),
        10 => Ok(GgufValue::Uint64(read_u64(r)?)),
        11 => Ok(GgufValue::Int64(read_i64(r)?)),
        12 => Ok(GgufValue::Float64(read_f64(r)?)),
        _ => Err(FuseError::InternalError(format!(
            "Unknown array element type: {}",
            type_id
        ))),
    }
}

// ── Writer helpers ──

fn write_u32<W: std::io::Write>(w: &mut W, v: u32) -> Result<()> {
    w.write_all(&v.to_le_bytes()).map_err(io_err)
}

fn write_u64<W: std::io::Write>(w: &mut W, v: u64) -> Result<()> {
    w.write_all(&v.to_le_bytes()).map_err(io_err)
}

fn write_string<W: std::io::Write>(w: &mut W, s: &str) -> Result<()> {
    write_u64(w, s.len() as u64)?;
    w.write_all(s.as_bytes()).map_err(io_err)
}

fn write_value<W: std::io::Write>(w: &mut W, v: &GgufValue) -> Result<()> {
    match v {
        GgufValue::Uint8(x) => {
            write_u32(w, 0)?;
            w.write_all(&[*x]).map_err(io_err)?;
        }
        GgufValue::Int8(x) => {
            write_u32(w, 1)?;
            w.write_all(&[*x as u8]).map_err(io_err)?;
        }
        GgufValue::Uint16(x) => {
            write_u32(w, 2)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
        GgufValue::Int16(x) => {
            write_u32(w, 3)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
        GgufValue::Uint32(x) => {
            write_u32(w, 4)?;
            write_u32(w, *x)?;
        }
        GgufValue::Int32(x) => {
            write_u32(w, 5)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
        GgufValue::Float32(x) => {
            write_u32(w, 6)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
        GgufValue::Bool(x) => {
            write_u32(w, 7)?;
            w.write_all(&[*x as u8]).map_err(io_err)?;
        }
        GgufValue::String(s) => {
            write_u32(w, 8)?;
            write_string(w, s)?;
        }
        GgufValue::Array(arr) => {
            write_u32(w, 9)?;
            let elem_type = if let Some(first) = arr.first() {
                value_type_id(first)
            } else {
                0 // Default to uint8 for empty arrays
            };
            write_u32(w, elem_type)?;
            write_u64(w, arr.len() as u64)?;
            for elem in arr {
                write_value_data(w, elem)?;
            }
        }
        GgufValue::Uint64(x) => {
            write_u32(w, 10)?;
            write_u64(w, *x)?;
        }
        GgufValue::Int64(x) => {
            write_u32(w, 11)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
        GgufValue::Float64(x) => {
            write_u32(w, 12)?;
            w.write_all(&x.to_le_bytes()).map_err(io_err)?;
        }
    }
    Ok(())
}

fn write_value_data<W: std::io::Write>(w: &mut W, v: &GgufValue) -> Result<()> {
    match v {
        GgufValue::Uint8(x) => w.write_all(&[*x]).map_err(io_err),
        GgufValue::Int8(x) => w.write_all(&[*x as u8]).map_err(io_err),
        GgufValue::Uint16(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Int16(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Uint32(x) => write_u32(w, *x),
        GgufValue::Int32(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Float32(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Bool(x) => w.write_all(&[*x as u8]).map_err(io_err),
        GgufValue::String(s) => write_string(w, s),
        GgufValue::Uint64(x) => write_u64(w, *x),
        GgufValue::Int64(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Float64(x) => w.write_all(&x.to_le_bytes()).map_err(io_err),
        GgufValue::Array(_) => Err(FuseError::InternalError(
            "Nested arrays not supported".to_string(),
        )),
    }
}

fn value_type_id(v: &GgufValue) -> u32 {
    match v {
        GgufValue::Uint8(_) => 0,
        GgufValue::Int8(_) => 1,
        GgufValue::Uint16(_) => 2,
        GgufValue::Int16(_) => 3,
        GgufValue::Uint32(_) => 4,
        GgufValue::Int32(_) => 5,
        GgufValue::Float32(_) => 6,
        GgufValue::Bool(_) => 7,
        GgufValue::String(_) => 8,
        GgufValue::Array(_) => 9,
        GgufValue::Uint64(_) => 10,
        GgufValue::Int64(_) => 11,
        GgufValue::Float64(_) => 12,
    }
}

fn io_err(e: std::io::Error) -> FuseError {
    FuseError::InternalError(format!("IO error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Create a minimal valid GGUF file in memory.
    fn create_test_gguf() -> Vec<u8> {
        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&GGUF_MAGIC.to_le_bytes()); // magic
        buf.extend_from_slice(&GGUF_VERSION_3.to_le_bytes()); // version
        buf.extend_from_slice(&1u64.to_le_bytes()); // tensor_count = 1
        buf.extend_from_slice(&2u64.to_le_bytes()); // metadata_kv_count = 2

        // Metadata KV 1: general.architecture = "llama"
        let key = "general.architecture";
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes()); // type = string
        let val = "llama";
        buf.extend_from_slice(&(val.len() as u64).to_le_bytes());
        buf.extend_from_slice(val.as_bytes());

        // Metadata KV 2: general.name = "test-model"
        let key = "general.name";
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes()); // type = string
        let val = "test-model";
        buf.extend_from_slice(&(val.len() as u64).to_le_bytes());
        buf.extend_from_slice(val.as_bytes());

        // Tensor info: "weight" with shape [4, 4], type Q4_0, offset 0
        let name = "weight";
        buf.extend_from_slice(&(name.len() as u64).to_le_bytes());
        buf.extend_from_slice(name.as_bytes());
        buf.extend_from_slice(&2u32.to_le_bytes()); // n_dims = 2
        buf.extend_from_slice(&4u64.to_le_bytes()); // dim[0] = 4
        buf.extend_from_slice(&4u64.to_le_bytes()); // dim[1] = 4
        buf.extend_from_slice(&2u32.to_le_bytes()); // type = Q4_0
        buf.extend_from_slice(&0u64.to_le_bytes()); // offset = 0

        buf
    }

    #[test]
    fn test_parse_header() {
        let data = create_test_gguf();
        let mut cursor = Cursor::new(&data);
        let gguf = GgufFile::parse(&mut cursor).unwrap();

        assert_eq!(gguf.header.magic, GGUF_MAGIC);
        assert_eq!(gguf.header.version, GGUF_VERSION_3);
        assert_eq!(gguf.header.tensor_count, 1);
        assert_eq!(gguf.header.metadata_kv_count, 2);
    }

    #[test]
    fn test_parse_metadata() {
        let data = create_test_gguf();
        let mut cursor = Cursor::new(&data);
        let gguf = GgufFile::parse(&mut cursor).unwrap();

        assert_eq!(gguf.architecture(), Some("llama"));
        assert_eq!(gguf.model_name(), Some("test-model"));
    }

    #[test]
    fn test_parse_tensor_info() {
        let data = create_test_gguf();
        let mut cursor = Cursor::new(&data);
        let gguf = GgufFile::parse(&mut cursor).unwrap();

        assert_eq!(gguf.tensors.len(), 1);
        let tensor = &gguf.tensors[0];
        assert_eq!(tensor.name, "weight");
        assert_eq!(tensor.dimensions, vec![4, 4]);
        assert_eq!(tensor.ggml_type, GgmlType::Q4_0);
        assert_eq!(tensor.n_elements(), 16);
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = create_test_gguf();
        data[0] = 0xFF; // Corrupt magic
        let mut cursor = Cursor::new(&data);
        assert!(GgufFile::parse(&mut cursor).is_err());
    }

    #[test]
    fn test_ggml_type_from_u32() {
        assert_eq!(GgmlType::from_u32(0).unwrap(), GgmlType::F32);
        assert_eq!(GgmlType::from_u32(2).unwrap(), GgmlType::Q4_0);
        assert_eq!(GgmlType::from_u32(12).unwrap(), GgmlType::Q4K);
        assert!(GgmlType::from_u32(255).is_err());
    }

    #[test]
    fn test_tensor_size_bytes() {
        let tensor = GgufTensorInfo {
            name: "test".to_string(),
            dimensions: vec![32],
            ggml_type: GgmlType::Q4_0,
            offset: 0,
        };
        // 32 elements / 32 block_size = 1 block * 18 bytes/block = 18
        assert_eq!(tensor.size_bytes(), 18);

        let tensor_f32 = GgufTensorInfo {
            name: "test".to_string(),
            dimensions: vec![10],
            ggml_type: GgmlType::F32,
            offset: 0,
        };
        // 10 elements * 4 bytes = 40
        assert_eq!(tensor_f32.size_bytes(), 40);
    }

    #[test]
    fn test_write_parse_roundtrip() {
        let data = create_test_gguf();
        let mut cursor = Cursor::new(&data);
        let gguf = GgufFile::parse(&mut cursor).unwrap();

        // Write it back
        let mut output = Vec::new();
        gguf.write(&mut output).unwrap();

        // Parse the written data
        let mut cursor2 = Cursor::new(&output);
        let gguf2 = GgufFile::parse(&mut cursor2).unwrap();

        assert_eq!(gguf2.header.version, gguf.header.version);
        assert_eq!(gguf2.header.tensor_count, gguf.header.tensor_count);
        assert_eq!(gguf2.architecture(), gguf.architecture());
        assert_eq!(gguf2.model_name(), gguf.model_name());
        assert_eq!(gguf2.tensors.len(), gguf.tensors.len());
        assert_eq!(gguf2.tensors[0].name, gguf.tensors[0].name);
        assert_eq!(gguf2.tensors[0].ggml_type, gguf.tensors[0].ggml_type);
    }

    #[test]
    fn test_metadata_value_types() {
        // Build GGUF with various metadata types
        let mut buf = Vec::new();
        buf.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
        buf.extend_from_slice(&GGUF_VERSION_3.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes()); // no tensors
        buf.extend_from_slice(&3u64.to_le_bytes()); // 3 KV pairs

        // uint32
        let key = "count";
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&4u32.to_le_bytes()); // type = uint32
        buf.extend_from_slice(&42u32.to_le_bytes());

        // float32
        let key = "score";
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&6u32.to_le_bytes()); // type = float32
        buf.extend_from_slice(&3.14f32.to_le_bytes());

        // bool
        let key = "flag";
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&7u32.to_le_bytes()); // type = bool
        buf.push(1u8);

        let mut cursor = Cursor::new(&buf);
        let gguf = GgufFile::parse(&mut cursor).unwrap();

        assert_eq!(gguf.metadata.get("count"), Some(&GgufValue::Uint32(42)));
        assert!(matches!(
            gguf.metadata.get("flag"),
            Some(GgufValue::Bool(true))
        ));
    }
}
