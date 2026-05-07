//! Role-based access control with multi-tenant support.

use crate::error::{FuseError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// A tenant in the multi-tenant system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub resource_quota: ResourceQuota,
}

/// Resource quotas per tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub max_models: usize,
    pub max_memory_bytes: u64,
    pub max_requests_per_minute: u32,
    pub allowed_models: Option<Vec<String>>,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_models: 5,
            max_memory_bytes: 8 * 1024 * 1024 * 1024, // 8GB
            max_requests_per_minute: 60,
            allowed_models: None, // All models allowed
        }
    }
}

/// A role with associated permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub permissions: HashSet<Permission>,
}

/// Permissions for RBAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ModelList,
    ModelPull,
    ModelLoad,
    ModelUnload,
    ModelDelete,
    ModelQuantize,
    InferenceRun,
    InferenceStream,
    ConfigRead,
    ConfigWrite,
    UserManage,
    TenantManage,
    AuditRead,
    SystemAdmin,
}

impl Permission {
    /// All available permissions.
    pub fn all() -> HashSet<Permission> {
        use Permission::*;
        [
            ModelList,
            ModelPull,
            ModelLoad,
            ModelUnload,
            ModelDelete,
            ModelQuantize,
            InferenceRun,
            InferenceStream,
            ConfigRead,
            ConfigWrite,
            UserManage,
            TenantManage,
            AuditRead,
            SystemAdmin,
        ]
        .into_iter()
        .collect()
    }
}

/// A user with tenant and role assignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
}

/// RBAC manager — manages tenants, roles, users, and permission checks.
pub struct RbacManager {
    tenants: Arc<DashMap<String, Tenant>>,
    roles: Arc<DashMap<String, Role>>,
    users: Arc<DashMap<String, User>>,
}

impl RbacManager {
    pub fn new() -> Self {
        let manager = Self {
            tenants: Arc::new(DashMap::new()),
            roles: Arc::new(DashMap::new()),
            users: Arc::new(DashMap::new()),
        };
        manager.init_default_roles();
        manager
    }

    fn init_default_roles(&self) {
        // Admin role — full access
        self.roles.insert(
            "admin".to_string(),
            Role {
                name: "admin".to_string(),
                permissions: Permission::all(),
            },
        );

        // User role — inference + model listing
        let mut user_perms = HashSet::new();
        user_perms.insert(Permission::ModelList);
        user_perms.insert(Permission::InferenceRun);
        user_perms.insert(Permission::InferenceStream);
        self.roles.insert(
            "user".to_string(),
            Role {
                name: "user".to_string(),
                permissions: user_perms,
            },
        );

        // Operator role — model management
        let mut op_perms = HashSet::new();
        op_perms.insert(Permission::ModelList);
        op_perms.insert(Permission::ModelPull);
        op_perms.insert(Permission::ModelLoad);
        op_perms.insert(Permission::ModelUnload);
        op_perms.insert(Permission::ModelQuantize);
        op_perms.insert(Permission::InferenceRun);
        op_perms.insert(Permission::InferenceStream);
        op_perms.insert(Permission::ConfigRead);
        self.roles.insert(
            "operator".to_string(),
            Role {
                name: "operator".to_string(),
                permissions: op_perms,
            },
        );
    }

    /// Create a tenant.
    pub fn create_tenant(&self, tenant: Tenant) -> Result<()> {
        if self.tenants.contains_key(&tenant.id) {
            return Err(FuseError::ValidationError(format!(
                "Tenant '{}' already exists",
                tenant.id
            )));
        }
        self.tenants.insert(tenant.id.clone(), tenant);
        Ok(())
    }

    /// Get a tenant.
    pub fn get_tenant(&self, id: &str) -> Option<Tenant> {
        self.tenants.get(id).map(|t| t.clone())
    }

    /// Create a role.
    pub fn create_role(&self, role: Role) -> Result<()> {
        self.roles.insert(role.name.clone(), role);
        Ok(())
    }

    /// Create a user.
    pub fn create_user(&self, user: User) -> Result<()> {
        // Verify tenant exists
        if !self.tenants.contains_key(&user.tenant_id) {
            return Err(FuseError::ValidationError(format!(
                "Tenant '{}' not found",
                user.tenant_id
            )));
        }
        self.users.insert(user.id.clone(), user);
        Ok(())
    }

    /// Check if a user has a specific permission.
    pub fn check_permission(&self, user_id: &str, permission: Permission) -> Result<bool> {
        let user = self
            .users
            .get(user_id)
            .ok_or_else(|| FuseError::PermissionDenied(format!("User '{}' not found", user_id)))?;

        // Check tenant is enabled
        let tenant = self.tenants.get(&user.tenant_id).ok_or_else(|| {
            FuseError::PermissionDenied(format!("Tenant '{}' not found", user.tenant_id))
        })?;

        if !tenant.enabled {
            return Ok(false);
        }

        // Check roles
        for role_name in &user.roles {
            if let Some(role) = self.roles.get(role_name) {
                if role.permissions.contains(&permission) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Require a permission — returns error if not granted.
    pub fn require_permission(&self, user_id: &str, permission: Permission) -> Result<()> {
        if self.check_permission(user_id, permission)? {
            Ok(())
        } else {
            Err(FuseError::PermissionDenied(format!(
                "User '{}' lacks permission {:?}",
                user_id, permission
            )))
        }
    }

    /// Check if a user can access a model (tenant model restrictions).
    pub fn check_model_access(&self, user_id: &str, model_name: &str) -> Result<bool> {
        let user = self
            .users
            .get(user_id)
            .ok_or_else(|| FuseError::PermissionDenied(format!("User '{}' not found", user_id)))?;

        let tenant = self.tenants.get(&user.tenant_id).ok_or_else(|| {
            FuseError::PermissionDenied(format!("Tenant '{}' not found", user.tenant_id))
        })?;

        if let Some(ref allowed) = tenant.resource_quota.allowed_models {
            Ok(allowed.iter().any(|m| m == model_name))
        } else {
            Ok(true) // No restrictions
        }
    }
}

impl Default for RbacManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> RbacManager {
        let mgr = RbacManager::new();
        mgr.create_tenant(Tenant {
            id: "t1".to_string(),
            name: "Test Tenant".to_string(),
            enabled: true,
            resource_quota: ResourceQuota::default(),
        })
        .unwrap();
        mgr.create_user(User {
            id: "admin1".to_string(),
            tenant_id: "t1".to_string(),
            roles: vec!["admin".to_string()],
        })
        .unwrap();
        mgr.create_user(User {
            id: "user1".to_string(),
            tenant_id: "t1".to_string(),
            roles: vec!["user".to_string()],
        })
        .unwrap();
        mgr
    }

    #[test]
    fn test_default_roles_created() {
        let mgr = RbacManager::new();
        assert!(mgr.roles.contains_key("admin"));
        assert!(mgr.roles.contains_key("user"));
        assert!(mgr.roles.contains_key("operator"));
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let mgr = setup();
        assert!(mgr
            .check_permission("admin1", Permission::SystemAdmin)
            .unwrap());
        assert!(mgr
            .check_permission("admin1", Permission::ModelDelete)
            .unwrap());
        assert!(mgr
            .check_permission("admin1", Permission::TenantManage)
            .unwrap());
    }

    #[test]
    fn test_user_limited_permissions() {
        let mgr = setup();
        assert!(mgr
            .check_permission("user1", Permission::InferenceRun)
            .unwrap());
        assert!(mgr
            .check_permission("user1", Permission::ModelList)
            .unwrap());
        assert!(!mgr
            .check_permission("user1", Permission::ModelDelete)
            .unwrap());
        assert!(!mgr
            .check_permission("user1", Permission::SystemAdmin)
            .unwrap());
    }

    #[test]
    fn test_require_permission_success() {
        let mgr = setup();
        assert!(mgr
            .require_permission("admin1", Permission::SystemAdmin)
            .is_ok());
    }

    #[test]
    fn test_require_permission_denied() {
        let mgr = setup();
        assert!(mgr
            .require_permission("user1", Permission::SystemAdmin)
            .is_err());
    }

    #[test]
    fn test_unknown_user() {
        let mgr = setup();
        assert!(mgr
            .check_permission("unknown", Permission::ModelList)
            .is_err());
    }

    #[test]
    fn test_disabled_tenant() {
        let mgr = RbacManager::new();
        mgr.create_tenant(Tenant {
            id: "disabled".to_string(),
            name: "Disabled".to_string(),
            enabled: false,
            resource_quota: ResourceQuota::default(),
        })
        .unwrap();
        mgr.create_user(User {
            id: "u_disabled".to_string(),
            tenant_id: "disabled".to_string(),
            roles: vec!["admin".to_string()],
        })
        .unwrap();
        assert!(!mgr
            .check_permission("u_disabled", Permission::SystemAdmin)
            .unwrap());
    }

    #[test]
    fn test_tenant_model_access_unrestricted() {
        let mgr = setup();
        assert!(mgr.check_model_access("admin1", "any-model").unwrap());
    }

    #[test]
    fn test_tenant_model_access_restricted() {
        let mgr = RbacManager::new();
        mgr.create_tenant(Tenant {
            id: "restricted".to_string(),
            name: "Restricted".to_string(),
            enabled: true,
            resource_quota: ResourceQuota {
                allowed_models: Some(vec!["llama3".to_string()]),
                ..Default::default()
            },
        })
        .unwrap();
        mgr.create_user(User {
            id: "ru1".to_string(),
            tenant_id: "restricted".to_string(),
            roles: vec!["user".to_string()],
        })
        .unwrap();

        assert!(mgr.check_model_access("ru1", "llama3").unwrap());
        assert!(!mgr.check_model_access("ru1", "gpt4").unwrap());
    }

    #[test]
    fn test_duplicate_tenant() {
        let mgr = setup();
        let result = mgr.create_tenant(Tenant {
            id: "t1".to_string(),
            name: "Duplicate".to_string(),
            enabled: true,
            resource_quota: ResourceQuota::default(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_user_invalid_tenant() {
        let mgr = RbacManager::new();
        let result = mgr.create_user(User {
            id: "u1".to_string(),
            tenant_id: "nonexistent".to_string(),
            roles: vec!["user".to_string()],
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_all() {
        let all = Permission::all();
        assert_eq!(all.len(), 14);
        assert!(all.contains(&Permission::SystemAdmin));
        assert!(all.contains(&Permission::InferenceRun));
    }

    #[test]
    fn test_resource_quota_default() {
        let quota = ResourceQuota::default();
        assert_eq!(quota.max_models, 5);
        assert_eq!(quota.max_requests_per_minute, 60);
        assert!(quota.allowed_models.is_none());
    }
}
