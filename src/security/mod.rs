//! AI Shield Gateway — security middleware for AI inference.
//!
//! Provides OWASP LLM Top 10 protection:
//! - Prompt injection detection
//! - PII redaction
//! - Content filtering
//! - RBAC (role-based access control)
//! - Audit logging
//! - Model SBOM generation

pub mod ai_shield;
pub mod audit;
pub mod rbac;

pub use ai_shield::{AiShield, ShieldConfig, ThreatLevel};
pub use audit::{AuditEvent, AuditLog};
pub use rbac::{Permission, RbacManager, Role, Tenant};
