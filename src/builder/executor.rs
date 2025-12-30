//! Build step executor
//!
//! Executes build steps from manifests with concurrency and proper error handling

use super::VmHandle;
use crate::templates::BuildStep;
use anyhow::{Context, Result};
use tokio::time::Duration;
use tracing::{debug, info};

/// Execute build steps, with concurrency where possible
///
/// TODO: This will be used by the full manifest-driven build() method
#[allow(dead_code)]
pub async fn execute_build_steps(vm: &VmHandle, steps: &[BuildStep]) -> Result<()> {
    for (idx, step) in steps.iter().enumerate() {
        info!("Executing step {}/{}: {:?}", idx + 1, steps.len(), step);
        execute_build_step(vm, step)
            .await
            .with_context(|| format!("Failed to execute step {}: {:?}", idx + 1, step))?;
    }

    Ok(())
}

/// Execute a single build step
async fn execute_build_step(vm: &VmHandle, step: &BuildStep) -> Result<()> {
    match step {
        BuildStep::WaitCloudInit { timeout_secs } => {
            vm.wait_for_cloud_init("ubuntu", Duration::from_secs(*timeout_secs))
                .await?;
        }

        BuildStep::AddRepository { name, url, key_url } => {
            info!("Adding repository: {}", name);

            // Add GPG key if provided
            if let Some(key) = key_url {
                let cmd = format!(
                    "curl -fsSL {} | sudo gpg --dearmor -o /usr/share/keyrings/{}.gpg",
                    key, name
                );
                vm.ssh_exec("ubuntu", &cmd).await?;
            }

            // Add repository
            let repo_file = format!("/etc/apt/sources.list.d/{}.list", name);
            let cmd = format!("echo 'deb {} noble main' | sudo tee {}", url, repo_file);
            vm.ssh_exec("ubuntu", &cmd).await?;

            // Update package lists
            vm.ssh_exec("ubuntu", "sudo apt-get update").await?;
        }

        BuildStep::InstallPackages { packages } => {
            vm.install_packages("ubuntu", packages).await?;
        }

        BuildStep::RunCommand {
            command,
            description,
        } => {
            if let Some(desc) = description {
                info!("Running: {}", desc);
            }
            vm.ssh_exec("ubuntu", command).await?;
        }

        BuildStep::EnableService { service } => {
            info!("Enabling service: {}", service);
            let cmd = format!("sudo systemctl enable {}", service);
            vm.ssh_exec("ubuntu", &cmd).await?;
        }

        BuildStep::CreateFile {
            path,
            content,
            mode,
        } => {
            info!("Creating file: {}", path);

            // Create file with content
            let cmd = format!("echo '{}' | sudo tee {}", content, path);
            vm.ssh_exec("ubuntu", &cmd).await?;

            // Set permissions if specified
            if let Some(m) = mode {
                let chmod_cmd = format!("sudo chmod {} {}", m, path);
                vm.ssh_exec("ubuntu", &chmod_cmd).await?;
            }
        }

        BuildStep::DownloadFile { url, dest } => {
            info!("Downloading: {} -> {}", url, dest);
            let cmd = format!("curl -fsSL -o {} {}", dest, url);
            vm.ssh_exec("ubuntu", &cmd).await?;
        }

        BuildStep::Reboot { wait_secs } => {
            info!("Rebooting VM...");

            // Initiate reboot (don't wait for response as connection will drop)
            let _ = vm.ssh_exec("ubuntu", "sudo reboot").await;

            // Wait for reboot
            tokio::time::sleep(Duration::from_secs(*wait_secs)).await;

            // Verify VM is back up
            vm.wait_for_cloud_init("ubuntu", Duration::from_secs(300))
                .await?;
        }
    }

    Ok(())
}

/// Verify VM against manifest requirements
///
/// TODO: This will be used by the full manifest-driven build() method
#[allow(dead_code)]
pub async fn verify_from_manifest(
    vm: &VmHandle,
    manifest: &crate::templates::TemplateManifest,
) -> Result<crate::builder::VerificationResult> {
    let mut errors = Vec::new();
    let mut cosmic_installed = false;
    let mut cosmic_package_count = 0;
    let mut greeter_enabled = false;
    let mut rustdesk_installed = false;
    let ssh_accessible = true; // Already SSH'd in, so this is true

    // Verify required packages
    for package in &manifest.verification.required_packages {
        debug!("Checking package: {}", package);
        let installed = vm.is_package_installed("ubuntu", package).await?;

        if !installed {
            errors.push(format!("Package {} not installed", package));
        } else {
            // Track specific packages
            if package.contains("cosmic") {
                cosmic_installed = true;
                cosmic_package_count += 1;
            }
            if package.contains("rustdesk") {
                rustdesk_installed = true;
            }
        }
    }

    // Verify required services
    for service in &manifest.verification.required_services {
        debug!("Checking service: {}", service);
        let cmd = format!("systemctl is-enabled {}", service);
        let service_enabled = vm.ssh_exec("ubuntu", &cmd).await.is_ok();

        if !service_enabled {
            errors.push(format!("Service {} not enabled", service));
        } else if service.contains("greeter") {
            greeter_enabled = true;
        }
    }

    // Verify required files
    for file in &manifest.verification.required_files {
        debug!("Checking file: {}", file);
        let cmd = format!("test -f {}", file);
        let file_exists = vm.ssh_exec("ubuntu", &cmd).await.is_ok();

        if !file_exists {
            errors.push(format!("File {} not found", file));
        }
    }

    // Run verification commands
    for vcmd in &manifest.verification.verification_commands {
        if let Some(desc) = &vcmd.description {
            debug!("Running verification: {}", desc);
        }

        let output = vm.ssh_exec("ubuntu", &vcmd.command).await;
        if output.is_err() {
            errors.push(format!(
                "Verification command failed: {}",
                vcmd.description.as_ref().unwrap_or(&vcmd.command)
            ));
        }
    }

    // Convert to new verification format
    use crate::builder::verification::{
        VerificationCheck, VerificationResult as NewVerificationResult,
    };

    let mut checks = Vec::new();

    // Add check results
    checks.push(VerificationCheck {
        name: "COSMIC installed".to_string(),
        passed: cosmic_installed,
        details: Some(format!("{} COSMIC packages found", cosmic_package_count)),
    });

    checks.push(VerificationCheck {
        name: "COSMIC greeter enabled".to_string(),
        passed: greeter_enabled,
        details: None,
    });

    checks.push(VerificationCheck {
        name: "RustDesk installed".to_string(),
        passed: rustdesk_installed,
        details: None,
    });

    checks.push(VerificationCheck {
        name: "SSH accessible".to_string(),
        passed: ssh_accessible,
        details: None,
    });

    // Add error checks
    for error in errors {
        checks.push(VerificationCheck {
            name: "Verification".to_string(),
            passed: false,
            details: Some(error),
        });
    }

    Ok(NewVerificationResult::from_checks(checks))
}
