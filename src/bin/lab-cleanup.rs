// SPDX-License-Identifier: AGPL-3.0-only
use anyhow::{Context, Result};
use benchscale::backend::libvirt::{VmRegistry, VmStatus};
use clap::{Parser, Subcommand};
use virt::connect::Connect;
use virt::domain::Domain;

fn libvirt_uri() -> String {
    benchscale::backend::libvirt_uri()
}

#[derive(Parser)]
#[command(name = "lab-cleanup")]
#[command(about = "VM lifecycle management and orphan cleanup utility")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show status of all registered VMs
    Status,
    
    /// Find and clean up orphaned VMs (creator process no longer exists)
    CleanOrphans {
        /// Actually perform cleanup (default is dry-run)
        #[arg(long)]
        execute: bool,
    },
    
    /// Find and clean up stale VMs (stuck in Creating/Building state)
    CleanStale {
        /// Maximum age in seconds (default: 3600 = 1 hour)
        #[arg(long, default_value = "3600")]
        max_age: u64,
        
        /// Actually perform cleanup (default is dry-run)
        #[arg(long)]
        execute: bool,
    },
    
    /// Clean all VMs except those in Running or Completed state
    CleanAll {
        /// Actually perform cleanup (default is dry-run)
        #[arg(long)]
        execute: bool,
    },
    
    /// Clear the registry (does not delete VMs, only registry entries)
    ClearRegistry,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Status => show_status().await?,
        Commands::CleanOrphans { execute } => clean_orphans(execute).await?,
        Commands::CleanStale { max_age, execute } => clean_stale(max_age, execute).await?,
        Commands::CleanAll { execute } => clean_all(execute).await?,
        Commands::ClearRegistry => clear_registry().await?,
    }
    
    Ok(())
}

async fn show_status() -> Result<()> {
    let registry = VmRegistry::new()?;
    let entries = registry.list_all();
    
    if entries.is_empty() {
        println!("📋 No VMs registered");
        return Ok(());
    }
    
    println!("📋 Registered VMs ({})", entries.len());
    println!("═══════════════════════════════════════════════════════════════");
    println!("{:<20} {:<12} {:<15} {:<10} {:<8}", "Name", "Status", "IP", "PID", "Age");
    println!("───────────────────────────────────────────────────────────────");
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    for entry in entries {
        let age_mins = (now - entry.created_at) / 60;
        let age_str = if age_mins < 60 {
            format!("{}m", age_mins)
        } else {
            format!("{}h", age_mins / 60)
        };
        
        let status_str = format!("{:?}", entry.status);
        let ip_str = entry.static_ip.as_deref().unwrap_or("-");
        
        // Check if process still exists
        let pid_str = if process_exists(entry.creator_pid) {
            format!("{}", entry.creator_pid)
        } else {
            format!("{}*", entry.creator_pid) // * = orphaned
        };
        
        println!(
            "{:<20} {:<12} {:<15} {:<10} {:<8}",
            entry.name, status_str, ip_str, pid_str, age_str
        );
    }
    
    println!("───────────────────────────────────────────────────────────────");
    println!("* = orphaned (creator process no longer exists)");
    
    // Show orphan and stale counts
    let orphans = registry.find_orphans();
    let stale = registry.find_stale(3600);
    
    if !orphans.is_empty() {
        println!("\n⚠️  {} orphaned VM(s) detected", orphans.len());
        println!("   Run: lab-cleanup clean-orphans --execute");
    }
    
    if !stale.is_empty() {
        println!("\n⚠️  {} stale VM(s) detected (>1h in Creating/Building)", stale.len());
        println!("   Run: lab-cleanup clean-stale --execute");
    }
    
    Ok(())
}

async fn clean_orphans(execute: bool) -> Result<()> {
    let mut registry = VmRegistry::new()?;
    let orphans = registry.find_orphans();
    
    if orphans.is_empty() {
        println!("✅ No orphaned VMs found");
        return Ok(());
    }
    
    println!("🔍 Found {} orphaned VM(s)", orphans.len());
    
    for entry in &orphans {
        println!("  • {} (PID {} not found)", entry.name, entry.creator_pid);
    }
    
    if !execute {
        println!("\n⚠️  DRY RUN - No changes made");
        println!("   Run with --execute to actually clean up");
        return Ok(());
    }
    
    println!("\n🧹 Cleaning up orphaned VMs...");
    
    let conn = Connect::open(Some(&libvirt_uri()))
        .context("Failed to connect to libvirt")?;
    
    // Collect VM names to avoid borrow issues
    let vm_names: Vec<String> = orphans.iter().map(|e| e.name.clone()).collect();
    
    for vm_name in vm_names {
        println!("  Cleaning VM '{}'...", vm_name);
        cleanup_vm(&conn, &vm_name).await?;
        registry.unregister(&vm_name)?;
        println!("  ✅ Cleaned up '{}'", vm_name);
    }
    
    println!("\n✅ Cleanup complete");
    Ok(())
}

async fn clean_stale(max_age: u64, execute: bool) -> Result<()> {
    let mut registry = VmRegistry::new()?;
    let stale = registry.find_stale(max_age);
    
    if stale.is_empty() {
        println!("✅ No stale VMs found (max age: {}s)", max_age);
        return Ok(());
    }
    
    println!("🔍 Found {} stale VM(s)", stale.len());
    
    for entry in &stale {
        let age = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(entry.updated_at);
        println!("  • {} ({:?}, age: {}s)", entry.name, entry.status, age);
    }
    
    if !execute {
        println!("\n⚠️  DRY RUN - No changes made");
        println!("   Run with --execute to actually clean up");
        return Ok(());
    }
    
    println!("\n🧹 Cleaning up stale VMs...");
    
    let conn = Connect::open(Some(&libvirt_uri()))
        .context("Failed to connect to libvirt")?;
    
    // Collect VM names to avoid borrow issues
    let vm_names: Vec<String> = stale.iter().map(|e| e.name.clone()).collect();
    
    for vm_name in vm_names {
        println!("  Cleaning VM '{}'...", vm_name);
        cleanup_vm(&conn, &vm_name).await?;
        registry.unregister(&vm_name)?;
        println!("  ✅ Cleaned up '{}'", vm_name);
    }
    
    println!("\n✅ Cleanup complete");
    Ok(())
}

async fn clean_all(execute: bool) -> Result<()> {
    let mut registry = VmRegistry::new()?;
    let all = registry.list_all();
    
    // Filter out Running and Completed VMs
    let to_clean: Vec<_> = all
        .into_iter()
        .filter(|e| !matches!(e.status, VmStatus::Running | VmStatus::Completed))
        .collect();
    
    if to_clean.is_empty() {
        println!("✅ No VMs to clean (all are Running or Completed)");
        return Ok(());
    }
    
    println!("🔍 Found {} VM(s) to clean", to_clean.len());
    
    for entry in &to_clean {
        println!("  • {} ({:?})", entry.name, entry.status);
    }
    
    if !execute {
        println!("\n⚠️  DRY RUN - No changes made");
        println!("   Run with --execute to actually clean up");
        return Ok(());
    }
    
    println!("\n🧹 Cleaning up VMs...");
    
    let conn = Connect::open(Some(&libvirt_uri()))
        .context("Failed to connect to libvirt")?;
    
    // Collect VM names to avoid borrow issues
    let vm_names: Vec<String> = to_clean.iter().map(|e| e.name.clone()).collect();
    
    for vm_name in vm_names {
        println!("  Cleaning VM '{}'...", vm_name);
        cleanup_vm(&conn, &vm_name).await?;
        registry.unregister(&vm_name)?;
        println!("  ✅ Cleaned up '{}'", vm_name);
    }
    
    println!("\n✅ Cleanup complete");
    Ok(())
}

async fn clear_registry() -> Result<()> {
    let mut registry = VmRegistry::new()?;
    let count = registry.list_all().len();
    
    if count == 0 {
        println!("✅ Registry is already empty");
        return Ok(());
    }
    
    println!("⚠️  This will clear {} registry entries", count);
    println!("   (VMs themselves will NOT be deleted)");
    println!("\n   Are you sure? Type 'yes' to continue:");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if input.trim() != "yes" {
        println!("❌ Cancelled");
        return Ok(());
    }
    
    registry.clear()?;
    println!("✅ Registry cleared");
    
    Ok(())
}

/// Cleanup a VM (destroy, undefine, remove disk)
async fn cleanup_vm(conn: &Connect, vm_name: &str) -> Result<()> {
    // 1. Destroy and undefine VM
    match Domain::lookup_by_name(conn, vm_name) {
        Ok(domain) => {
            if domain.is_active().unwrap_or(false) {
                domain.destroy().ok();
            }
            domain.undefine().ok();
        }
        Err(_) => {
            // VM not found, that's okay
        }
    }
    
    let images_dir = benchscale::constants::paths::libvirt_images_dir();

    std::fs::remove_file(images_dir.join(format!("{vm_name}.qcow2"))).ok();
    std::fs::remove_file(images_dir.join(format!("{vm_name}-cidata.iso"))).ok();
    
    Ok(())
}

/// Check if a process exists via `/proc` (Linux-only, no subprocess spawn).
fn process_exists(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

