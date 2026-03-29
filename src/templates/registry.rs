// SPDX-License-Identifier: AGPL-3.0-only
//! Template registry for managing templates
//!
//! Provides storage and retrieval of template manifests with checksums.

use super::manifest::TemplateManifest;
use super::TemplateInfo;
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
    use tempfile::TempDir;

    #[test]
    fn test_registry_creation() {
        let temp_dir = TempDir::new().unwrap();
        let registry = TemplateRegistry::new(temp_dir.path()).unwrap();

        assert_eq!(registry.list_templates().len(), 0);
    }
}
