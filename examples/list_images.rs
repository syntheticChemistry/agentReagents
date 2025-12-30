//! List available images in agentReagents

use agent_reagents::images::ImageManager;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let reagents_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manager = ImageManager::new(&reagents_root);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  agentReagents Image Inventory                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // List cloud images
    println!("📦 Cloud Images:");
    match manager.list_cloud_images().await {
        Ok(images) => {
            if images.is_empty() {
                println!("   (none found)");
            } else {
                for img in images {
                    let size_mb = img.size_bytes as f64 / 1_048_576.0;
                    println!("   • {} ({:.1} MB)", img.name, size_mb);
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // List ISOs
    println!("💿 ISO Images:");
    match manager.list_iso_images().await {
        Ok(images) => {
            if images.is_empty() {
                println!("   (none found)");
            } else {
                for img in images {
                    let size_gb = img.size_bytes as f64 / 1_073_741_824.0;
                    println!("   • {} ({:.2} GB)", img.name, size_gb);
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // List templates
    println!("🎨 Templates:");
    match manager.list_templates().await {
        Ok(images) => {
            if images.is_empty() {
                println!("   (none found)");
            } else {
                for img in images {
                    let size_gb = img.size_bytes as f64 / 1_073_741_824.0;
                    println!("   • {} ({:.2} GB)", img.name, size_gb);
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // Search for ubuntu
    println!("🔍 Searching for 'ubuntu-24' cloud images...");
    match manager.find_cloud_image("ubuntu-24").await? {
        Some(img) => {
            println!("   ✅ Found: {}", img.name);
            println!("      Path: {:?}", img.path);
        }
        None => {
            println!("   ❌ Not found");
        }
    }

    Ok(())
}

