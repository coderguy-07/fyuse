use crate::error::{FuseError, Result};
use crate::model::Auth;
use crate::storage::database::Database;
use serde::{Deserialize, Serialize};

/// Remote endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteEndpoint {
    /// Unique name for this endpoint
    pub name: String,
    /// URL of the remote endpoint
    pub url: String,
    /// Authentication credentials
    pub auth: Option<Auth>,
    /// Whether this endpoint is enabled
    pub enabled: bool,
    /// Optional description
    pub description: Option<String>,
}

impl RemoteEndpoint {
    /// Create a new remote endpoint
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            auth: None,
            enabled: true,
            description: None,
        }
    }

    /// Set authentication
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set enabled status
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate the endpoint configuration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(FuseError::ValidationError(
                "Endpoint name cannot be empty".to_string(),
            ));
        }

        if self.url.is_empty() {
            return Err(FuseError::ValidationError(
                "Endpoint URL cannot be empty".to_string(),
            ));
        }

        // Validate URL format
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(FuseError::ValidationError(format!(
                "Invalid URL format: {}. Must start with http:// or https://",
                self.url
            )));
        }

        Ok(())
    }
}

/// Repository for managing remote endpoints
pub struct RemoteEndpointRepository {
    db: Database,
}

impl RemoteEndpointRepository {
    /// Create a new remote endpoint repository
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Add a new remote endpoint
    pub fn add(&self, endpoint: RemoteEndpoint) -> Result<()> {
        // Validate endpoint
        endpoint.validate()?;

        // Check if endpoint with same name already exists
        if self.get(&endpoint.name)?.is_some() {
            return Err(FuseError::ValidationError(format!(
                "Remote endpoint '{}' already exists",
                endpoint.name
            )));
        }

        // Store in database
        self.db
            .put("config", &format!("remote:{}", endpoint.name), &endpoint)?;

        tracing::info!(
            endpoint_name = %endpoint.name,
            endpoint_url = %endpoint.url,
            "Added remote endpoint"
        );

        Ok(())
    }

    /// Remove a remote endpoint
    pub fn remove(&self, name: &str) -> Result<()> {
        // Check if endpoint exists
        if self.get(name)?.is_none() {
            return Err(FuseError::ValidationError(format!(
                "Remote endpoint '{}' not found",
                name
            )));
        }

        // Delete from database
        self.db.delete("config", &format!("remote:{}", name))?;

        tracing::info!(
            endpoint_name = %name,
            "Removed remote endpoint"
        );

        Ok(())
    }

    /// Get a remote endpoint by name
    pub fn get(&self, name: &str) -> Result<Option<RemoteEndpoint>> {
        self.db.get("config", &format!("remote:{}", name))
    }

    /// List all remote endpoints
    pub fn list(&self) -> Result<Vec<RemoteEndpoint>> {
        let keys = self.db.list_keys("config")?;
        let mut endpoints = Vec::new();

        for key in keys {
            if key.starts_with("remote:") {
                if let Some(endpoint) = self.db.get::<RemoteEndpoint>("config", &key)? {
                    endpoints.push(endpoint);
                }
            }
        }

        // Sort by name
        endpoints.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(endpoints)
    }

    /// Update an existing remote endpoint
    pub fn update(&self, endpoint: RemoteEndpoint) -> Result<()> {
        // Validate endpoint
        endpoint.validate()?;

        // Check if endpoint exists
        if self.get(&endpoint.name)?.is_none() {
            return Err(FuseError::ValidationError(format!(
                "Remote endpoint '{}' not found",
                endpoint.name
            )));
        }

        // Update in database
        self.db
            .put("config", &format!("remote:{}", endpoint.name), &endpoint)?;

        tracing::info!(
            endpoint_name = %endpoint.name,
            endpoint_url = %endpoint.url,
            "Updated remote endpoint"
        );

        Ok(())
    }

    /// Enable or disable a remote endpoint
    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut endpoint = self.get(name)?.ok_or_else(|| {
            FuseError::ValidationError(format!("Remote endpoint '{}' not found", name))
        })?;

        endpoint.enabled = enabled;
        self.update(endpoint)?;

        tracing::info!(
            endpoint_name = %name,
            enabled = %enabled,
            "Changed remote endpoint status"
        );

        Ok(())
    }

    /// Get all enabled remote endpoints
    pub fn list_enabled(&self) -> Result<Vec<RemoteEndpoint>> {
        let all_endpoints = self.list()?;
        Ok(all_endpoints.into_iter().filter(|e| e.enabled).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (RemoteEndpointRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = Database::new(db_path).unwrap();
        let repo = RemoteEndpointRepository::new(db);
        (repo, temp_dir)
    }

    #[test]
    fn test_remote_endpoint_creation() {
        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        assert_eq!(endpoint.name, "test");
        assert_eq!(endpoint.url, "https://example.com");
        assert!(endpoint.enabled);
        assert!(endpoint.auth.is_none());
        assert!(endpoint.description.is_none());
    }

    #[test]
    fn test_remote_endpoint_with_auth() {
        let endpoint = RemoteEndpoint::new("test", "https://example.com")
            .with_auth(Auth::ApiKey("key123".to_string()));

        assert!(endpoint.auth.is_some());
        assert!(matches!(endpoint.auth.unwrap(), Auth::ApiKey(_)));
    }

    #[test]
    fn test_remote_endpoint_with_description() {
        let endpoint =
            RemoteEndpoint::new("test", "https://example.com").with_description("Test endpoint");

        assert_eq!(endpoint.description, Some("Test endpoint".to_string()));
    }

    #[test]
    fn test_remote_endpoint_validation() {
        // Valid endpoint
        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        assert!(endpoint.validate().is_ok());

        // Empty name
        let endpoint = RemoteEndpoint::new("", "https://example.com");
        assert!(endpoint.validate().is_err());

        // Empty URL
        let endpoint = RemoteEndpoint::new("test", "");
        assert!(endpoint.validate().is_err());

        // Invalid URL format
        let endpoint = RemoteEndpoint::new("test", "example.com");
        assert!(endpoint.validate().is_err());

        // Valid http URL
        let endpoint = RemoteEndpoint::new("test", "http://example.com");
        assert!(endpoint.validate().is_ok());
    }

    #[test]
    fn test_add_remote_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        assert!(repo.add(endpoint).is_ok());

        // Verify it was added
        let retrieved = repo.get("test").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test");
    }

    #[test]
    fn test_add_duplicate_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        assert!(repo.add(endpoint.clone()).is_ok());

        // Try to add duplicate
        let result = repo.add(endpoint);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_remove_remote_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        repo.add(endpoint).unwrap();

        // Remove it
        assert!(repo.remove("test").is_ok());

        // Verify it was removed
        let retrieved = repo.get("test").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_remove_nonexistent_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let result = repo.remove("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_list_remote_endpoints() {
        let (repo, _temp_dir) = create_test_repo();

        // Add multiple endpoints
        repo.add(RemoteEndpoint::new("endpoint1", "https://example1.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("endpoint2", "https://example2.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("endpoint3", "https://example3.com"))
            .unwrap();

        let endpoints = repo.list().unwrap();
        assert_eq!(endpoints.len(), 3);

        // Verify they are sorted by name
        assert_eq!(endpoints[0].name, "endpoint1");
        assert_eq!(endpoints[1].name, "endpoint2");
        assert_eq!(endpoints[2].name, "endpoint3");
    }

    #[test]
    fn test_update_remote_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        repo.add(endpoint).unwrap();

        // Update it
        let updated =
            RemoteEndpoint::new("test", "https://updated.com").with_description("Updated endpoint");
        repo.update(updated).unwrap();

        // Verify it was updated
        let retrieved = repo.get("test").unwrap().unwrap();
        assert_eq!(retrieved.url, "https://updated.com");
        assert_eq!(retrieved.description, Some("Updated endpoint".to_string()));
    }

    #[test]
    fn test_update_nonexistent_endpoint() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("nonexistent", "https://example.com");
        let result = repo.update(endpoint);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_set_enabled() {
        let (repo, _temp_dir) = create_test_repo();

        let endpoint = RemoteEndpoint::new("test", "https://example.com");
        repo.add(endpoint).unwrap();

        // Disable it
        repo.set_enabled("test", false).unwrap();

        let retrieved = repo.get("test").unwrap().unwrap();
        assert!(!retrieved.enabled);

        // Enable it again
        repo.set_enabled("test", true).unwrap();

        let retrieved = repo.get("test").unwrap().unwrap();
        assert!(retrieved.enabled);
    }

    #[test]
    fn test_list_enabled() {
        let (repo, _temp_dir) = create_test_repo();

        // Add multiple endpoints
        repo.add(RemoteEndpoint::new("endpoint1", "https://example1.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("endpoint2", "https://example2.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("endpoint3", "https://example3.com"))
            .unwrap();

        // Disable one
        repo.set_enabled("endpoint2", false).unwrap();

        let enabled = repo.list_enabled().unwrap();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().any(|e| e.name == "endpoint1"));
        assert!(enabled.iter().any(|e| e.name == "endpoint3"));
        assert!(!enabled.iter().any(|e| e.name == "endpoint2"));
    }

    #[test]
    fn test_endpoint_serialization() {
        let endpoint = RemoteEndpoint::new("test", "https://example.com")
            .with_auth(Auth::ApiKey("key123".to_string()))
            .with_description("Test endpoint");

        let serialized = serde_json::to_string(&endpoint).unwrap();
        let deserialized: RemoteEndpoint = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.name, endpoint.name);
        assert_eq!(deserialized.url, endpoint.url);
        assert_eq!(deserialized.enabled, endpoint.enabled);
        assert_eq!(deserialized.description, endpoint.description);
    }

    // Integration tests for endpoint configuration management

    #[test]
    fn test_multiple_endpoints_management() {
        let (repo, _temp_dir) = create_test_repo();

        // Add multiple endpoints with different configurations
        let endpoint1 = RemoteEndpoint::new("aws-prod", "https://aws.example.com")
            .with_auth(Auth::ApiKey("aws-key".to_string()))
            .with_description("AWS production endpoint");

        let endpoint2 = RemoteEndpoint::new("gcp-staging", "https://gcp.example.com")
            .with_auth(Auth::BearerToken("gcp-token".to_string()))
            .with_description("GCP staging endpoint");

        let endpoint3 = RemoteEndpoint::new("on-prem", "http://internal.example.com")
            .with_auth(Auth::Basic {
                username: "admin".to_string(),
                password: "secret".to_string(),
            })
            .with_description("On-premises endpoint");

        // Add all endpoints
        repo.add(endpoint1).unwrap();
        repo.add(endpoint2).unwrap();
        repo.add(endpoint3).unwrap();

        // Verify all were added
        let endpoints = repo.list().unwrap();
        assert_eq!(endpoints.len(), 3);

        // Verify they can be retrieved individually
        let aws = repo.get("aws-prod").unwrap().unwrap();
        assert_eq!(aws.url, "https://aws.example.com");
        assert!(matches!(aws.auth, Some(Auth::ApiKey(_))));

        let gcp = repo.get("gcp-staging").unwrap().unwrap();
        assert_eq!(gcp.url, "https://gcp.example.com");
        assert!(matches!(gcp.auth, Some(Auth::BearerToken(_))));

        let on_prem = repo.get("on-prem").unwrap().unwrap();
        assert_eq!(on_prem.url, "http://internal.example.com");
        assert!(matches!(on_prem.auth, Some(Auth::Basic { .. })));
    }

    #[test]
    fn test_endpoint_lifecycle() {
        let (repo, _temp_dir) = create_test_repo();

        // Create endpoint
        let endpoint = RemoteEndpoint::new("lifecycle-test", "https://example.com")
            .with_description("Testing lifecycle");

        // Add
        repo.add(endpoint).unwrap();
        assert!(repo.get("lifecycle-test").unwrap().is_some());

        // Update
        let updated = RemoteEndpoint::new("lifecycle-test", "https://updated.example.com")
            .with_description("Updated description")
            .with_auth(Auth::ApiKey("new-key".to_string()));
        repo.update(updated).unwrap();

        let retrieved = repo.get("lifecycle-test").unwrap().unwrap();
        assert_eq!(retrieved.url, "https://updated.example.com");
        assert_eq!(
            retrieved.description,
            Some("Updated description".to_string())
        );
        assert!(retrieved.auth.is_some());

        // Disable
        repo.set_enabled("lifecycle-test", false).unwrap();
        let retrieved = repo.get("lifecycle-test").unwrap().unwrap();
        assert!(!retrieved.enabled);

        // Re-enable
        repo.set_enabled("lifecycle-test", true).unwrap();
        let retrieved = repo.get("lifecycle-test").unwrap().unwrap();
        assert!(retrieved.enabled);

        // Remove
        repo.remove("lifecycle-test").unwrap();
        assert!(repo.get("lifecycle-test").unwrap().is_none());
    }

    #[test]
    fn test_endpoint_filtering_by_status() {
        let (repo, _temp_dir) = create_test_repo();

        // Add multiple endpoints
        repo.add(RemoteEndpoint::new("enabled1", "https://e1.example.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("enabled2", "https://e2.example.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("disabled1", "https://d1.example.com"))
            .unwrap();
        repo.add(RemoteEndpoint::new("disabled2", "https://d2.example.com"))
            .unwrap();

        // Disable some endpoints
        repo.set_enabled("disabled1", false).unwrap();
        repo.set_enabled("disabled2", false).unwrap();

        // Get all endpoints
        let all = repo.list().unwrap();
        assert_eq!(all.len(), 4);

        // Get only enabled endpoints
        let enabled = repo.list_enabled().unwrap();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|e| e.enabled));
        assert!(enabled.iter().any(|e| e.name == "enabled1"));
        assert!(enabled.iter().any(|e| e.name == "enabled2"));
        assert!(!enabled.iter().any(|e| e.name == "disabled1"));
        assert!(!enabled.iter().any(|e| e.name == "disabled2"));
    }

    #[test]
    fn test_endpoint_validation_edge_cases() {
        // Test various URL formats
        let valid_https = RemoteEndpoint::new("test", "https://example.com");
        assert!(valid_https.validate().is_ok());

        let valid_http = RemoteEndpoint::new("test", "http://example.com");
        assert!(valid_http.validate().is_ok());

        let valid_with_port = RemoteEndpoint::new("test", "https://example.com:8080");
        assert!(valid_with_port.validate().is_ok());

        let valid_with_path = RemoteEndpoint::new("test", "https://example.com/api/v1");
        assert!(valid_with_path.validate().is_ok());

        let valid_localhost = RemoteEndpoint::new("test", "http://localhost:3000");
        assert!(valid_localhost.validate().is_ok());

        let valid_ip = RemoteEndpoint::new("test", "http://192.168.1.1");
        assert!(valid_ip.validate().is_ok());

        // Invalid formats
        let invalid_no_protocol = RemoteEndpoint::new("test", "example.com");
        assert!(invalid_no_protocol.validate().is_err());

        let invalid_ftp = RemoteEndpoint::new("test", "ftp://example.com");
        assert!(invalid_ftp.validate().is_err());

        let invalid_empty_name = RemoteEndpoint::new("", "https://example.com");
        assert!(invalid_empty_name.validate().is_err());

        let invalid_empty_url = RemoteEndpoint::new("test", "");
        assert!(invalid_empty_url.validate().is_err());
    }

    #[test]
    fn test_endpoint_persistence_across_operations() {
        let (repo, _temp_dir) = create_test_repo();

        // Add endpoint with all fields
        let original = RemoteEndpoint::new("persistent", "https://example.com")
            .with_auth(Auth::ApiKey("key123".to_string()))
            .with_description("Test persistence")
            .with_enabled(true);

        repo.add(original.clone()).unwrap();

        // Retrieve and verify all fields persisted
        let retrieved = repo.get("persistent").unwrap().unwrap();
        assert_eq!(retrieved.name, original.name);
        assert_eq!(retrieved.url, original.url);
        assert_eq!(retrieved.enabled, original.enabled);
        assert_eq!(retrieved.description, original.description);
        assert_eq!(retrieved.auth, original.auth);

        // Update one field and verify others remain unchanged
        repo.set_enabled("persistent", false).unwrap();
        let retrieved = repo.get("persistent").unwrap().unwrap();
        assert_eq!(retrieved.url, original.url);
        assert_eq!(retrieved.description, original.description);
        assert_eq!(retrieved.auth, original.auth);
        assert!(!retrieved.enabled); // Only this changed
    }

    #[test]
    fn test_endpoint_auth_types_persistence() {
        let (repo, _temp_dir) = create_test_repo();

        // Test ApiKey auth
        let api_key_endpoint = RemoteEndpoint::new("api-key", "https://example.com")
            .with_auth(Auth::ApiKey("test-key".to_string()));
        repo.add(api_key_endpoint).unwrap();
        let retrieved = repo.get("api-key").unwrap().unwrap();
        assert!(matches!(retrieved.auth, Some(Auth::ApiKey(_))));

        // Test BearerToken auth
        let bearer_endpoint = RemoteEndpoint::new("bearer", "https://example.com")
            .with_auth(Auth::BearerToken("test-token".to_string()));
        repo.add(bearer_endpoint).unwrap();
        let retrieved = repo.get("bearer").unwrap().unwrap();
        assert!(matches!(retrieved.auth, Some(Auth::BearerToken(_))));

        // Test Basic auth
        let basic_endpoint =
            RemoteEndpoint::new("basic", "https://example.com").with_auth(Auth::Basic {
                username: "user".to_string(),
                password: "pass".to_string(),
            });
        repo.add(basic_endpoint).unwrap();
        let retrieved = repo.get("basic").unwrap().unwrap();
        assert!(matches!(retrieved.auth, Some(Auth::Basic { .. })));

        // Test no auth
        let no_auth_endpoint = RemoteEndpoint::new("no-auth", "https://example.com");
        repo.add(no_auth_endpoint).unwrap();
        let retrieved = repo.get("no-auth").unwrap().unwrap();
        assert!(retrieved.auth.is_none());
    }

    #[test]
    fn test_concurrent_endpoint_operations() {
        let (repo, _temp_dir) = create_test_repo();

        // Add multiple endpoints
        for i in 0..10 {
            let endpoint = RemoteEndpoint::new(
                format!("endpoint-{}", i),
                format!("https://example{}.com", i),
            );
            repo.add(endpoint).unwrap();
        }

        // Verify all were added
        let endpoints = repo.list().unwrap();
        assert_eq!(endpoints.len(), 10);

        // Update some
        for i in 0..5 {
            let updated = RemoteEndpoint::new(
                format!("endpoint-{}", i),
                format!("https://updated{}.com", i),
            );
            repo.update(updated).unwrap();
        }

        // Disable some
        for i in 5..8 {
            repo.set_enabled(&format!("endpoint-{}", i), false).unwrap();
        }

        // Remove some
        for i in 8..10 {
            repo.remove(&format!("endpoint-{}", i)).unwrap();
        }

        // Verify final state
        let all = repo.list().unwrap();
        assert_eq!(all.len(), 8);

        let enabled = repo.list_enabled().unwrap();
        assert_eq!(enabled.len(), 5);

        // Verify updates were applied
        for i in 0..5 {
            let endpoint = repo.get(&format!("endpoint-{}", i)).unwrap().unwrap();
            assert_eq!(endpoint.url, format!("https://updated{}.com", i));
        }
    }
}
