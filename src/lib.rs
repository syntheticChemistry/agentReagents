//! agentReagents - VM Image and Package Management
//!
//! Provides reusable VM images and software packages for the biomeOS ecosystem.
//!
//! # Modules
//!
//! - `builder` - Image builder with async state machine and verification
//! - `images` - Image management and discovery
//! - `packages` - Package management
//!
//! # Example
//!
//! ```no_run
//! use agent_reagents::builder::ImageBuilder;
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut builder = ImageBuilder::new(
//!         "cosmic-desktop",
//!         PathBuf::from("/path/to/ubuntu-24.04.img")
//!     );
//!     
//!     let result = builder
//!         .memory(4096)
//!         .vcpus(2)
//!         .build_cosmic_desktop("ssh-rsa AAAA...".to_string())
//!         .await?;
//!     
//!     println!("Template created: {:?}", result.template_path);
//!     Ok(())
//! }
//! ```

pub mod builder;
pub mod images;
pub mod packages;
pub mod templates;
pub mod discovery;

// Re-export commonly used types
pub use builder::{BuildResult, BuildState, ImageBuilder, VerificationResult};
pub use discovery::ReagentsProvider;
