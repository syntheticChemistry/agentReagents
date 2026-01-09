//! Network monitoring and resilience for VM builds
//!
//! Provides continuous network verification and automatic recovery mechanisms
//! to ensure reliable VM provisioning even when network issues occur.

use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::builder::VmHandle;

/// Network monitoring and recovery for VM builds
///
/// This module provides resilience against network disruptions during VM provisioning.
/// It continuously monitors network connectivity and attempts automatic recovery
/// when issues are detected.
#[derive(Debug, Clone)]
pub struct NetworkMonitor {
    /// Target VM IP address
    vm_ip: String,
    /// How often to check connectivity
    check_interval: Duration,
    /// Maximum consecutive failures before attempting recovery
    max_failures: usize,
    /// Timeout for each connectivity check
    check_timeout: Duration,
}

impl NetworkMonitor {
    /// Create a new network monitor
    ///
    /// # Arguments
    /// * `vm_ip` - IP address of the VM to monitor
    /// * `check_interval` - How often to check connectivity (default: 10s)
    /// * `max_failures` - Max consecutive failures before recovery (default: 3)
    pub fn new(vm_ip: impl Into<String>) -> Self {
        Self {
            vm_ip: vm_ip.into(),
            check_interval: Duration::from_secs(10),
            max_failures: 3,
            check_timeout: Duration::from_secs(5),
        }
    }

    /// Set the check interval
    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Set the maximum consecutive failures
    pub fn with_max_failures(mut self, max: usize) -> Self {
        self.max_failures = max;
        self
    }

    /// Set the check timeout
    pub fn with_check_timeout(mut self, timeout: Duration) -> Self {
        self.check_timeout = timeout;
        self
    }

    /// Check if network is accessible via ping
    ///
    /// Returns Ok(()) if ping succeeds, Err otherwise
    pub async fn check_ping(&self) -> Result<()> {
        debug!("Checking network connectivity to {} via ping", self.vm_ip);

        let output = tokio::time::timeout(
            self.check_timeout,
            tokio::process::Command::new("ping")
                .args(["-c", "1", "-W", "2", &self.vm_ip])
                .output(),
        )
        .await
        .context("Ping command timed out")?
        .context("Failed to execute ping command")?;

        if output.status.success() {
            debug!("✅ Ping to {} successful", self.vm_ip);
            Ok(())
        } else {
            anyhow::bail!("Ping to {} failed", self.vm_ip)
        }
    }

    /// Check if SSH is accessible
    ///
    /// Returns Ok(()) if SSH connection succeeds, Err otherwise
    pub async fn check_ssh(&self, username: &str) -> Result<()> {
        debug!("Checking SSH connectivity to {}@{}", username, self.vm_ip);

        let output = tokio::time::timeout(
            self.check_timeout,
            tokio::process::Command::new("ssh")
                .args([
                    "-o",
                    "ConnectTimeout=3",
                    "-o",
                    "StrictHostKeyChecking=no",
                    "-o",
                    "UserKnownHostsFile=/dev/null",
                    "-o",
                    "BatchMode=yes",
                    &format!("{}@{}", username, self.vm_ip),
                    "true",
                ])
                .output(),
        )
        .await
        .context("SSH command timed out")?
        .context("Failed to execute SSH command")?;

        if output.status.success() {
            debug!("✅ SSH to {}@{} successful", username, self.vm_ip);
            Ok(())
        } else {
            anyhow::bail!("SSH to {}@{} failed", username, self.vm_ip)
        }
    }

    /// Comprehensive connectivity check (ping + SSH)
    pub async fn check_connectivity(&self, username: &str) -> Result<()> {
        // Try ping first (faster)
        self.check_ping().await.context("Ping check failed")?;

        // Then verify SSH works
        self.check_ssh(username).await.context("SSH check failed")?;

        Ok(())
    }

    /// Continuously monitor network with automatic recovery
    ///
    /// This method runs indefinitely, checking network connectivity at regular
    /// intervals. If connectivity fails, it attempts automatic recovery.
    ///
    /// # Arguments
    /// * `vm_handle` - Handle to the VM being monitored
    /// * `username` - Username for SSH connectivity checks
    ///
    /// # Returns
    /// Only returns on unrecoverable errors
    pub async fn monitor_with_recovery(
        &self,
        vm_handle: &VmHandle,
        username: &str,
    ) -> Result<()> {
        let mut consecutive_failures = 0;

        loop {
            match self.check_connectivity(username).await {
                Ok(_) => {
                    if consecutive_failures > 0 {
                        info!(
                            "✅ Network recovered after {} failures",
                            consecutive_failures
                        );
                    }
                    consecutive_failures = 0;
                    sleep(self.check_interval).await;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    warn!(
                        "⚠️  Network check failed (attempt {}/{}): {}",
                        consecutive_failures, self.max_failures, e
                    );

                    if consecutive_failures >= self.max_failures {
                        info!("🔧 Max failures reached, attempting recovery...");
                        match self.attempt_recovery(vm_handle, username).await {
                            Ok(_) => {
                                info!("✅ Network recovery succeeded");
                                consecutive_failures = 0;
                            }
                            Err(recovery_err) => {
                                warn!("❌ Network recovery failed: {}", recovery_err);
                                // Don't reset counter, will retry recovery on next iteration
                            }
                        }
                    }

                    // Wait before next check
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Attempt to recover network connectivity
    ///
    /// Tries multiple recovery strategies in sequence:
    /// 1. Restart systemd-networkd
    /// 2. Reapply network configuration with netplan
    /// 3. Bounce the network interface
    ///
    /// Returns Ok(()) if recovery succeeds, Err otherwise
    async fn attempt_recovery(&self, vm_handle: &VmHandle, username: &str) -> Result<()> {
        info!("🔧 Network recovery: Restarting systemd-networkd...");

        // Strategy 1: Restart systemd-networkd
        if vm_handle
            .ssh_exec(username, "sudo systemctl restart systemd-networkd")
            .await
            .is_ok()
        {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("✅ Recovery successful via systemd-networkd restart");
                return Ok(());
            }
        }

        info!("🔧 Network recovery: Applying netplan configuration...");

        // Strategy 2: Reapply netplan
        if vm_handle.ssh_exec(username, "sudo netplan apply").await.is_ok() {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("✅ Recovery successful via netplan apply");
                return Ok(());
            }
        }

        info!("🔧 Network recovery: Bouncing network interface...");

        // Strategy 3: Bounce the interface
        let bounce_cmd = "sudo ip link set enp1s0 down && sleep 2 && sudo ip link set enp1s0 up";
        if vm_handle.ssh_exec(username, bounce_cmd).await.is_ok() {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("✅ Recovery successful via interface bounce");
                return Ok(());
            }
        }

        anyhow::bail!("All recovery strategies failed")
    }

    /// Single verification attempt (non-continuous)
    ///
    /// Checks connectivity once with retries, but doesn't run indefinitely.
    /// Useful for one-time verification points during build.
    ///
    /// # Arguments
    /// * `username` - Username for SSH checks
    /// * `retries` - Number of retry attempts
    /// * `retry_delay` - Delay between retries
    pub async fn verify_once(
        &self,
        username: &str,
        retries: usize,
        retry_delay: Duration,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 1..=retries {
            match self.check_connectivity(username).await {
                Ok(_) => {
                    info!(
                        "✅ Network verification successful (attempt {})",
                        attempt
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "⚠️  Network verification failed (attempt {}/{}): {}",
                        attempt, retries, e
                    );
                    last_error = Some(e);

                    if attempt < retries {
                        sleep(retry_delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Network verification failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_monitor_creation() {
        let monitor = NetworkMonitor::new("192.168.122.10");
        assert_eq!(monitor.vm_ip, "192.168.122.10");
        assert_eq!(monitor.max_failures, 3);
    }

    #[test]
    fn test_network_monitor_configuration() {
        let monitor = NetworkMonitor::new("192.168.122.10")
            .with_check_interval(Duration::from_secs(5))
            .with_max_failures(5)
            .with_check_timeout(Duration::from_secs(10));

        assert_eq!(monitor.check_interval, Duration::from_secs(5));
        assert_eq!(monitor.max_failures, 5);
        assert_eq!(monitor.check_timeout, Duration::from_secs(10));
    }
}

