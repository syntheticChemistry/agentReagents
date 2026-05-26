// SPDX-License-Identifier: AGPL-3.0-or-later
//! Template registry for managing templates
//!
//! Provides storage and retrieval of template manifests with checksums.

use super::TemplateInfo;
use super::manifest::TemplateManifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

/// Registry error types
#[derive(Debug, Error)]
pub enum RegistryError {
    /// No template registered under that name.
    #[error("Template not found: {0}")]
    NotFound(String),

    /// Register would overwrite an existing template name.
    #[error("Template already exists: {0}")]
    AlreadyExists(String),

    /// Manifest or path failed validation.
    #[error("Invalid template: {0}")]
    Invalid(String),

    /// Underlying filesystem error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Template registry
pub struct TemplateRegistry {
    registry_dir: PathBuf,
    templates_dir: PathBuf,
    index: RegistryIndex,
}

/// Registry index (stored as JSON)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegistryIndex {
    templates: HashMap<String, TemplateInfo>,
}

impl TemplateRegistry {
    /// Create or open a template registry under `base_dir/registry` and `base_dir/templates`.
    pub fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        let registry_dir = base_dir.join("registry");
        let templates_dir = base_dir.join("templates");

        // Create directories if they don't exist
        std::fs::create_dir_all(&registry_dir)?;
        std::fs::create_dir_all(&templates_dir)?;

        // Load index
        let index_path = registry_dir.join("index.json");
        let index = if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)?;
            serde_json::from_str(&content)?
        } else {
            RegistryIndex::default()
        };

        info!(
            "Opened template registry: {} templates",
            index.templates.len()
        );

        Ok(Self {
            registry_dir,
            templates_dir,
            index,
        })
    }

    /// List all templates
    pub fn list_templates(&self) -> Vec<TemplateInfo> {
        self.index.templates.values().cloned().collect()
    }

    /// Get template by name
    pub fn get_template(&self, name: &str) -> Result<TemplateInfo, RegistryError> {
        self.index
            .templates
            .get(name)
            .cloned()
            .ok_or_else(|| RegistryError::NotFound(name.to_string()))
    }

    /// Check if template exists
    pub fn has_template(&self, name: &str) -> bool {
        self.index.templates.contains_key(name)
    }

    /// Register a new template
    pub fn register_template(
        &mut self,
        manifest: &TemplateManifest,
        template_path: &Path,
    ) -> Result<()> {
        // Validate manifest
        manifest
            .validate()
            .context("Template manifest validation failed")?;

        // Check if already exists
        if self.has_template(&manifest.name) {
            return Err(RegistryError::AlreadyExists(manifest.name.clone()).into());
        }

        // Calculate checksum
        let checksum = Self::calculate_checksum(template_path)?;

        // Get file size
        let size_bytes = std::fs::metadata(template_path)?.len();

        // Copy template to registry
        let dest_path = self.templates_dir.join(format!("{}.qcow2", manifest.name));
        std::fs::copy(template_path, &dest_path)?;

        // Save manifest
        let manifest_path = self.registry_dir.join(format!("{}.yaml", manifest.name));
        manifest.to_yaml_file(&manifest_path)?;

        // Add to index
        let info = TemplateInfo {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            path: dest_path,
            size_bytes,
            checksum,
            verified: true,
        };

        self.index.templates.insert(manifest.name.clone(), info);

        // Save index
        self.save_index()?;

        info!(
            "Registered template: {} v{}",
            manifest.name, manifest.version
        );

        Ok(())
    }

    /// Get manifest for a template
    pub fn get_manifest(&self, name: &str) -> Result<TemplateManifest> {
        let manifest_path = self.registry_dir.join(format!("{}.yaml", name));

        if !manifest_path.exists() {
            anyhow::bail!("Manifest not found for template: {}", name);
        }

        TemplateManifest::from_yaml_file(&manifest_path)
    }

    /// Verify template integrity
    pub fn verify_template(&self, name: &str) -> Result<bool> {
        let info = self.get_template(name).map_err(|e| anyhow::anyhow!(e))?;

        if !info.path.exists() {
            anyhow::bail!("Template file not found: {}", info.path.display());
        }

        debug!("Verifying template checksum: {}", name);
        let actual_checksum = Self::calculate_checksum(&info.path)?;

        Ok(actual_checksum == info.checksum)
    }

    /// Delete a template
    pub fn delete_template(&mut self, name: &str) -> Result<()> {
        let info = self.get_template(name).map_err(|e| anyhow::anyhow!(e))?;

        // Delete template file
        if info.path.exists() {
            std::fs::remove_file(&info.path)?;
        }

        // Delete manifest
        let manifest_path = self.registry_dir.join(format!("{}.yaml", name));
        if manifest_path.exists() {
            std::fs::remove_file(&manifest_path)?;
        }

        // Remove from index
        self.index.templates.remove(name);

        // Save index
        self.save_index()?;

        info!("Deleted template: {}", name);

        Ok(())
    }

    /// Calculate SHA256 checksum of a file
    fn calculate_checksum(path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Save registry index
    fn save_index(&self) -> Result<()> {
        let index_path = self.registry_dir.join("index.json");
        let json = serde_json::to_string_pretty(&self.index)?;
        std::fs::write(index_path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::manifest::{
        ResourceConfig, TemplateManifest, UserConfig, VerificationConfig,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn minimal_manifest(name: &str) -> TemplateManifest {
        TemplateManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            base_image: "ubuntu.img".to_string(),
            golden_image: None,
            description: None,
            resources: ResourceConfig {
                memory_mb: 2048,
                vcpus: 2,
                disk_gb: 30,
                timeout_secs: 2400,
                static_ip: None,
            },
            pci_passthrough: vec![],
            package_manager: crate::templates::PackageManager::default(),
            users: vec![UserConfig {
                name: "ubuntu".to_string(),
                password: None,
                groups: vec![],
                ssh_authorized_keys: vec![],
            }],
            build_steps: vec![],
            post_boot_steps: vec![],
            verification: VerificationConfig {
                required_packages: vec![],
                required_services: vec![],
                required_files: vec![],
                verification_commands: vec![],
            },
            metadata: HashMap::default(),
            created: None,
            checksum: None,
        }
    }

    #[test]
    fn test_registry_creation() {
        let temp_dir = TempDir::new().unwrap();
        let registry = TemplateRegistry::new(temp_dir.path()).unwrap();

        assert_eq!(registry.list_templates().len(), 0);
    }

    #[test]
    fn register_get_manifest_verify_delete_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let img = temp_dir.path().join("in.qcow2");
        std::fs::write(&img, b"fake-disk").unwrap();

        let mut registry = TemplateRegistry::new(temp_dir.path()).unwrap();
        let manifest = minimal_manifest("demo-template");

        registry
            .register_template(&manifest, &img)
            .expect("register");

        assert!(registry.has_template("demo-template"));
        let info = registry.get_template("demo-template").expect("get");
        assert_eq!(info.name, "demo-template");
        assert_eq!(info.version, "1.0.0");
        assert!(info.path.ends_with("demo-template.qcow2"));

        let loaded = registry.get_manifest("demo-template").expect("manifest");
        assert_eq!(loaded.name, manifest.name);

        assert!(registry.verify_template("demo-template").expect("verify"));

        registry.delete_template("demo-template").expect("delete");
        assert!(!registry.has_template("demo-template"));
    }

    #[test]
    fn register_duplicate_returns_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let img = temp_dir.path().join("in.qcow2");
        std::fs::write(&img, b"x").unwrap();

        let mut registry = TemplateRegistry::new(temp_dir.path()).unwrap();
        let manifest = minimal_manifest("dup");
        registry.register_template(&manifest, &img).unwrap();

        let err = registry
            .register_template(&manifest, &img)
            .expect_err("dup");
        assert!(
            err.to_string().contains("already exists") || err.to_string().contains("AlreadyExists")
        );
    }

    #[test]
    fn get_template_not_found_maps_to_registry_error() {
        let temp_dir = TempDir::new().unwrap();
        let registry = TemplateRegistry::new(temp_dir.path()).unwrap();
        let err = registry.get_template("nope").unwrap_err();
        assert!(matches!(err, RegistryError::NotFound(_)));
    }

    #[test]
    fn verify_template_fails_when_file_removed() {
        let temp_dir = TempDir::new().unwrap();
        let img = temp_dir.path().join("in.qcow2");
        std::fs::write(&img, b"x").unwrap();

        let mut registry = TemplateRegistry::new(temp_dir.path()).unwrap();
        let manifest = minimal_manifest("gone");
        registry.register_template(&manifest, &img).unwrap();

        let path = temp_dir.path().join("templates/gone.qcow2");
        std::fs::remove_file(path).unwrap();

        let r = registry.verify_template("gone");
        assert!(r.is_err());
    }
}
