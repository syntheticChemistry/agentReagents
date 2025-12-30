//! Template Registry System
//! 
//! Provides reproducible template management with manifests, verification,
//! and checksums. This replaces bash scripts with type-safe Rust.

mod manifest;
mod registry;

pub use manifest::{TemplateManifest, BuildStep};
pub use registry::{TemplateRegistry, RegistryError};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Template name
    pub name: String,
    /// Template version
    pub version: String,
    /// Path to template image
    pub path: PathBuf,
    /// Size in bytes
    pub size_bytes: u64,
    /// SHA256 checksum
    pub checksum: String,
    /// Verification status
    pub verified: bool,
}

