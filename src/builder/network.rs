// SPDX-License-Identifier: AGPL-3.0-only
//! Network monitoring and resilience for VM builds
//!
//! Provides continuous network verification and automatic recovery mechanisms
//! to ensure reliable VM provisioning even when network issues occur.

use anyhow::{Context, Result};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::TcpStream;
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
    /// Parsed IP for socket construction
    addr: IpAddr,
    /// SSH port to probe
    ssh_port: u16,
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
    pub fn new(vm_ip: impl Into<String>) -> Self {
        let ip_str = vm_ip.into();
        let addr = ip_str.parse::<IpAddr>().unwrap_or_else(|_| {
            // Fallback: treat as hostname, use unspecified for now —
            // TCP connect will use the original string via ToSocketAddrs.
            IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
        });
        Self {
            vm_ip: ip_str,
            addr,
            ssh_port: 22,
            check_interval: Duration::from_secs(10),
            max_failures: 3,
            check_timeout: Duration::from_secs(5),
        }
    }

    /// Set the check interval
    pub const fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Set the maximum consecutive failures
    pub const fn with_max_failures(mut self, max: usize) -> Self {
        self.max_failures = max;
        self
    }

    /// Set the check timeout
    pub const fn with_check_timeout(mut self, timeout: Duration) -> Self {
        self.check_timeout = timeout;
        self
    }

    /// Check if network is accessible via TCP connect to SSH port.
    ///
    /// A successful TCP handshake to port 22 proves both IP reachability
    /// and that the SSH daemon is listening — strictly more useful than ICMP ping.
    pub async fn check_ping(&self) -> Result<()> {
        debug!("Checking network connectivity to {} via TCP:22", self.vm_ip);

        let target = SocketAddr::new(self.addr, self.ssh_port);
        let connect_fut = TcpStream::connect(target);
        let _stream: TcpStream = tokio::time::timeout(self.check_timeout, connect_fut)
            .await
            .context("TCP connect timed out")?
            .with_context(|| format!("TCP connect to {} failed", target))?;

        debug!("TCP probe to {} successful", self.vm_ip);
        Ok(())
    }

    /// Check if SSH is accessible by verifying the SSH banner.
    ///
    /// Connects to the SSH port and reads the first bytes — a real SSH
    /// daemon will send a version banner starting with `SSH-`.
    pub async fn check_ssh(&self, _username: &str) -> Result<()> {
        debug!("Checking SSH banner on {}", self.vm_ip);

        let target = SocketAddr::new(self.addr, self.ssh_port);
        let connect_fut = TcpStream::connect(target);
        let stream: TcpStream = tokio::time::timeout(self.check_timeout, connect_fut)
            .await
            .context("SSH connect timed out")?
            .with_context(|| format!("SSH connect to {} failed", target))?;

        // Wait for the server to send its banner
        stream.readable().await.context("waiting for SSH banner")?;
        let mut buf = [0u8; 32];
        match stream.try_read(&mut buf) {
            Ok(n) if n >= 4 && buf.starts_with(b"SSH-") => {
                debug!("SSH banner from {} verified", self.vm_ip);
                Ok(())
            }
            Ok(n) => {
                anyhow::bail!(
                    "unexpected banner from {} ({} bytes, starts with {:?})",
                    self.vm_ip,
                    n,
                    &buf[..n.min(8)]
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                anyhow::bail!("SSH banner not ready on {}", self.vm_ip);
            }
            Err(e) => {
                anyhow::bail!("reading SSH banner from {}: {}", self.vm_ip, e);
            }
        }
    }

    /// Comprehensive connectivity check (TCP + SSH banner)
    pub async fn check_connectivity(&self, username: &str) -> Result<()> {
        self.check_ping().await.context("TCP probe failed")?;
        self.check_ssh(username).await.context("SSH check failed")?;
        Ok(())
    }

    /// Continuously monitor network with automatic recovery
    ///
    /// This method runs indefinitely, checking network connectivity at regular
    /// intervals. If connectivity fails, it attempts automatic recovery.
    pub async fn monitor_with_recovery(
        &self,
        vm_handle: &VmHandle,
        username: &str,
    ) -> Result<()> {
        let mut consecutive_failures = 0;

        loop {
            match self.check_connectivity(username).await {
                Ok(()) => {
                    if consecutive_failures > 0 {
                        info!(
                            "Network recovered after {} failures",
                            consecutive_failures
                        );
                    }
                    consecutive_failures = 0;
                    sleep(self.check_interval).await;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    warn!(
                        "Network check failed (attempt {}/{}): {}",
                        consecutive_failures, self.max_failures, e
                    );

                    if consecutive_failures >= self.max_failures {
                        info!("Max failures reached, attempting recovery...");
                        match self.attempt_recovery(vm_handle, username).await {
                            Ok(()) => {
                                info!("Network recovery succeeded");
                                consecutive_failures = 0;
                            }
                            Err(recovery_err) => {
                                warn!("Network recovery failed: {}", recovery_err);
                            }
                        }
                    }

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
    async fn attempt_recovery(&self, vm_handle: &VmHandle, username: &str) -> Result<()> {
        info!("Network recovery: Restarting systemd-networkd...");

        if vm_handle
            .ssh_exec(username, "sudo systemctl restart systemd-networkd")
            .await
            .is_ok()
        {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("Recovery successful via systemd-networkd restart");
                return Ok(());
            }
        }

        info!("Network recovery: Applying netplan configuration...");

        if vm_handle.ssh_exec(username, "sudo netplan apply").await.is_ok() {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("Recovery successful via netplan apply");
                return Ok(());
            }
        }

        info!("Network recovery: Bouncing network interface...");

        let bounce_cmd = concat!(
            "iface=$(ip -o -4 route show default 2>/dev/null | awk '{print $5}' | head -1); ",
            "[ -n \"$iface\" ] && sudo ip link set \"$iface\" down && sleep 2 && sudo ip link set \"$iface\" up"
        );
        if vm_handle.ssh_exec(username, bounce_cmd).await.is_ok() {
            sleep(Duration::from_secs(5)).await;
            if self.check_connectivity(username).await.is_ok() {
                info!("Recovery successful via interface bounce");
                return Ok(());
            }
        }

        anyhow::bail!("All recovery strategies failed")
    }

    /// Single verification attempt (non-continuous)
    ///
    /// Checks connectivity once with retries, but doesn't run indefinitely.
    /// Useful for one-time verification points during build.
    pub async fn verify_once(
        &self,
        username: &str,
        retries: usize,
        retry_delay: Duration,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 1..=retries {
            match self.check_connectivity(username).await {
                Ok(()) => {
                    info!(
                        "Network verification successful (attempt {})",
                        attempt
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "Network verification failed (attempt {}/{}): {}",
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
        assert_eq!(monitor.addr, "192.168.122.10".parse::<IpAddr>().unwrap());
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
