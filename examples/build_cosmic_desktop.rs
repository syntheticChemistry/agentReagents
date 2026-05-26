// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build a COSMIC desktop image with verification
//!
//! This example demonstrates the modern Rust-based image builder
//! that replaces the old bash scripts.

use agent_reagents::builder::ImageBuilder;
use agent_reagents::images::ImageManager;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for observability
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  COSMIC Desktop Builder - Modern Rust Implementation        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Find Ubuntu 24.04 cloud image
    let reagents_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let image_manager = ImageManager::new(&reagents_root);

    println!("🔍 Looking for Ubuntu 24.04 cloud image...");
    let base_image = image_manager
        .find_cloud_image("ubuntu-24.04")
        .await?
        .context("Ubuntu 24.04 cloud image not found. Run: scripts/download-cloud-images.sh")?;

    println!("✅ Found: {}", base_image.name);
    println!("   Path: {:?}", base_image.path);
    println!();

    // Get SSH public key
    let ssh_key = get_ssh_public_key()?;
    println!("🔑 Using SSH key for VM access");
    println!();

    // Create manifest for COSMIC desktop build
    use agent_reagents::templates::{ResourceConfig, TemplateManifest, VerificationConfig};
    let manifest = TemplateManifest {
        name: "popos-cosmic-desktop".to_string(),
        version: "1.0.0".to_string(),
        base_image: base_image.path.to_string_lossy().to_string(),
        golden_image: None,
        description: Some("COSMIC Desktop build".to_string()),
        resources: ResourceConfig {
            memory_mb: 4096,
            vcpus: 2,
            disk_gb: 30,
            timeout_secs: 2400,
            static_ip: None,
        },
        pci_passthrough: vec![],
        users: vec![],
        build_steps: vec![],
        post_boot_steps: vec![],
        verification: VerificationConfig {
            required_packages: vec![],
            required_services: vec![],
            required_files: vec![],
            verification_commands: vec![],
        },
        package_manager: Default::default(),
        metadata: std::collections::HashMap::new(),
        created: None,
        checksum: None,
    };

    // Create manifest-driven builder
    let mut builder = ImageBuilder::from_manifest(manifest);

    println!("🚀 Starting COSMIC desktop build...");
    println!("   Memory: 4096 MB");
    println!("   vCPUs: 2");
    println!("   Disk: 30 GB");
    println!("   Timeout: 40 minutes");
    println!();

    // Build with progress tracking
    println!("📊 Build progress will be displayed...");
    println!();

    match builder.build(ssh_key).await {
        Ok(result) => {
            println!();
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║  ✅ BUILD SUCCESSFUL                                         ║");
            println!("╚══════════════════════════════════════════════════════════════╝");
            println!();
            println!("📍 Template: {:?}", result.template_path);
            println!(
                "📊 Size: {:.2} GB",
                result.size_bytes as f64 / 1_073_741_824.0
            );
            println!("⏱️  Duration: {:?}", result.build_duration);
            println!();
            println!("✅ Verification:");
            println!("{}", result.verification.summary());
            println!();
            println!("🎯 Ready to use with benchScale!");
        }
        Err(e) => {
            eprintln!();
            eprintln!("╔══════════════════════════════════════════════════════════════╗");
            eprintln!("║  ❌ BUILD FAILED                                             ║");
            eprintln!("╚══════════════════════════════════════════════════════════════╝");
            eprintln!();
            eprintln!("Error: {:?}", e);
            eprintln!();
            bail!("Build failed");
        }
    }

    Ok(())
}

/// Get SSH public key from ~/.ssh/id_rsa.pub or generate one
fn get_ssh_public_key() -> Result<String> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let ssh_key_path = PathBuf::from(home).join(".ssh/id_rsa.pub");

    if ssh_key_path.exists() {
        let key =
            std::fs::read_to_string(&ssh_key_path).context("Failed to read SSH public key")?;
        Ok(key.trim().to_string())
    } else {
        bail!(
            "SSH public key not found at {:?}. Generate one with: ssh-keygen -t rsa",
            ssh_key_path
        );
    }
}
