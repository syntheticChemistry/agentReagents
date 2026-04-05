// SPDX-License-Identifier: AGPL-3.0-or-later
//! agent-reagents CLI tool
//!
//! Reproducible VM image management with manifest-driven builds

use agent_reagents::templates::{TemplateManifest, TemplateRegistry};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "agent-reagents")]
#[command(about = "Reproducible VM image management", long_about = None)]
#[command(version)]
struct Cli {
    /// Registry directory
    #[arg(short, long, default_value = "./reagents")]
    registry: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available templates
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show template information
    Info {
        /// Template name
        name: String,
    },

    /// Build a template from manifest
    Build {
        /// Manifest file (YAML)
        manifest: PathBuf,

        /// SSH public key for access
        #[arg(short, long)]
        ssh_key: Option<String>,

        /// SSH public key file
        #[arg(short = 'k', long)]
        ssh_key_file: Option<PathBuf>,
    },

    /// Verify a template
    Verify {
        /// Template name
        name: String,
    },

    /// Delete a template
    Delete {
        /// Template name
        name: String,

        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Validate a manifest file
    Validate {
        /// Manifest file (YAML)
        manifest: PathBuf,
    },

    /// Start JSON-RPC 2.0 server (UniBin compliance)
    Server {
        /// TCP port to listen on
        #[arg(long)]
        port: u16,
        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        listen: String,
        /// Run in standalone mode (no registry registration)
        #[arg(long, default_value_t = true)]
        standalone: bool,
        /// Logical service name for capability-based registry registration (`REGISTRY_SERVICE_NAME`)
        #[arg(long, env = "REGISTRY_SERVICE_NAME", default_value = "agent-reagents")]
        service_name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::List { verbose } => {
            cmd_list(&cli.registry, verbose).await?;
        }

        Commands::Info { name } => {
            cmd_info(&cli.registry, &name).await?;
        }

        Commands::Build {
            manifest,
            ssh_key,
            ssh_key_file,
        } => {
            cmd_build(&cli.registry, &manifest, ssh_key, ssh_key_file).await?;
        }

        Commands::Verify { name } => {
            cmd_verify(&cli.registry, &name).await?;
        }

        Commands::Delete { name, yes } => {
            cmd_delete(&cli.registry, &name, yes).await?;
        }

        Commands::Validate { manifest } => {
            cmd_validate(&manifest).await?;
        }

        Commands::Server {
            port,
            listen,
            standalone,
            service_name,
        } => {
            let addr: std::net::SocketAddr = format!("{listen}:{port}")
                .parse()
                .expect("invalid listen address");
            let registration =
                agent_reagents::server::RegistrationSettings::with_default_socket(service_name);
            agent_reagents::server::run_server(addr, cli.registry, standalone, registration).await?;
        }
    }

    Ok(())
}

async fn cmd_list(registry_dir: &Path, verbose: bool) -> Result<()> {
    let registry = TemplateRegistry::new(registry_dir)?;
    let templates = registry.list_templates();

    if templates.is_empty() {
        println!("No templates found.");
        return Ok(());
    }

    println!("Available templates:");
    println!();

    for template in templates {
        if verbose {
            println!("  {} v{}", template.name, template.version);
            println!("    Path: {}", template.path.display());
            println!("    Size: {} bytes", template.size_bytes);
            println!("    Checksum: {}", template.checksum);
            println!(
                "    Verified: {}",
                if template.verified { "✓" } else { "✗" }
            );
            println!();
        } else {
            let status = if template.verified { "✓" } else { "✗" };
            println!("  {} {} v{}", status, template.name, template.version);
        }
    }

    Ok(())
}

async fn cmd_info(registry_dir: &Path, name: &str) -> Result<()> {
    let registry = TemplateRegistry::new(registry_dir)?;

    let info = registry
        .get_template(name)
        .map_err(|e| anyhow::anyhow!(e))?;

    let manifest = registry.get_manifest(name)?;

    println!("Template: {}", info.name);
    println!("Version: {}", info.version);
    println!("Path: {}", info.path.display());
    println!(
        "Size: {} bytes ({:.2} MB)",
        info.size_bytes,
        info.size_bytes as f64 / 1_048_576.0
    );
    println!("Checksum: {}", info.checksum);
    println!("Verified: {}", if info.verified { "✓" } else { "✗" });
    println!();

    if let Some(desc) = &manifest.description {
        println!("Description: {}", desc);
        println!();
    }

    println!("Resources:");
    println!("  Memory: {} MB", manifest.resources.memory_mb);
    println!("  vCPUs: {}", manifest.resources.vcpus);
    println!("  Disk: {} GB", manifest.resources.disk_gb);
    println!();

    println!("Build steps: {}", manifest.build_steps.len());
    println!(
        "Verification checks: {}",
        manifest.verification.required_packages.len()
            + manifest.verification.required_services.len()
            + manifest.verification.required_files.len()
            + manifest.verification.verification_commands.len()
    );

    Ok(())
}

async fn cmd_build(
    _registry_dir: &Path,
    manifest_path: &Path,
    ssh_key: Option<String>,
    ssh_key_file: Option<PathBuf>,
) -> Result<()> {
    use agent_reagents::builder::ImageBuilder;

    // AUTO-CLEANUP: Clean up orphaned VMs before starting build
    info!("🧹 Running pre-build cleanup...");
    if let Err(e) = cleanup_orphaned_vms().await {
        // Non-fatal - log warning and continue
        tracing::warn!("Pre-build cleanup failed: {}", e);
    }

    // Load manifest
    let manifest =
        TemplateManifest::from_yaml_file(manifest_path).context("Failed to load manifest")?;

    // Validate manifest
    manifest.validate().context("Manifest validation failed")?;

    info!("Building template: {} v{}", manifest.name, manifest.version);
    println!("📦 Template: {} v{}", manifest.name, manifest.version);
    if let Some(desc) = &manifest.description {
        println!("📝 Description: {}", desc);
    }

    // Get SSH key
    let ssh_key = if let Some(key) = ssh_key {
        key
    } else if let Some(key_file) = ssh_key_file {
        std::fs::read_to_string(key_file)
            .context("Failed to read SSH key file")?
            .trim()
            .to_string()
    } else {
        // Try default SSH key
        let default_key = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".ssh/id_rsa.pub");

        if default_key.exists() {
            info!("Using default SSH key: {}", default_key.display());
            std::fs::read_to_string(default_key)
                .context("Failed to read default SSH key")?
                .trim()
                .to_string()
        } else {
            anyhow::bail!("No SSH key provided. Use --ssh-key or --ssh-key-file");
        }
    };

    // Get base image path
    let base_image = PathBuf::from(&manifest.base_image);
    if !base_image.exists() {
        anyhow::bail!("Base image not found: {}", base_image.display());
    }

    println!("🖼️  Base image: {}", base_image.display());
    println!();
    println!("Starting build process...");
    println!();

    // Create manifest-driven builder
    // Deep debt solution: All builds are now declarative and manifest-driven
    let timeout = std::time::Duration::from_secs(manifest.resources.timeout_secs);
    let mut builder = ImageBuilder::from_manifest(manifest).with_timeout(timeout);

    // Execute build (NOTE: build_cosmic_desktop is deprecated, use generic build() in future)
    #[expect(deprecated, reason = "build_cosmic_desktop retained until manifest-driven build() is the default CLI path")]
    let result = builder
        .build_cosmic_desktop(ssh_key)
        .await
        .context("Build failed")?;

    println!();
    println!("✅ Build completed successfully!");
    println!("   Template: {}", result.template_path.display());
    println!(
        "   Size: {} bytes ({:.2} MB)",
        result.size_bytes,
        result.size_bytes as f64 / 1024.0 / 1024.0
    );
    println!("   Duration: {:?}", result.build_duration);
    println!();
    println!("Verification:");
    println!("{}", result.verification.summary());

    Ok(())
}

async fn cmd_verify(registry_dir: &Path, name: &str) -> Result<()> {
    let registry = TemplateRegistry::new(registry_dir)?;

    info!("Verifying template: {}", name);

    let is_valid = registry.verify_template(name)?;

    if is_valid {
        println!("✓ Template checksum verified: {}", name);
    } else {
        error!("✗ Template checksum verification failed: {}", name);
        anyhow::bail!("Checksum mismatch");
    }

    Ok(())
}

async fn cmd_delete(registry_dir: &Path, name: &str, yes: bool) -> Result<()> {
    let mut registry = TemplateRegistry::new(registry_dir)?;

    // Check if template exists
    let _ = registry
        .get_template(name)
        .map_err(|e| anyhow::anyhow!(e))?;

    // Confirm deletion
    if !yes {
        println!("Are you sure you want to delete template '{}'? [y/N]", name);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    registry.delete_template(name)?;
    println!("✓ Deleted template: {}", name);

    Ok(())
}

async fn cmd_validate(manifest_path: &Path) -> Result<()> {
    info!("Validating manifest: {}", manifest_path.display());

    let manifest =
        TemplateManifest::from_yaml_file(manifest_path).context("Failed to load manifest")?;

    manifest.validate().context("Manifest validation failed")?;

    println!("✓ Manifest is valid");
    println!("  Name: {}", manifest.name);
    println!("  Version: {}", manifest.version);
    println!("  Base image: {}", manifest.base_image);
    println!("  Build steps: {}", manifest.build_steps.len());

    Ok(())
}

/// Auto-cleanup orphaned VMs before starting a build
/// This prevents resource leaks and ensures a clean starting state
#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::{CommandFactory, Parser};
    use std::path::Path;

    #[test]
    fn cli_parses_list_and_validate() {
        let cli = Cli::try_parse_from([
            "agent-reagents",
            "-r",
            "/tmp/reagents",
            "list",
            "--verbose",
        ])
        .expect("parse");
        assert!(matches!(cli.command, Commands::List { verbose: true }));

        let v = Cli::try_parse_from([
            "agent-reagents",
            "validate",
            "/path/to/m.yaml",
        ])
        .expect("validate");
        assert!(matches!(v.command, Commands::Validate { .. }));
    }

    #[test]
    fn cli_build_accepts_key_flags() {
        let b = Cli::try_parse_from([
            "agent-reagents",
            "build",
            "manifest.yaml",
            "--ssh-key",
            "ssh-ed25519 AAA",
        ])
        .expect("build");
        match b.command {
            Commands::Build {
                ssh_key,
                ssh_key_file,
                ..
            } => {
                assert_eq!(ssh_key.as_deref(), Some("ssh-ed25519 AAA"));
                assert!(ssh_key_file.is_none());
            }
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn cli_server_command_factory() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_build_manifest_path() {
        let b = Cli::try_parse_from(["agent-reagents", "build", "./m.yaml"]).expect("build");
        match b.command {
            Commands::Build { manifest, .. } => {
                assert_eq!(manifest, Path::new("./m.yaml"));
            }
            _ => panic!("expected Build"),
        }
    }
}

async fn cleanup_orphaned_vms() -> Result<()> {
    use benchscale::backend::libvirt::VmRegistry;
    use virt::connect::Connect;
    use virt::domain::Domain;

    let mut registry = VmRegistry::new()?;
    let orphans = registry.find_orphans();

    if orphans.is_empty() {
        info!("   ✅ No orphaned VMs found");
        return Ok(());
    }

    info!(
        "   🔍 Found {} orphaned VM(s), cleaning up...",
        orphans.len()
    );

    // Collect VM names to clean (avoid borrow issues)
    let vm_names: Vec<String> = orphans.iter().map(|e| e.name.clone()).collect();

    let conn = Connect::open(Some("qemu:///system")).context("Failed to connect to libvirt")?;

    let mut cleaned = 0;
    for vm_name in vm_names {
        info!("   • Cleaning up orphaned VM: {}", vm_name);

        // Destroy and undefine VM
        if let Ok(domain) = Domain::lookup_by_name(&conn, &vm_name) {
            if domain.is_active().unwrap_or(false) {
                let _ = domain.destroy();
            }
            let _ = domain.undefine();
        }

        // Remove disk
        let disk_path = format!("/var/lib/libvirt/images/{}.qcow2", vm_name);
        let _ = std::fs::remove_file(&disk_path);

        // Remove cloud-init ISO
        let iso_path = format!("/var/lib/libvirt/images/{}-cidata.iso", vm_name);
        let _ = std::fs::remove_file(&iso_path);

        // Unregister from registry
        registry.unregister(&vm_name)?;
        cleaned += 1;
    }

    info!("   ✅ Cleaned up {} orphaned VM(s)", cleaned);
    Ok(())
}
