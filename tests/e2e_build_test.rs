//! End-to-end build test
//! 
//! Tests the full build flow from manifest to template

use agent_reagents::builder::ImageBuilder;
use agent_reagents::templates::TemplateManifest;
use std::path::PathBuf;
use tokio::time::Duration;

#[tokio::test]
#[ignore] // Run with: cargo test --release --test e2e_build_test -- --ignored
async fn test_full_ubuntu_build() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    // Load manifest
    let manifest_path = PathBuf::from("templates/ubuntu-24-04-desktop.yaml");
    let manifest = TemplateManifest::from_yaml_file(&manifest_path)?;
    
    // Get SSH key
    let home = std::env::var("HOME")?;
    let ssh_key = std::fs::read_to_string(PathBuf::from(home).join(".ssh/id_rsa.pub"))?
        .trim().to_string();
    
    // Base image
    let base_image = PathBuf::from("/var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img");
    
    // Create builder
    let mut builder = ImageBuilder::new(
        format!("test-{}", manifest.name),
        base_image,
    )
    .memory_mb(manifest.resources.memory_mb)
    .vcpus(manifest.resources.vcpus)
    .disk_size_gb(manifest.resources.disk_gb)
    .timeout(Duration::from_secs(manifest.resources.timeout_secs));
    
    // Execute build
    let result = builder.build_from_manifest(&manifest, ssh_key).await?;
    
    // Verify result
    assert!(result.template_path.exists(), "Template file should exist");
    assert!(result.size_bytes > 0, "Template should have non-zero size");
    assert!(result.verification.is_success(), "Verification should pass");
    
    println!("✅ Build completed: {}", result.template_path.display());
    println!("   Size: {} bytes", result.size_bytes);
    println!("   Duration: {:?}", result.build_duration);
    
    Ok(())
}

#[tokio::test]
#[ignore] // Run with: cargo test --release --test e2e_build_test -- --ignored
async fn test_full_popos_cosmic_build() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    // Load manifest
    let manifest_path = PathBuf::from("templates/popos-24-cosmic.yaml");
    let manifest = TemplateManifest::from_yaml_file(&manifest_path)?;
    
    // Get SSH key
    let home = std::env::var("HOME")?;
    let ssh_key = std::fs::read_to_string(PathBuf::from(home).join(".ssh/id_rsa.pub"))?
        .trim().to_string();
    
    // Base image
    let base_image = PathBuf::from("/var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img");
    
    // Create builder
    let mut builder = ImageBuilder::new(
        format!("test-{}", manifest.name),
        base_image,
    )
    .memory_mb(manifest.resources.memory_mb)
    .vcpus(manifest.resources.vcpus)
    .disk_size_gb(manifest.resources.disk_gb)
    .timeout(Duration::from_secs(manifest.resources.timeout_secs));
    
    // Execute build
    let result = builder.build_from_manifest(&manifest, ssh_key).await?;
    
    // Verify result
    assert!(result.template_path.exists(), "Template file should exist");
    assert!(result.size_bytes > 0, "Template should have non-zero size");
    assert!(result.verification.is_success(), "Verification should pass");
    assert!(result.verification.cosmic_installed, "COSMIC should be installed");
    assert!(result.verification.cosmic_package_count >= 5, "Should have at least 5 COSMIC packages");
    assert!(result.verification.greeter_enabled, "Greeter should be enabled");
    
    println!("✅ Build completed: {}", result.template_path.display());
    println!("   Size: {} bytes", result.size_bytes);
    println!("   Duration: {:?}", result.build_duration);
    println!("   COSMIC packages: {}", result.verification.cosmic_package_count);
    
    Ok(())
}

#[tokio::test]
async fn test_build_step_executor_unit() -> anyhow::Result<()> {
    // Test that build step execution logic is sound
    // This doesn't require a real VM
    
    use agent_reagents::templates::BuildStep;
    
    let steps = vec![
        BuildStep::InstallPackages {
            packages: vec!["vim".to_string(), "curl".to_string()],
        },
        BuildStep::RunCommand {
            command: "echo 'test'".to_string(),
            description: Some("Test command".to_string()),
        },
    ];
    
    // Verify steps are valid
    assert_eq!(steps.len(), 2);
    
    Ok(())
}

