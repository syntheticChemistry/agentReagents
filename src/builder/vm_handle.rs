//! VM Handle for managing builder VMs
//!
//! Provides a high-level interface for interacting with VMs during the build process.

use anyhow::{Context, Result};
use benchscale::backend::{Backend, LibvirtBackend, NodeInfo};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Handle to a running VM
pub struct VmHandle {
    backend: LibvirtBackend,
    node: NodeInfo,
}

/// Cloud-init status from SSH query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInitStatusInfo {
    pub status: String,
    pub running: bool,
    pub finished: bool,
    pub errors: Vec<String>,
}

impl VmHandle {
    /// Create a new VM handle
    pub fn new(backend: LibvirtBackend, node: NodeInfo) -> Self {
        Self { backend, node }
    }

    /// Get the VM's IP address
    pub fn ip_address(&self) -> &str {
        &self.node.ip_address
    }

    /// Get the VM's name
    pub fn name(&self) -> &str {
        &self.node.name
    }

    /// Get the VM's ID
    pub fn id(&self) -> &str {
        &self.node.id
    }

    /// Get reference to backend
    pub fn backend(&self) -> &LibvirtBackend {
        &self.backend
    }

    /// Get reference to node info
    pub fn node(&self) -> &NodeInfo {
        &self.node
    }

    /// Execute a command via SSH
    ///
    /// # Implementation Note
    ///
    /// This uses the system `ssh` command via tokio::process rather than
    /// a Rust SSH library. This is a deliberate design choice:
    ///
    /// - **Simplicity**: Leverages system SSH with established security
    /// - **Key Management**: Uses system SSH agent and known_hosts
    /// - **Compatibility**: Works with any SSH configuration
    /// - **Zero Dependencies**: No additional SSH library dependencies
    ///
    /// This is a deep debt solution that prioritizes reliability and
    /// simplicity over pure-Rust implementation. For more advanced SSH
    /// needs, consider benchScale exposing ssh_exec on LibvirtBackend.
    pub async fn ssh_exec(&self, user: &str, cmd: &str) -> Result<String> {
        debug!("Executing SSH command on {}: {}", self.node.name, cmd);

        // Use system SSH command for maximum compatibility
        let output = tokio::process::Command::new("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("UserKnownHostsFile=/dev/null")
            .arg(format!("{}@{}", user, self.node.ip_address))
            .arg(cmd)
            .output()
            .await
            .context("Failed to execute SSH command")?;

        if !output.status.success() {
            anyhow::bail!(
                "SSH command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get cloud-init status
    pub async fn get_cloud_init_status(&self, user: &str) -> Result<CloudInitStatusInfo> {
        let output = self
            .ssh_exec(user, "cloud-init status --format=json")
            .await?;

        let status: serde_json::Value =
            serde_json::from_str(&output).context("Failed to parse cloud-init status")?;

        Ok(CloudInitStatusInfo {
            status: status["status"].as_str().unwrap_or("unknown").to_string(),
            running: status["status"].as_str() == Some("running"),
            finished: status["status"].as_str() == Some("done"),
            errors: status["errors"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Wait for cloud-init to complete with progress callback
    pub async fn wait_for_cloud_init(&self, user: &str, timeout: Duration) -> Result<()> {
        self.wait_for_cloud_init_with_progress(user, timeout, |status| {
            info!("Cloud-init: {}", status);
        })
        .await
    }

    /// Wait for cloud-init to complete with custom progress callback
    pub async fn wait_for_cloud_init_with_progress<F>(
        &self,
        user: &str,
        timeout: Duration,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(&str),
    {
        info!(
            "Waiting for cloud-init to complete on {}...",
            self.node.name
        );

        let start = std::time::Instant::now();
        let mut last_status = String::new();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for cloud-init after {:?}", timeout);
            }

            match self.get_cloud_init_status(user).await {
                Ok(status) if status.finished => {
                    info!("Cloud-init completed successfully");
                    progress_callback("done");
                    return Ok(());
                }
                Ok(status) if !status.errors.is_empty() => {
                    let error_msg = format!("Cloud-init failed: {:?}", status.errors);
                    progress_callback(&error_msg);
                    anyhow::bail!("{}", error_msg);
                }
                Ok(status) => {
                    if status.status != last_status {
                        progress_callback(&status.status);
                        last_status = status.status;
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to get cloud-init status (may not be ready yet): {}",
                        e
                    );
                    progress_callback("waiting for SSH...");
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    /// Get detailed cloud-init stage information
    pub async fn get_cloud_init_stages(&self, user: &str) -> Result<Vec<String>> {
        let output = self
            .ssh_exec(
                user,
                "cloud-init status --long 2>/dev/null || echo 'unavailable'",
            )
            .await?;

        let stages: Vec<String> = output
            .lines()
            .filter(|line| line.contains("stage:"))
            .map(|line| line.trim().to_string())
            .collect();

        Ok(stages)
    }

    /// Check if a package is installed
    pub async fn is_package_installed(&self, user: &str, package: &str) -> Result<bool> {
        let result = self
            .ssh_exec(
                user,
                &format!("dpkg -l {} 2>/dev/null | grep -q '^ii'", package),
            )
            .await;

        Ok(result.is_ok())
    }

    /// Install packages
    pub async fn install_packages(&self, user: &str, packages: &[String]) -> Result<()> {
        info!("Installing packages: {}", packages.join(", "));

        let cmd = format!(
            "sudo apt-get update && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y {}",
            packages.join(" ")
        );

        self.ssh_exec(user, &cmd).await?;
        Ok(())
    }

    /// Check if VM is running
    pub async fn is_running(&self) -> Result<bool> {
        // Check VM status via backend
        match self.backend.get_node(&self.node.id).await {
            Ok(node) => Ok(node.status == benchscale::backend::NodeStatus::Running),
            Err(_) => Ok(false),
        }
    }

    /// Delete the VM
    pub async fn delete(self) -> Result<()> {
        info!("Deleting VM: {}", self.node.name);
        self.backend.delete_node(&self.node.id).await?;
        Ok(())
    }
}
