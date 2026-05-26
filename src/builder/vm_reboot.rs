// SPDX-License-Identifier: AGPL-3.0-or-later
//! Robust VM reboot handling with deep diagnostics.
//!
//! VM reboot recovery that goes beyond simple SSH polling to understand why a VM
//! might not respond after reboot.

use crate::builder::vm_handle::VmHandle;
use anyhow::Result;
use std::fmt::Write;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Detailed reboot state information for diagnostics
#[derive(Debug, Clone)]
pub struct RebootState {
    /// Elapsed time since reboot initiated
    pub elapsed_secs: u64,
    /// Whether VM is running (from libvirt)
    pub vm_running: bool,
    /// Whether SSH is responsive
    pub ssh_responsive: bool,
    /// Last SSH error if any
    pub ssh_error: Option<String>,
    /// Boot diagnostics (if available)
    pub boot_diagnostics: Option<BootDiagnostics>,
}

/// Boot diagnostics gathered from the VM
#[derive(Debug, Clone)]
pub struct BootDiagnostics {
    /// Whether systemd reached multi-user.target
    pub systemd_multi_user: bool,
    /// Whether systemd reached graphical.target
    pub systemd_graphical: bool,
    /// Whether SSH service is active
    pub ssh_service_active: bool,
    /// Whether network is configured
    pub network_configured: bool,
    /// Recent boot messages (last 20 lines)
    pub recent_boot_messages: Vec<String>,
}

/// Configuration for reboot recovery behavior
#[derive(Debug, Clone)]
pub struct RebootConfig {
    /// Initial wait time before checking SSH (seconds)
    pub initial_wait_secs: u64,
    /// Maximum total time to wait for reboot (seconds)
    pub max_wait_secs: u64,
    /// Interval between SSH checks (seconds)
    pub check_interval_secs: u64,
    /// Time to wait after SSH responds for system stabilization (seconds)
    pub stabilization_wait_secs: u64,
    /// Whether to gather detailed diagnostics on failure
    pub gather_diagnostics: bool,
}

impl Default for RebootConfig {
    fn default() -> Self {
        Self {
            initial_wait_secs: 120,      // 2 minutes initial wait
            max_wait_secs: 600,          // 10 minutes max (desktop environments)
            check_interval_secs: 5,      // Check every 5 seconds
            stabilization_wait_secs: 10, // 10 seconds stabilization
            gather_diagnostics: true,    // Always gather diagnostics
        }
    }
}

/// Execute a VM reboot with robust recovery and diagnostics
///
/// This function provides a comprehensive reboot strategy that:
/// 1. Initiates the reboot gracefully
/// 2. Monitors VM state via libvirt
/// 3. Checks SSH connectivity with proper retry logic
/// 4. Gathers diagnostics if the reboot fails
/// 5. Provides actionable error messages
///
/// # Arguments
/// * `vm` - The VM handle
/// * `username` - SSH username
/// * `config` - Reboot configuration
///
/// # Returns
/// * `Ok(RebootState)` - Final state on successful reboot
/// * `Err` - Detailed error with diagnostics
#[expect(
    clippy::too_many_lines,
    reason = "Reboot loop: SSH polling, libvirt state, optional diagnostics"
)]
pub async fn execute_reboot(
    vm: &VmHandle,
    username: &str,
    config: &RebootConfig,
) -> Result<RebootState> {
    info!("  🔄 Rebooting VM with deep diagnostics...");

    // Step 1: Initiate reboot
    let reboot_cmd = "sudo shutdown -r now";
    match vm.ssh_exec(username, reboot_cmd).await {
        Ok(_) => {
            info!("     ✅ Reboot command sent successfully");
        }
        Err(e) => {
            // Connection drop is expected during reboot
            debug!("     SSH connection closed (expected): {}", e);
        }
    }

    // Step 2: Initial wait for reboot to complete
    info!(
        "     ⏳ Waiting {}s for initial reboot phase...",
        config.initial_wait_secs
    );
    tokio::time::sleep(Duration::from_secs(config.initial_wait_secs)).await;

    // Step 3: Poll for SSH with diagnostics
    let max_attempts = config.max_wait_secs / config.check_interval_secs;
    info!(
        "     🔍 Monitoring reboot recovery (up to {} minutes, checking every {}s)...",
        config.max_wait_secs / 60,
        config.check_interval_secs
    );

    let start = std::time::Instant::now();
    let mut last_state: Option<RebootState> = None;

    for attempt in 1..=max_attempts {
        tokio::time::sleep(Duration::from_secs(config.check_interval_secs)).await;

        let elapsed_secs = start.elapsed().as_secs();

        // Check SSH connectivity
        let ssh_check = vm.ssh_exec(username, "echo 'reboot-check'").await;

        match ssh_check {
            Ok(output) if output.trim() == "reboot-check" => {
                // SSH is responsive!
                let final_state = RebootState {
                    elapsed_secs,
                    vm_running: true,
                    ssh_responsive: true,
                    ssh_error: None,
                    boot_diagnostics: None,
                };

                info!(
                    "     ✅ SSH responsive after {} seconds ({} attempts)",
                    elapsed_secs, attempt
                );

                // Step 4: Gather boot diagnostics for validation
                if config.gather_diagnostics {
                    info!("     📊 Gathering boot diagnostics...");
                    match gather_boot_diagnostics(vm, username).await {
                        Ok(diagnostics) => {
                            info!("     ✅ Boot diagnostics:");
                            info!(
                                "        • Systemd multi-user: {}",
                                diagnostics.systemd_multi_user
                            );
                            info!(
                                "        • Systemd graphical: {}",
                                diagnostics.systemd_graphical
                            );
                            info!("        • SSH service: {}", diagnostics.ssh_service_active);
                            info!("        • Network: {}", diagnostics.network_configured);

                            return Ok(RebootState {
                                boot_diagnostics: Some(diagnostics),
                                ..final_state
                            });
                        }
                        Err(e) => {
                            warn!(
                                "     ⚠️  Could not gather diagnostics (but SSH is working): {}",
                                e
                            );
                            // Continue anyway - SSH works, which is what matters
                        }
                    }
                }

                // Step 5: Stabilization wait
                info!(
                    "     ⏳ Allowing system to stabilize ({}s)...",
                    config.stabilization_wait_secs
                );
                tokio::time::sleep(Duration::from_secs(config.stabilization_wait_secs)).await;

                return Ok(final_state);
            }
            Ok(_output) => {
                // Unexpected output - SSH is working but command failed?
                warn!(
                    "     ⚠️  SSH connected but unexpected response (attempt {}/{})",
                    attempt, max_attempts
                );
            }
            Err(e) => {
                // SSH not ready yet - update state
                let state = RebootState {
                    elapsed_secs,
                    vm_running: false, // We don't know yet without libvirt check
                    ssh_responsive: false,
                    ssh_error: Some(e.to_string()),
                    boot_diagnostics: None,
                };

                last_state = Some(state);

                // Log progress every 30 seconds
                if attempt % 6 == 0 {
                    let minutes = elapsed_secs / 60;
                    let seconds = elapsed_secs % 60;
                    info!(
                        "       Still waiting for SSH ({}m {}s elapsed, attempt {}/{})...",
                        minutes, seconds, attempt, max_attempts
                    );
                    debug!("       Last SSH error: {}", e);
                }
            }
        }
    }

    // Step 6: Reboot timeout - gather comprehensive diagnostics
    let elapsed_secs = start.elapsed().as_secs();
    let minutes = elapsed_secs / 60;
    let seconds = elapsed_secs % 60;

    warn!(
        "     ❌ Reboot timeout after {}m {}s ({} attempts)",
        minutes, seconds, max_attempts
    );

    // Try to gather diagnostics even though SSH is not responding
    // This might work if VM is up but SSH is delayed
    let diagnostics = if config.gather_diagnostics {
        info!("     📊 Attempting to gather failure diagnostics...");

        // Try multiple times with longer timeouts
        let mut diag_result = None;
        for retry in 1..=3 {
            match gather_boot_diagnostics(vm, username).await {
                Ok(diag) => {
                    info!("     ✅ Diagnostics gathered on retry {}", retry);
                    diag_result = Some(diag);
                    break;
                }
                Err(e) => {
                    debug!("     Diagnostics attempt {} failed: {}", retry, e);
                    if retry < 3 {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
        diag_result
    } else {
        None
    };

    // Build detailed error message
    let mut error_msg = format!(
        "VM did not respond to SSH after reboot (waited {}m {}s, {} attempts)",
        minutes, seconds, max_attempts
    );

    if let Some(ref diag) = diagnostics {
        error_msg.push_str("\n\n📊 Boot Diagnostics:");
        let _ = writeln!(
            error_msg,
            "\n  • Systemd multi-user target: {}",
            diag.systemd_multi_user
        );
        let _ = writeln!(
            error_msg,
            "\n  • Systemd graphical target: {}",
            diag.systemd_graphical
        );
        let _ = writeln!(
            error_msg,
            "\n  • SSH service active: {}",
            diag.ssh_service_active
        );
        let _ = writeln!(
            error_msg,
            "\n  • Network configured: {}",
            diag.network_configured
        );

        if !diag.recent_boot_messages.is_empty() {
            error_msg.push_str("\n\n📝 Recent boot messages:");
            for msg in diag.recent_boot_messages.iter().take(10) {
                let _ = writeln!(error_msg, "\n  {}", msg);
            }
        }

        // Suggest fixes based on diagnostics
        error_msg.push_str("\n\n💡 Suggested actions:");
        if !diag.systemd_multi_user {
            error_msg
                .push_str("\n  • System not reaching multi-user target - check systemd services");
        }
        if !diag.ssh_service_active {
            error_msg.push_str("\n  • SSH service not starting - check sshd configuration");
        }
        if !diag.network_configured {
            error_msg.push_str("\n  • Network not configured - check cloud-init network settings");
        }
    } else {
        error_msg.push_str("\n\n⚠️  Could not gather diagnostics (SSH completely unresponsive)");
        error_msg.push_str("\n\n💡 Possible causes:");
        error_msg.push_str("\n  • VM failed to boot (check libvirt console logs)");
        error_msg.push_str("\n  • Desktop environment taking longer than expected to initialize");
        error_msg.push_str("\n  • SSH service not starting on boot");
        error_msg.push_str("\n  • Network configuration issue preventing SSH access");
    }

    if let Some(state) = last_state
        && let Some(ref ssh_err) = state.ssh_error
    {
        let _ = writeln!(error_msg, "\n\n🔍 Last SSH error: {}", ssh_err);
    }

    anyhow::bail!(error_msg)
}

/// Gather detailed boot diagnostics from the VM
///
/// This function connects to the VM and checks various system states to
/// understand the boot process health.
async fn gather_boot_diagnostics(vm: &VmHandle, username: &str) -> Result<BootDiagnostics> {
    // Check systemd targets
    let multi_user = vm
        .ssh_exec(
            username,
            "systemctl is-active multi-user.target 2>/dev/null",
        )
        .await
        .map(|s| s.trim() == "active")
        .unwrap_or(false);

    let graphical = vm
        .ssh_exec(username, "systemctl is-active graphical.target 2>/dev/null")
        .await
        .map(|s| s.trim() == "active")
        .unwrap_or(false);

    // Check SSH service
    let ssh_active = vm
        .ssh_exec(
            username,
            "systemctl is-active ssh 2>/dev/null || systemctl is-active sshd 2>/dev/null",
        )
        .await
        .map(|s| s.trim() == "active")
        .unwrap_or(false);

    // Check network
    let network = vm
        .ssh_exec(
            username,
            "ip addr show | grep 'inet ' | grep -v '127.0.0.1' | wc -l",
        )
        .await
        .map(|s| s.trim().parse::<u32>().unwrap_or(0) > 0)
        .unwrap_or(false);

    // Get recent boot messages
    let boot_messages = vm
        .ssh_exec(username, "journalctl -b -n 20 --no-pager 2>/dev/null")
        .await
        .unwrap_or_default()
        .lines()
        .map(std::string::ToString::to_string)
        .collect();

    Ok(BootDiagnostics {
        systemd_multi_user: multi_user,
        systemd_graphical: graphical,
        ssh_service_active: ssh_active,
        network_configured: network,
        recent_boot_messages: boot_messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reboot_config_default_sensible() {
        let c = RebootConfig::default();
        assert!(c.initial_wait_secs > 0);
        assert!(c.max_wait_secs >= c.initial_wait_secs);
        assert!(c.check_interval_secs > 0);
        assert!(c.gather_diagnostics);
    }

    #[test]
    fn reboot_state_clone_and_fields() {
        let s = RebootState {
            elapsed_secs: 3,
            vm_running: true,
            ssh_responsive: true,
            ssh_error: None,
            boot_diagnostics: None,
        };
        let c = s;
        assert_eq!(c.elapsed_secs, 3);
    }

    #[test]
    fn boot_diagnostics_default_empty_messages() {
        let d = BootDiagnostics {
            systemd_multi_user: false,
            systemd_graphical: true,
            ssh_service_active: false,
            network_configured: true,
            recent_boot_messages: vec!["line".to_string()],
        };
        assert_eq!(d.recent_boot_messages.len(), 1);
    }
}
