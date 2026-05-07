//! Model registries — download models from HuggingFace, Ollama, etc.

pub mod huggingface;
pub mod ollama;

pub use huggingface::{HfDownloadOptions, HfModelRegistry};
pub use ollama::{OllamaModelRef, OllamaRegistry};
