// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post-Boot Step Executor
//!
//! Executes steps via SSH after cloud-init completes.
//! This is the "add heat-sensitive compounds after cooling" phase.

use crate::builder::vm_handle::VmHandle;
use crate::builder::vm_reboot::{RebootConfig, execute_reboot};
use crate::templates::PostBootStep;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};

const DEFAULT_SSH_CMD_TIMEOUT_SECS: u64 = 120;

/// Execute all post-boot steps on a VM.
#[tracing::instrument(skip(vm, steps))]
pub async fn execute_post_boot_steps(
    vm: &VmHandle,
    steps: &[PostBootStep],
    username: &str,
    pm: crate::templates::PackageManager,
) -> Result<()> {
    if steps.is_empty() {
        info!("No post-boot steps to execute");
        return Ok(());
    }

    info!(
        "Executing {} post-boot steps (package manager: {:?})",
        steps.len(), pm
    );

    for (idx, step) in steps.iter().enumerate() {
        info!("  Step {}/{}: {:?}", idx + 1, steps.len(), step);
        execute_post_boot_step(vm, step, username, pm)
            .await
            .with_context(|| {
                format!(
                    "Failed to execute post-boot step {}/{}",
                    idx + 1,
                    steps.len()
                )
            })?;
    }

    info!("All post-boot steps completed successfully");
    Ok(())
}

/// Execute a single post-boot step
async fn execute_post_boot_step(
    vm: &VmHandle,
    step: &PostBootStep,
    username: &str,
    pm: crate::templates::PackageManager,
) -> Result<()> {
    match step {
        PostBootStep::InstallPackages {
            packages,
            retry,
            timeout_secs,
            description,
        } => {
            if let Some(desc) = description {
                info!("  {}", desc);
            }
            info!(
                "     Installing {} packages via {:?} (timeout: {}s)...",
                packages.len(),
                pm,
                timeout_secs
            );
            info!(count = packages.len(), "Installing packages: {}", packages.join(", "));

            let result = if *retry {
                let command = format!(
                    "sudo {} {}",
                    pm.install_command(),
                    packages.join(" ")
                );
                execute_with_retry(vm, &command, *timeout_secs, 3, username).await
            } else {
                execute_package_install_monitored(vm, packages, *timeout_secs, username, pm).await
            };

            result.with_context(|| format!("Failed to install packages: {:?}", packages))?;
            info!("Packages installed");
        }

        PostBootStep::RunCommand {
            command,
            description,
            timeout_secs,
        } => {
            if let Some(desc) = description {
                info!("  🔧 {}", desc);
            }
            execute_with_timeout(vm, command, *timeout_secs, username)
                .await
                .with_context(|| format!("Failed to run command: {}", command))?;
            info!("     ✅ Command executed");
        }

        PostBootStep::CreateFile {
            path,
            content,
            mode,
            owner,
        } => {
            info!("  📝 Creating file: {}", path);

            // Use heredoc for clean multi-line content
            let command = format!(
                "sudo tee {} > /dev/null <<'EOFAGENTREAGENTS'\n{}\nEOFAGENTREAGENTS",
                path, content
            );
            execute_with_timeout(vm, &command, 60, username).await?;

            // Set permissions
            let chmod_cmd = format!("sudo chmod {} {}", mode, path);
            execute_with_timeout(vm, &chmod_cmd, 10, username).await?;

            // Set owner if specified
            if let Some(owner_name) = owner {
                let chown_cmd = format!("sudo chown {} {}", owner_name, path);
                execute_with_timeout(vm, &chown_cmd, 10, username).await?;
            }

            info!("     ✅ File created");
        }

        PostBootStep::CopyFile {
            source,
            destination,
            mode,
        } => {
            info!("  Copying file: {} -> {}", source, destination);
            vm.scp_push(username, std::path::Path::new(source), destination)
                .await
                .with_context(|| format!("Failed to copy {} to guest:{}", source, destination))?;

            let chmod_cmd = format!("sudo chmod {} {}", mode, destination);
            execute_with_timeout(vm, &chmod_cmd, 10, username).await?;

            info!("     File copied with mode {}", mode);
        }

        PostBootStep::FetchFile {
            remote_path,
            local_path,
            recursive,
        } => {
            info!("  Fetching artifact: guest:{} -> host:{}", remote_path, local_path);
            vm.scp_fetch(username, remote_path, std::path::Path::new(local_path), *recursive)
                .await
                .with_context(|| format!("Failed to fetch {} from guest", remote_path))?;
            info!("     Artifact fetched");
        }

        PostBootStep::EnableService { service, start } => {
            info!("  ⚙️  Enabling service: {}", service);

            let enable_cmd = format!("sudo systemctl enable {}", service);
            execute_with_timeout(vm, &enable_cmd, 30, username).await?;

            if *start {
                let start_cmd = format!("sudo systemctl start {}", service);
                execute_with_timeout(vm, &start_cmd, 60, username).await?;
                info!("     ✅ Service enabled and started");
            } else {
                info!("     ✅ Service enabled");
            }
        }

        PostBootStep::Reboot { wait_secs } => {
            // EVOLUTION #9: Deep reboot diagnostics with idiomatic Rust
            // Use the new vm_reboot module for comprehensive reboot handling
            let config = RebootConfig {
                initial_wait_secs: *wait_secs,
                max_wait_secs: 600, // 10 minutes for desktop environments
                check_interval_secs: 5,
                stabilization_wait_secs: 10,
                gather_diagnostics: true,
            };

            let _state = execute_reboot(vm, username, &config)
                .await
                .context("Failed to complete VM reboot")?;
        }
    }

    Ok(())
}

/// Execute a command with timeout.
///
/// A `timeout_secs` of 0 is treated as "use default", not "instant".
async fn execute_with_timeout(
    vm: &VmHandle,
    command: &str,
    timeout_secs: u64,
    username: &str,
) -> Result<()> {
    let effective = if timeout_secs == 0 {
        DEFAULT_SSH_CMD_TIMEOUT_SECS
    } else {
        timeout_secs
    };
    let duration = Duration::from_secs(effective);

    timeout(duration, vm.ssh_exec(username, command))
        .await
        .with_context(|| format!("Command timed out after {effective}s"))??;

    Ok(())
}

/// Execute package install with progress monitoring (OS-agnostic)
#[allow(clippy::too_many_lines)]
async fn execute_package_install_monitored(
    vm: &VmHandle,
    packages: &[String],
    timeout_secs: u64,
    username: &str,
    pm: crate::templates::PackageManager,
) -> Result<()> {
    info!(
        "     Starting monitored {:?} install (timeout: {}s)",
        pm, timeout_secs
    );

    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("System clock error: time is before UNIX epoch: {}", e))?
        .as_secs();
    let marker_file = format!("/tmp/pkg-progress-{}.log", unique_id);
    let completion_marker = format!("/tmp/pkg-complete-{}", unique_id);
    let script_path = format!("/tmp/pkg-install-{}.sh", unique_id);
    let cleanup_cmd = format!(
        "rm -f {} {} {}",
        marker_file, completion_marker, script_path
    );
    let _ = vm.ssh_exec(username, &cleanup_cmd).await;

    let env_preamble = match pm {
        crate::templates::PackageManager::Apt | crate::templates::PackageManager::Auto => {
            "export DEBIAN_FRONTEND=noninteractive\nexport NEEDRESTART_MODE=a\nexport NEEDRESTART_SUSPEND=1"
        }
        _ => "",
    };

    let script_path = format!("/tmp/pkg-install-{}.sh", unique_id);
    let script_content = format!(
        r#"#!/bin/bash
# Auto-generated package install script
{env_preamble}

# Run install and log output
sudo {install_cmd} {pkgs} 2>&1 | tee {log}

# Write completion marker
echo "DONE" > {done}
"#,
        env_preamble = env_preamble,
        install_cmd = pm.install_command(),
        pkgs = packages.join(" "),
        log = marker_file,
        done = completion_marker,
    );

    // Write script to VM
    let write_script_cmd = format!(
        "cat > {} << 'SCRIPT_EOF'\n{}\nSCRIPT_EOF\nchmod +x {}",
        script_path, script_content, script_path
    );

    info!("     Creating install script: {}", script_path);
    vm.ssh_exec(username, &write_script_cmd).await?;

    // Execute script in background
    let exec_cmd = format!("nohup {} > /dev/null 2>&1 &", script_path);
    info!("     Launching: {} {}", pm.install_command(), packages.join(" "));
    vm.ssh_exec(username, &exec_cmd).await?;

    // Give the background process time to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Monitor progress by checking for completion marker
    let start = std::time::Instant::now();
    let duration = Duration::from_secs(timeout_secs);
    let mut last_size = 0u64;
    let mut stall_count = 0;

    loop {
        if start.elapsed() > duration {
            anyhow::bail!("apt-get install timed out after {}s", timeout_secs);
        }

        tokio::time::sleep(Duration::from_secs(5)).await;

        // IDIOMATIC RUST: Explicit error handling with proper logging
        // Check for completion marker with retry for transient SSH issues
        let check_cmd = format!(
            "test -f {} && echo 'done' || echo 'running'",
            completion_marker
        );

        let status = match vm.ssh_exec(username, &check_cmd).await {
            Ok(output) => output,
            Err(e) => {
                // OBSERVABILITY: Log SSH failures explicitly
                warn!("     ⚠️  SSH check failed: {} (will retry)", e);
                // Don't bail immediately - SSH might be temporarily busy
                // Continue loop and let timeout handle persistent failures
                "error".to_string()
            }
        };

        let status_trimmed = status.trim();

        // OBSERVABILITY: Debug log for troubleshooting
        if status_trimmed != "running" {
            info!(
                "     🔍 Completion check: status='{}' (expecting 'done')",
                status_trimmed
            );
        }

        if status_trimmed == "done" {
            info!("     Package install completed");
            break;
        }

        // Get progress info
        let elapsed = start.elapsed().as_secs();
        let remaining = timeout_secs.saturating_sub(elapsed);

        // Check log file size as a proxy for progress
        let size_cmd = format!("stat -c %s {} 2>/dev/null || echo 0", marker_file);
        let size_str = match vm.ssh_exec(username, &size_cmd).await {
            Ok(s) => s,
            Err(e) => {
                warn!("     ⚠️  Failed to check log size: {}", e);
                "0".to_string()
            }
        };
        let current_size = size_str.trim().parse::<u64>().unwrap_or(0);

        // Check for stall (no new output)
        if current_size == last_size {
            stall_count += 1;
            if stall_count >= 6 {
                // 30 seconds of no output
                warn!(
                    "     ⚠️  No progress for 30s - possible stall? ({}s elapsed, {}s remaining)",
                    elapsed, remaining
                );

                // Show last few lines of log
                let tail_cmd = format!("tail -3 {} 2>/dev/null || echo 'No log yet'", marker_file);
                if let Ok(tail) = vm.ssh_exec(username, &tail_cmd).await {
                    for line in tail.lines() {
                        info!("        📝 {}", line.trim());
                    }
                }
                stall_count = 0; // Reset to avoid spam
            }
        } else {
            stall_count = 0;
            let kb = current_size / 1024;
            info!(
                "     ⏳ Installing... ({} KB logged, {}s elapsed, {}s remaining)",
                kb, elapsed, remaining
            );

            // Show last line of progress
            let tail_cmd = format!("tail -1 {} 2>/dev/null", marker_file);
            if let Ok(tail) = vm.ssh_exec(username, &tail_cmd).await {
                let line = tail.trim();
                if !line.is_empty() && line.len() < 200 {
                    info!("        📝 {}", line);
                }
            }
        }

        last_size = current_size;
    }

    info!("     Cleaning up install artifacts");
    let cleanup_cmd = format!("rm -f {} {} {}", marker_file, completion_marker, script_path);
    let _ = vm.ssh_exec(username, &cleanup_cmd).await;

    Ok(())
}

/// Execute a command with retry
async fn execute_with_retry(
    vm: &VmHandle,
    command: &str,
    timeout_secs: u64,
    max_retries: usize,
    username: &str,
) -> Result<()> {
    let mut last_error = None;

    for attempt in 1..=max_retries {
        match execute_with_timeout(vm, command, timeout_secs, username).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                warn!("     ⚠️  Attempt {}/{} failed: {}", attempt, max_retries, e);
                last_error = Some(e);

                if attempt < max_retries {
                    info!("       Retrying in 5 seconds...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries failed")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_boot_step_serialization() {
        let step = PostBootStep::InstallPackages {
            packages: vec!["vim".to_string(), "curl".to_string()],
            retry: true,
            timeout_secs: 300,
            description: Some("Install basic tools".to_string()),
        };

        let yaml = serde_yaml::to_string(&step).unwrap();
        assert!(yaml.contains("install_packages"));
        assert!(yaml.contains("vim"));
    }

    #[test]
    fn post_boot_reboot_and_create_file_roundtrip() {
        let reboot = PostBootStep::Reboot { wait_secs: 30 };
        let y = serde_yaml::to_string(&reboot).unwrap();
        let back: PostBootStep = serde_yaml::from_str(&y).unwrap();
        assert!(matches!(back, PostBootStep::Reboot { wait_secs: 30 }));

        let cf = PostBootStep::CreateFile {
            path: "/etc/foo.conf".to_string(),
            content: "x=1\n".to_string(),
            mode: "0640".to_string(),
            owner: Some("root:root".to_string()),
        };
        let y2 = serde_yaml::to_string(&cf).unwrap();
        assert!(y2.contains("create_file"));
    }

    #[test]
    fn post_boot_run_command_enable_copy_roundtrip() {
        let steps = vec![
            PostBootStep::RunCommand {
                command: "systemctl daemon-reload".to_string(),
                description: Some("reload".to_string()),
                timeout_secs: 120,
            },
            PostBootStep::EnableService {
                service: "nginx".to_string(),
                start: true,
            },
            PostBootStep::CopyFile {
                source: "/host/a".to_string(),
                destination: "/guest/a".to_string(),
                mode: "0644".to_string(),
            },
        ];
        let yaml = serde_yaml::to_string(&steps).expect("to yaml");
        let back: Vec<PostBootStep> = serde_yaml::from_str(&yaml).expect("from yaml");
        assert_eq!(back.len(), 3);
        assert!(matches!(
            &back[1],
            PostBootStep::EnableService { service, start: true } if service == "nginx"
        ));
    }
}
