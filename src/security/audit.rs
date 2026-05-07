//! Audit logging — tracks all security-relevant events.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

/// An auditable event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub resource: String,
    pub action: String,
    pub outcome: AuditOutcome,
    pub details: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    ModelAccess,
    ConfigChange,
    DataAccess,
    SecurityThreat,
    SystemEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    Success,
    Failure,
    Blocked,
}

/// In-memory audit log with configurable max size.
pub struct AuditLog {
    events: Arc<RwLock<VecDeque<AuditEvent>>>,
    max_events: usize,
}

impl AuditLog {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(max_events))),
            max_events,
        }
    }

    /// Record an audit event.
    pub fn record(&self, event: AuditEvent) {
        tracing::info!(
            event_type = ?event.event_type,
            action = %event.action,
            outcome = ?event.outcome,
            "Audit event recorded"
        );
        let mut events = self.events.write();
        events.push_back(event);
        while events.len() > self.max_events {
            events.pop_front();
        }
    }

    /// Create and record a simple event.
    pub fn log(
        &self,
        event_type: AuditEventType,
        resource: impl Into<String>,
        action: impl Into<String>,
        outcome: AuditOutcome,
    ) {
        self.record(AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            tenant_id: None,
            user_id: None,
            resource: resource.into(),
            action: action.into(),
            outcome,
            details: None,
            ip_address: None,
        });
    }

    /// Get recent events.
    pub fn recent(&self, count: usize) -> Vec<AuditEvent> {
        let events = self.events.read();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Get events for a specific tenant.
    pub fn by_tenant(&self, tenant_id: &str) -> Vec<AuditEvent> {
        let events = self.events.read();
        events
            .iter()
            .filter(|e| e.tenant_id.as_deref() == Some(tenant_id))
            .cloned()
            .collect()
    }

    /// Total event count.
    pub fn count(&self) -> usize {
        self.events.read().len()
    }

    /// Clear all events.
    pub fn clear(&self) {
        self.events.write().clear();
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_record() {
        let log = AuditLog::new(100);
        log.log(
            AuditEventType::ModelAccess,
            "model/llama3",
            "load",
            AuditOutcome::Success,
        );
        assert_eq!(log.count(), 1);
    }

    #[test]
    fn test_audit_log_recent() {
        let log = AuditLog::new(100);
        for i in 0..5 {
            log.log(
                AuditEventType::SystemEvent,
                format!("resource-{}", i),
                "action",
                AuditOutcome::Success,
            );
        }
        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_audit_log_max_events() {
        let log = AuditLog::new(3);
        for i in 0..5 {
            log.log(
                AuditEventType::SystemEvent,
                format!("resource-{}", i),
                "action",
                AuditOutcome::Success,
            );
        }
        assert_eq!(log.count(), 3);
    }

    #[test]
    fn test_audit_log_by_tenant() {
        let log = AuditLog::new(100);
        log.record(AuditEvent {
            id: "1".to_string(),
            timestamp: Utc::now(),
            event_type: AuditEventType::ModelAccess,
            tenant_id: Some("tenant-a".to_string()),
            user_id: None,
            resource: "model".to_string(),
            action: "load".to_string(),
            outcome: AuditOutcome::Success,
            details: None,
            ip_address: None,
        });
        log.record(AuditEvent {
            id: "2".to_string(),
            timestamp: Utc::now(),
            event_type: AuditEventType::ModelAccess,
            tenant_id: Some("tenant-b".to_string()),
            user_id: None,
            resource: "model".to_string(),
            action: "load".to_string(),
            outcome: AuditOutcome::Success,
            details: None,
            ip_address: None,
        });
        assert_eq!(log.by_tenant("tenant-a").len(), 1);
        assert_eq!(log.by_tenant("tenant-b").len(), 1);
        assert_eq!(log.by_tenant("tenant-c").len(), 0);
    }

    #[test]
    fn test_audit_log_clear() {
        let log = AuditLog::new(100);
        log.log(
            AuditEventType::SystemEvent,
            "test",
            "action",
            AuditOutcome::Success,
        );
        assert_eq!(log.count(), 1);
        log.clear();
        assert_eq!(log.count(), 0);
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent {
            id: "test-id".to_string(),
            timestamp: Utc::now(),
            event_type: AuditEventType::Authentication,
            tenant_id: Some("t1".to_string()),
            user_id: Some("u1".to_string()),
            resource: "api".to_string(),
            action: "login".to_string(),
            outcome: AuditOutcome::Success,
            details: None,
            ip_address: Some("127.0.0.1".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Authentication"));
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
    }
}
