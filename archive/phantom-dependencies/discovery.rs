//! Discovery integration for agentReagents.
//!
//! This module makes agentReagents discoverable as a template building
//! and image management service using primal-substrate.
//!
//! ## Philosophy
//!
//! agentReagents knows:
//! - Who it is (identity)
//! - What it can do (template building, image management)
//! - Where its templates are
//!
//! agentReagents discovers:
//! - VM providers (benchScale via VmProvisioning capability)
//! - Other template providers (for template sharing)
//!
//! ## Example
//!
//! ```rust,no_run
//! use agent_reagents::discovery::ReagentsProvider;
//! use primal_substrate::Discovery;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let discovery = Discovery::new().await?;
//!
//! // Register ourselves
//! let provider = ReagentsProvider::new("/path/to/templates")?;
//! provider.register(&discovery).await?;
//!
//! // Now others can discover us!
//! # Ok(())
//! # }
//! ```

use primal_substrate::{Capability, Discovery, PrimalIdentity};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

/// Extended capability for template building
const CAP_TEMPLATE_BUILDING: &str = "template-building";
/// Extended capability for image management
const CAP_IMAGE_MANAGEMENT: &str = "image-management";

/// agentReagents provider for discovery
pub struct ReagentsProvider {
    identity: PrimalIdentity,
    templates_path: PathBuf,
    metadata: HashMap<String, String>,
}

impl ReagentsProvider {
    /// Create new reagents provider
    ///
    /// # Arguments
    /// * `templates_path` - Path to template directory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use agent_reagents::discovery::ReagentsProvider;
    ///
    /// let provider = ReagentsProvider::new("/opt/agentReagents/templates")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(templates_path: impl Into<PathBuf>) -> Result<Self> {
        let templates_path = templates_path.into();
        
        // Validate templates path exists
        if !templates_path.exists() {
            std::fs::create_dir_all(&templates_path)?;
        }
        
        let mut metadata = HashMap::new();
        metadata.insert(
            "templates_path".to_string(),
            templates_path.to_string_lossy().to_string(),
        );
        metadata.insert(
            "version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        
        // Create identity with capabilities
        let identity = PrimalIdentity::new("agent-reagents", env!("CARGO_PKG_VERSION"))
            .with_capability(Capability::Custom(CAP_TEMPLATE_BUILDING.to_string()))
            .with_capability(Capability::Custom(CAP_IMAGE_MANAGEMENT.to_string()))
            .with_metadata("templates_path", templates_path.to_string_lossy().as_ref());
        
        Ok(Self {
            identity,
            templates_path,
            metadata,
        })
    }
    
    /// Register with discovery system
    ///
    /// Makes agentReagents discoverable by other primals.
    pub async fn register(&self, discovery: &Discovery) -> Result<()> {
        discovery.register(&self.identity).await?;
        tracing::info!(
            "Registered agentReagents at {}",
            self.templates_path.display()
        );
        Ok(())
    }
    
    /// Unregister from discovery
    pub async fn unregister(&self, discovery: &Discovery) -> Result<()> {
        discovery.unregister(&self.identity).await?;
        tracing::info!("Unregistered agentReagents");
        Ok(())
    }
    
    /// Find VM providers (benchScale or others)
    ///
    /// Uses discovery to find any service that can provision VMs,
    /// without hardcoding benchScale or libvirt.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use agent_reagents::discovery::ReagentsProvider;
    /// use primal_substrate::Discovery;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let discovery = Discovery::new().await?;
    /// let provider = ReagentsProvider::new("/tmp/templates")?;
    ///
    /// // Zero hardcoding!
    /// let vm_provider = provider.find_vm_provider(&discovery).await?;
    /// println!("Using VM provider: {}", vm_provider.name);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_vm_provider(
        &self,
        discovery: &Discovery,
    ) -> Result<primal_substrate::ServiceInfo> {
        let provider = discovery
            .find_capability(Capability::VmProvisioning)
            .await?;
        
        tracing::info!(
            "Discovered VM provider: {} ({})",
            provider.name,
            provider.version
        );
        
        Ok(provider)
    }
    
    /// Find other template providers
    ///
    /// Discovers other agentReagents instances or compatible
    /// template building services.
    pub async fn find_template_providers(
        &self,
        discovery: &Discovery,
    ) -> Result<Vec<primal_substrate::ServiceInfo>> {
        let providers: Vec<primal_substrate::ServiceInfo> = discovery
            .discover_all()
            .await?
            .into_iter()
            .filter(|s| {
                s.capabilities
                    .contains(&Capability::Custom(CAP_TEMPLATE_BUILDING.to_string()))
            })
            .collect();
        
        tracing::info!("Discovered {} template providers", providers.len());
        
        Ok(providers)
    }
    
    /// Get our identity
    pub fn identity(&self) -> &PrimalIdentity {
        &self.identity
    }
    
    /// Get templates path
    pub fn templates_path(&self) -> &PathBuf {
        &self.templates_path
    }
    
    /// Get metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primal_substrate::adapter::FileAdapter;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_reagents_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("registry.json");
        
        // Create file adapter for testing
        let adapter = FileAdapter::new(temp_file).unwrap();
        let discovery = Discovery::with_adapter(adapter);
        
        // Create provider
        let provider = ReagentsProvider::new(temp_dir.path()).unwrap();
        
        // Register
        provider.register(&discovery).await.unwrap();
        
        // Discover template builders
        let builders = provider.find_template_providers(&discovery).await.unwrap();
        
        assert_eq!(builders.len(), 1);
        assert_eq!(builders[0].name, "agent-reagents");
        assert!(builders[0]
            .capabilities
            .contains(&Capability::Custom(CAP_TEMPLATE_BUILDING.to_string())));
    }
    
    #[tokio::test]
    async fn test_zero_hardcoding_vm_discovery() {
        // This test validates that agentReagents doesn't hardcode benchScale
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("registry.json");
        
        let adapter = FileAdapter::new(temp_file).unwrap();
        let discovery = Discovery::with_adapter(adapter);
        
        let provider = ReagentsProvider::new(temp_dir.path()).unwrap();
        
        // Try to find VM provider (should fail - none registered)
        let result = provider.find_vm_provider(&discovery).await;
        assert!(result.is_err());
        
        // This proves agentReagents doesn't hardcode a VM provider!
        // It discovers them at runtime
    }
    
    #[tokio::test]
    async fn test_identity_self_knowledge() {
        // Validate self-knowledge only
        let temp_dir = TempDir::new().unwrap();
        let provider = ReagentsProvider::new(temp_dir.path()).unwrap();
        
        let identity = provider.identity();
        
        // Knows itself
        assert_eq!(identity.name, "agent-reagents");
        assert_eq!(identity.version, env!("CARGO_PKG_VERSION"));
        
        // Knows what it can do
        assert!(identity
            .capabilities
            .contains(&Capability::Custom(CAP_TEMPLATE_BUILDING.to_string())));
        assert!(identity
            .capabilities
            .contains(&Capability::Custom(CAP_IMAGE_MANAGEMENT.to_string())));
        
        // Knows where its templates are
        assert!(identity.metadata.contains_key("templates_path"));
    }
}

