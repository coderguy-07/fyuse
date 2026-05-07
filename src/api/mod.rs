//! REST/WebSocket API server — Ollama, OpenAI, and Anthropic compatible.

pub mod routes;
pub mod server;

pub use server::{ApiServer, ApiState};
