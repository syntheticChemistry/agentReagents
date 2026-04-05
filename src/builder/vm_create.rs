// SPDX-License-Identifier: AGPL-3.0-or-later

//! Builder VM creation via libvirt / benchScale (domain, disks, cloud-init attachment).

use anyhow::{Context, Result};
use benchscale::CloudInit;
use benchscale::backend::LibvirtBackend;
use benchscale::backend::libvirt::VmGuard;
use benchscale::backend::senescence::SenescenceMonitor;
use benchscale::config::{BenchScaleConfig, MonitoringConfig, TimeoutConfig};
use std::path::Path;
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{info, warn};

use super::ImageBuilder;
use super::network::NetworkMonitor;
use super::post_boot;
use super::vm_handle::VmHandle;

impl ImageBuilder {
    /// Create the builder VM using benchScale with manifest configuration
    ///
    /// Deep debt: Uses runtime capability discovery via benchScale backend
    /// instead of hardcoded paths and IPs.
    ///
    /// Returns both the VmHandle and a VmGuard for automatic cleanup on failure.
    #[expect(
        clippy::too_many_lines,
        reason = "VM bring-up: senescence monitor, cloud-init, network, post-boot"
    )]
    pub(super) async fn create_builder_vm(
        &self,
        cloud_init: CloudInit,
    ) -> Result<(VmHandle, VmGuard)> {
        info!("Creating builder VM: {}", self.manifest.name);

        // Create LibvirtBackend - it discovers capabilities at runtime
        let backend = LibvirtBackend::new().context("Failed to create libvirt backend")?;

        // EVOLUTION #20: Ensure libvirt infrastructure is healthy before VM creation
        // This prevents failures from orphaned processes or network corruption
        info!("🔍 Checking libvirt infrastructure health...");
        backend
            .ensure_healthy()
            .await
            .context("libvirt infrastructure health check failed")?;
        info!("✅ Infrastructure health verified");

        // Get base image path from manifest
        let base_image = Path::new(&self.manifest.base_image);

        // Convert manifest PCI passthrough configs to benchScale types
        let pci_devices: Vec<benchscale::PciPassthroughDevice> = self
            .manifest
            .pci_passthrough
            .iter()
            .map(|p| benchscale::PciPassthroughDevice { bdf: p.bdf.clone() })
            .collect();

        // Create VM using benchScale with manifest-defined resources
        let mut node = backend
            .create_desktop_vm_with_pci(
                &self.manifest.name,
                base_image,
                &cloud_init,
                self.manifest
                    .resources
                    .memory_mb
                    .try_into()
                    .context("Invalid memory size")?,
                self.manifest
                    .resources
                    .vcpus
                    .try_into()
                    .context("Invalid vCPU count")?,
                self.manifest
                    .resources
                    .disk_gb
                    .try_into()
                    .context("Invalid disk size")?,
                self.manifest.resources.static_ip.clone(),
                &pci_devices,
            )
            .await
            .context("Failed to create desktop VM")?;

        info!("VM created: {} (pool IP: {})", node.name, node.ip_address);

        // Create VmGuard for automatic cleanup on failure
        // Evolution #17: Capability-based path discovery (no hardcoded /var/lib/libvirt)
        // DEEP DEBT SOLUTION: VM will be automatically cleaned up if builder panics or fails
        // Note: We create a new connection for the guard since benchScale uses Arc<Mutex<Connect>>
        let images_dir = backend.capabilities().storage.images_dir.clone();
        let guard = VmGuard::new(
            self.manifest.name.clone(),
            backend.raw_connection()?, // Get the raw connection from backend
            images_dir,                // Evolution #17: Pass discovered path, not hardcoded
        );
        info!("🔒 VmGuard enabled: VM will be cleaned up on build failure");

        // Evolution #15: Cloud-Init Timing Deep Debt Fix
        //
        // ARCHITECTURAL FIX: Wait for cloud-init BEFORE network verification
        //
        // Problem: Ping timeout (60s) << cloud-init time (10+ min for desktop)
        // Solution: Let cloud-init complete, THEN verify network
        //
        // Modern idiomatic approach:
        // 1. benchScale discovers DHCP IP via lease query (instant)
        // 2. Wait for cloud-init to complete (package installation, SSH setup)
        // 3. THEN verify network connectivity
        info!(
            "⏳ VM created with DHCP IP: {} - waiting for cloud-init...",
            node.ip_address
        );

        let actual_ip = node.ip_address.clone();

        // Step 1: Advanced senescence monitoring for cloud-init (DHCP-aware)
        // Use continuous health monitoring instead of polling
        info!("⏳ Starting VM senescence monitoring for cloud-init...");
        println!("⏳ Starting advanced VM monitoring (senescence tracking enabled)...");

        let vm_name = node.name.clone();

        // Evolution #22: Extract MAC address for DHCP lease tracking
        let mac_address = node.metadata.get("mac_address").cloned();
        if let Some(ref mac) = mac_address {
            info!(
                "Evolution #22: MAC address {} will be tracked for IP changes",
                mac
            );
        }

        // Phase 2B: Use configuration system for monitoring behavior
        // Evolution #21: Configurable max_failures for cloud-init (30min tolerance)
        // Evolution #22: MAC address tracking for periodic IP re-discovery
        let config = BenchScaleConfig {
            monitoring: MonitoringConfig::for_cloud_init_packages(),
            timeouts: TimeoutConfig::default(),
            ..Default::default()
        };

        let monitor = Arc::new(SenescenceMonitor::from_config(
            vm_name,
            actual_ip.clone(),
            mac_address,
            &config.monitoring,
        ));

        // Start background monitoring
        // Phase 1A: Use helper method with proper logging
        let username = self.get_username_with_fallback().to_string();

        let monitor_handle = monitor.clone().start_monitoring(username.clone()).await;

        // Wait for cloud-init with progress callbacks
        // Phase 2B: Use config for timeout
        let cloud_init_timeout = config.timeouts.cloud_init();
        info!(
            "Waiting for cloud-init with {}min timeout",
            cloud_init_timeout.as_secs() / 60
        );
        println!(
            "   ⏱️  Timeout: {}min | Monitoring: ping, SSH, cloud-init status",
            cloud_init_timeout.as_secs() / 60
        );

        match monitor
            .wait_for_cloud_init(cloud_init_timeout, |metrics| {
                // Progress callback - called periodically
                println!(
                    "   📊 Health: {:?} | Ping: {} | SSH: {} | Uptime: {}s | Failures: {}",
                    metrics.health,
                    if metrics.ping_ok { "✓" } else { "✗" },
                    if metrics.ssh_ok { "✓" } else { "✗" },
                    metrics.uptime.as_secs(),
                    metrics.consecutive_failures
                );

                if let Some(ref cloud_init) = metrics.cloud_init {
                    println!(
                        "   🔧 Cloud-init: {} {}",
                        cloud_init.status,
                        cloud_init.detail.as_deref().unwrap_or("")
                    );
                }
            })
            .await
        {
            Ok(()) => {
                info!("✅ Cloud-init completed successfully");
                println!("✅ Cloud-init completed successfully!");
            }
            Err(e) => {
                warn!(
                    "⚠️ Cloud-init monitoring ended: {} (VM may still be usable)",
                    e
                );
                println!("⚠️  Cloud-init monitoring ended, but VM appears to be running");

                // Check if VM is at least responsive
                if !monitor.is_healthy().await {
                    monitor_handle.abort();
                    return Err(e).context("VM became unhealthy during cloud-init");
                }
            }
        }

        // Step 2: Network verification AFTER cloud-init (Evolution #15)
        // Now that cloud-init has completed, verify network is stable
        info!("🔧 Network Resilience: Verifying network stability after cloud-init");

        // Evolution #22 Part 2: Get CURRENT IP from senescence monitor
        // (IP may have changed during long cloud-init process)
        let current_metrics = monitor.metrics().await;
        let current_ip = &current_metrics.ip_address;
        if current_ip != &actual_ip {
            info!(
                "Evolution #22: Using updated IP {} for verification (was {})",
                current_ip, actual_ip
            );
        }

        let network_monitor = NetworkMonitor::new(current_ip)
            .with_check_interval(Duration::from_secs(10))
            .with_max_failures(3)
            .with_check_timeout(Duration::from_secs(5));

        info!("🔍 Verifying network stability via ping + SSH...");
        network_monitor
            .verify_once(&username, 5, Duration::from_secs(5))
            .await
            .context("Network verification failed after cloud-init completion")?;

        info!("✅ Network verified and stable");

        // Stop monitoring
        monitor_handle.abort();

        info!("✅ VM senescence monitoring complete");

        info!(
            "✅ Final IP: {} (initial: {}, pool allocated: {})",
            current_ip, actual_ip, node.ip_address
        );

        // Evolution #22: Use current IP (may have changed during build)
        node.ip_address = current_ip.clone();

        info!("VM ready: {} (IP: {})", node.name, node.ip_address);

        let vm_handle = VmHandle::new(backend, node);

        // Execute post-boot steps (the "add heat-sensitive compounds" phase)
        if !self.manifest.post_boot_steps.is_empty() {
            info!("🧪 Starting post-boot synthesis phase...");
            println!("🧪 Executing post-boot steps (laboratory stepwise synthesis)...");

            // Phase 1A: Use helper method with proper logging
            let username = self.get_username_with_fallback();

            // SUDO-FREE ARCHITECTURE: Filter post_boot_steps to remove packages
            // that were already installed via cloud-init
            let filtered_steps = self.filter_post_boot_steps();
            info!(
                "Filtered post-boot steps: {} steps remaining (packages moved to cloud-init)",
                filtered_steps.len()
            );

            post_boot::execute_post_boot_steps(&vm_handle, &filtered_steps, username)
                .await
                .context("Failed to execute post-boot steps")?;
            println!("✅ Post-boot synthesis complete!");
        }

        Ok((vm_handle, guard))
    }
}
