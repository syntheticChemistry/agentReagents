//! Build a COSMIC desktop image with verification
//! 
//! This example demonstrates the modern Rust-based image builder
//! that replaces the old bash scripts.

use agent_reagents::builder::ImageBuilder;
use agent_reagents::images::ImageManager;
use std::path::PathBuf;
use anyhow::{Result, Context, bail};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for observability
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  COSMIC Desktop Builder - Modern Rust Implementation        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Find Ubuntu 24.04 cloud image
    let reagents_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let image_manager = ImageManager::new(&reagents_root);
    
    println!("🔍 Looking for Ubuntu 24.04 cloud image...");
    let base_image = image_manager.find_cloud_image("ubuntu-24.04")
        .await?
        .context("Ubuntu 24.04 cloud image not found. Run: scripts/download-cloud-images.sh")?;
    
    println!("✅ Found: {}", base_image.name);
    println!("   Path: {:?}", base_image.path);
    println!();

    // Get SSH public key
    let ssh_key = get_ssh_public_key()?;
    println!("🔑 Using SSH key for VM access");
    println!();

    // Create builder
    let mut builder = ImageBuilder::new(
        "popos-cosmic-desktop",
        base_image.path
    )
    .memory(4096)
    .vcpus(2)
    .disk_size(30);

    println!("🚀 Starting COSMIC desktop build...");
    println!("   Memory: 4096 MB");
    println!("   vCPUs: 2");
    println!("   Disk: 30 GB");
    println!("   Timeout: 40 minutes");
    println!();

    // Build with progress tracking
    println!("📊 Build progress will be displayed...");
    println!();

    match builder.build_cosmic_desktop(ssh_key).await {
        Ok(result) => {
            println!();
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║  ✅ BUILD SUCCESSFUL                                         ║");
            println!("╚══════════════════════════════════════════════════════════════╝");
            println!();
            println!("📍 Template: {:?}", result.template_path);
            println!("📊 Size: {:.2} GB", result.size_bytes as f64 / 1_073_741_824.0);
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
        let key = std::fs::read_to_string(&ssh_key_path)
            .context("Failed to read SSH public key")?;
        Ok(key.trim().to_string())
    } else {
        bail!("SSH public key not found at {:?}. Generate one with: ssh-keygen -t rsa", ssh_key_path);
    }
}

