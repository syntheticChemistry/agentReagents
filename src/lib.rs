// SPDX-License-Identifier: AGPL-3.0-or-later
//! agentReagents - VM Image and Package Management
//!
//! Provides reusable VM images and software packages for the biomeOS ecosystem.
//!
//! # Modules
//!
//! - `builder` - Image builder with async state machine and verification
//! - `images` - Image management
//! - `packages` - Package management
//! - `templates` - Template manifest system
//!
//! # Service Discovery
//!
//! **Note**: agentReagents does NOT provide custom service discovery.
//!
//! For runtime backend selection, use standard solutions:
//! - **mDNS/DNS-SD**: Local network discovery (Avahi, Bonjour)
//! - **Consul**: Distributed service registry
//! - **Environment variables**: Explicit configuration (recommended)
//!
//! # Example
//!
//! ```no_run
//! use agent_reagents::builder::ImageBuilder;
//! use agent_reagents::templates::{ResourceConfig, TemplateManifest, VerificationConfig};
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let manifest = TemplateManifest {
//!         name: "cosmic-desktop".to_string(),
//!         version: "1.0.0".to_string(),
//!         base_image: "/path/to/ubuntu-24.04.img".to_string(),
//!         description: None,
//!         resources: ResourceConfig {
//!             memory_mb: 4096,
//!             vcpus: 2,
//!             disk_gb: 30,
//!             timeout_secs: 2400,
//!             static_ip: None,
//!         },
//!         pci_passthrough: vec![],
//!         package_manager: Default::default(),
//!         users: vec![],
//!         build_steps: vec![],
//!         post_boot_steps: vec![],
//!         verification: VerificationConfig {
//!             required_packages: vec![],
//!             required_services: vec![],
//!             required_files: vec![],
//!             verification_commands: vec![],
//!         },
//!         metadata: HashMap::new(),
//!         created: None,
//!         checksum: None,
//!     };
//!     let mut builder = ImageBuilder::from_manifest(manifest);
//!     let result = builder
//!         .build_cosmic_desktop("ssh-rsa AAAA...".to_string())
//!         .await?;
//!     println!("Template created: {:?}", result.template_path);
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_async)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::option_if_let_else)]

/// Manifest-driven VM image builder, verification, and post-boot steps.
pub mod builder;
/// Image listing and discovery under the reagents directory.
pub mod images;
/// Package file discovery (.deb, etc.) under the reagents tree.
pub mod packages;
/// JSON-RPC 2.0 server (UniBin `server --port` mode).
pub mod server;
/// Template manifests, registry, and YAML definitions.
pub mod templates;

// Discovery: Use standard solutions (mDNS, DNS-SD, Consul)
// NOT creating custom substrate - primal philosophy is to use existing capabilities
// Archived: src/discovery.rs (was using phantom primal-substrate dependency)

// Re-export commonly used types
pub use builder::{BuildResult, BuildState, ImageBuilder, VerificationResult};
pub use server::RegistrationSettings;
