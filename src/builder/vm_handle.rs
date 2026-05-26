// SPDX-License-Identifier: AGPL-3.0-or-later
//! VM Handle for managing builder VMs
//!
//! Provides a high-level interface for interacting with VMs during the build process.

use anyhow::{Context, Result};
use benchscale::backend::{Backend, LibvirtBackend, NodeInfo};
use benchscale::SshClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Handle to a running VM
pub struct VmHandle {
    backend: LibvirtBackend,
    node: NodeInfo,
    /// Lazily initialized russh session (replaces ssh/scp CLI).
    ssh_session: Arc<Mutex<Option<SshClient>>>,
}

/// Cloud-init status from SSH query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInitStatusInfo {
    /// Raw status string from `cloud-init status` or JSON.
    pub status: String,
    /// Whether cloud-init reports still running.
    pub running: bool,
    /// Whether cloud-init reports finished successfully.
    pub finished: bool,
    /// Non-fatal or fatal messages from the status output.
    pub errors: Vec<String>,
}

impl VmHandle {
    /// Create a new VM handle
    pub fn new(backend: LibvirtBackend, node: NodeInfo) -> Self {
        Self {
            backend,
            node,
            ssh_session: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or create a persistent SSH session via russh.
    ///
    /// The session is cached in `self.ssh_session` so that repeated
    /// calls reuse the same connection instead of spawning a new
    /// `ssh` CLI process each time.
    async fn get_or_connect_ssh(&self, user: &str) -> Result<()> {
        let mut guard = self.ssh_session.lock().await;
        if guard.is_none() {
            debug!("Opening russh session to {}@{}", user, self.node.ip_address);
            let client = SshClient::connect_with_key(
                &self.node.ip_address,
                22,
                user,
                None,
            )
            .await
            .map_err(|e| anyhow::anyhow!("russh connect failed: {}", e))?;
            *guard = Some(client);
        }
        drop(guard);
        Ok(())
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
    pub const fn backend(&self) -> &LibvirtBackend {
        &self.backend
    }

    /// Get reference to node info
    pub const fn node(&self) -> &NodeInfo {
        &self.node
    }

    /// Execute a command via SSH using a persistent russh session.
    ///
    /// The first call lazily connects via key-based auth. Subsequent
    /// calls reuse the same session, avoiding the overhead of spawning
    /// a new `ssh` CLI process for every command.
    ///
    /// Falls back to the system `ssh` CLI if russh fails to connect
    /// (e.g., when the SSH key is passphrase-protected and no agent is
    /// available).
    pub async fn ssh_exec(&self, user: &str, cmd: &str) -> Result<String> {
        debug!("Executing SSH command on {}: {}", self.node.name, cmd);

        // Try russh first
        if let Err(e) = self.get_or_connect_ssh(user).await {
            warn!("russh session unavailable ({}), falling back to ssh CLI", e);
            return self.ssh_exec_cli(user, cmd).await;
        }

        let mut guard = self.ssh_session.lock().await;
        if let Some(ref mut client) = *guard {
            match client.exec_stdout(cmd).await {
                Ok(stdout) => return Ok(stdout),
                Err(e) => {
                    warn!("russh exec failed ({}), reconnecting...", e);
                    *guard = None;
                    drop(guard);
                    // One retry after reconnect
                    if self.get_or_connect_ssh(user).await.is_ok() {
                        let mut g2 = self.ssh_session.lock().await;
                        if let Some(ref mut c) = *g2 {
                            return c.exec_stdout(cmd).await
                                .map_err(|e| anyhow::anyhow!("russh exec failed after reconnect: {}", e));
                        }
                    }
                }
            }
        }

        self.ssh_exec_cli(user, cmd).await
    }

    /// Build common SSH CLI args with identity detection for sudo contexts.
    fn ssh_cli_args(user: &str, ip: &str) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(), "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(), "LogLevel=ERROR".to_string(),
            "-o".to_string(), "BatchMode=yes".to_string(),
        ];
        if let Some(key) = super::vm_create::detect_ssh_private_key() {
            args.push("-i".to_string());
            args.push(key.display().to_string());
        }
        args.push(format!("{user}@{ip}"));
        args
    }

    /// Fallback: execute via system `ssh` CLI.
    async fn ssh_exec_cli(&self, user: &str, cmd: &str) -> Result<String> {
        let mut args = Self::ssh_cli_args(user, &self.node.ip_address);
        args.push(cmd.to_string());

        let output = tokio::process::Command::new("ssh")
            .args(&args)
            .output()
            .await
            .context("Failed to execute SSH command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let filtered_error = Self::filter_ssh_warnings(&stderr);
            anyhow::bail!(
                "Command failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                if filtered_error.is_empty() { "Command returned non-zero exit code" } else { filtered_error.as_str() }
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Filter SSH warnings from error messages (Evolution #18)
    ///
    /// SSH client outputs informational warnings to stderr that don't indicate
    /// actual failures. This function filters out known benign warnings.
    ///
    /// Common warnings filtered:
    /// - "Warning: Permanently added..." (host key additions)
    /// - "Warning: Permanently added..." (ED25519/RSA key info)
    /// - Other SSH informational messages
    ///
    /// # Arguments
    /// * `stderr` - Raw stderr output from SSH command
    ///
    /// # Returns
    /// Filtered error message with only actual errors
    fn filter_ssh_warnings(stderr: &str) -> String {
        stderr
            .lines()
            .filter(|line| {
                let line_lower = line.to_lowercase();
                // Filter out common SSH informational warnings
                !line_lower.contains("warning: permanently added")
                    && !line_lower.contains("warning: permanent")
                    && !line_lower.starts_with("warning:")
                    // Keep actual error messages
                    && !line.trim().is_empty()
            })
            .collect::<Vec<_>>()
            .join("\n")
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

    /// Verify network connectivity (ping + SSH)
    ///
    /// Performs a comprehensive network check to ensure the VM is accessible.
    /// This is useful for verification points during the build process.
    ///
    /// # Arguments
    /// * `username` - Username for SSH connectivity check
    ///
    /// # Returns
    /// Ok(()) if both ping and SSH succeed, Err otherwise
    pub async fn verify_network(&self, username: &str) -> Result<()> {
        use crate::builder::NetworkMonitor;

        let monitor = NetworkMonitor::new(&self.node.ip_address);

        // Single verification attempt with retries
        monitor
            .verify_once(username, 3, Duration::from_secs(5))
            .await
            .context("Network verification failed")
    }

    /// Fetch a file from the guest to the host via russh.
    ///
    /// For single files, reads the remote file through the SSH channel
    /// (base64-encoded) and writes it locally. Falls back to the `scp`
    /// CLI for recursive directory copies.
    pub async fn scp_fetch(
        &self,
        user: &str,
        remote_path: &str,
        local_path: &std::path::Path,
        recursive: bool,
    ) -> Result<()> {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create local directory {}", parent.display()))?;
        }

        if recursive {
            return self.scp_fetch_cli(user, remote_path, local_path, true).await;
        }

        // Try russh channel transfer
        if self.get_or_connect_ssh(user).await.is_ok() {
            let mut guard = self.ssh_session.lock().await;
            if let Some(ref mut client) = *guard {
                match client.fetch_data(remote_path).await {
                    Ok(data) => {
                        std::fs::write(local_path, &data)
                            .with_context(|| format!("failed to write {}", local_path.display()))?;
                        info!(remote = remote_path, local = %local_path.display(), "fetched via russh");
                        return Ok(());
                    }
                    Err(e) => {
                        warn!("russh fetch failed ({}), falling back to scp CLI", e);
                    }
                }
            }
        }

        self.scp_fetch_cli(user, remote_path, local_path, false).await
    }

    /// Fallback: fetch via system `scp` CLI.
    async fn scp_fetch_cli(
        &self, user: &str, remote_path: &str,
        local_path: &std::path::Path, recursive: bool,
    ) -> Result<()> {
        let remote_spec = format!("{}@{}:{}", user, self.node.ip_address, remote_path);
        let mut cmd = tokio::process::Command::new("scp");
        cmd.arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("UserKnownHostsFile=/dev/null")
            .arg("-o").arg("LogLevel=ERROR")
            .arg("-o").arg("BatchMode=yes");
        if let Some(key) = super::vm_create::detect_ssh_private_key() {
            cmd.arg("-i").arg(key);
        }
        if recursive { cmd.arg("-r"); }
        cmd.arg(&remote_spec).arg(local_path.as_os_str());
        let output = cmd.output().await.context("failed to execute scp")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("scp failed (exit {}): {}", output.status.code().unwrap_or(-1), stderr);
        }
        info!(remote = remote_path, local = %local_path.display(), "fetched via scp CLI");
        Ok(())
    }

    /// Push a file from the host to the guest via russh.
    ///
    /// Reads the local file, transfers it through the SSH channel
    /// (base64-encoded), and writes it on the guest. Falls back to
    /// the `scp` CLI on failure.
    pub async fn scp_push(
        &self,
        user: &str,
        local_path: &std::path::Path,
        remote_path: &str,
    ) -> Result<()> {
        let data = std::fs::read(local_path)
            .with_context(|| format!("failed to read {}", local_path.display()))?;

        if self.get_or_connect_ssh(user).await.is_ok() {
            let mut guard = self.ssh_session.lock().await;
            if let Some(ref mut client) = *guard {
                match client.push_data(&data, remote_path).await {
                    Ok(()) => {
                        info!(local = %local_path.display(), remote = remote_path, "pushed via russh");
                        return Ok(());
                    }
                    Err(e) => {
                        warn!("russh push failed ({}), falling back to scp CLI", e);
                    }
                }
            }
        }

        self.scp_push_cli(local_path, user, remote_path).await
    }

    /// Fallback: push via system `scp` CLI.
    async fn scp_push_cli(
        &self, local_path: &std::path::Path, user: &str, remote_path: &str,
    ) -> Result<()> {
        let remote_spec = format!("{}@{}:{}", user, self.node.ip_address, remote_path);
        let mut cmd = tokio::process::Command::new("scp");
        cmd.arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("UserKnownHostsFile=/dev/null")
            .arg("-o").arg("LogLevel=ERROR")
            .arg("-o").arg("BatchMode=yes");
        if let Some(key) = super::vm_create::detect_ssh_private_key() {
            cmd.arg("-i").arg(key);
        }
        cmd.arg(local_path.as_os_str()).arg(&remote_spec);
        let output = cmd.output().await.context("failed to execute scp")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("scp push failed (exit {}): {}", output.status.code().unwrap_or(-1), stderr);
        }
        info!(local = %local_path.display(), remote = remote_path, "pushed via scp CLI");
        Ok(())
    }

    /// Delete the VM
    pub async fn delete(self) -> Result<()> {
        info!("Deleting VM: {}", self.node.name);
        self.backend.delete_node(&self.node.id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::VmHandle;

    #[test]
    fn filter_ssh_warnings_strips_benign_lines() {
        let raw = "Warning: Permanently added '10.0.0.1' (ED25519) to the list of known hosts.\n\
             actual error: permission denied\n";
        let filtered = VmHandle::filter_ssh_warnings(raw);
        assert!(filtered.contains("permission denied"));
        assert!(!filtered.to_lowercase().contains("permanently added"));
    }

    #[test]
    fn filter_ssh_warnings_empty_when_only_warnings() {
        let raw = "Warning: Permanently added host key\n";
        let filtered = VmHandle::filter_ssh_warnings(raw);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_ssh_warnings_drops_permanent_variant_and_generic_warning_prefix() {
        let raw = "Warning: permanent host key for 'x' added\n\
             real problem: connection refused\n\
             warning: something else\n";
        let filtered = VmHandle::filter_ssh_warnings(raw);
        assert!(filtered.contains("connection refused"));
        assert!(!filtered.to_lowercase().contains("permanent host"));
        assert!(!filtered.contains("something else"));
    }

    #[test]
    fn filter_ssh_warnings_preserves_non_warning_stderr() {
        let raw = "Permission denied (publickey).\n";
        assert_eq!(VmHandle::filter_ssh_warnings(raw), raw.trim_end());
    }
}
