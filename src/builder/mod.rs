// SPDX-License-Identifier: AGPL-3.0-or-later
// agentReagents Image Builder
// Modern idiomatic Rust implementation replacing bash scripts

mod cloud_init_monitor;
pub mod network;
mod post_boot; // NEW: Post-boot step executor
mod state;
pub mod verification;
pub mod vm_handle;
/// Deep reboot handling, SSH polling, and boot diagnostics.
pub mod vm_reboot;

mod cloud_init;
mod vm_create;

pub use cloud_init_monitor::{CloudInitStage, CloudInitStatus};
pub use network::NetworkMonitor;
pub use state::{BuildProgress, BuildState};
pub use verification::VerificationResult;
pub use vm_handle::{CloudInitStatusInfo, VmHandle};

use crate::templates::TemplateManifest;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use tokio::time::{Duration, sleep, timeout};
use tracing::{debug, error, info, instrument, warn};
// Serialization handled by sub-modules

/// Image builder for creating VM templates with desktop environments
///
/// This builder is manifest-driven: it takes a TemplateManifest and
/// executes the build steps defined within. This ensures all builds
/// are declarative and reproducible.
pub struct ImageBuilder {
    /// Template manifest defining the build
    manifest: TemplateManifest,
    /// Build timeout (can override manifest)
    timeout: Duration,
    /// Current build state
    state: BuildState,
    /// Optional plasmidBin path for injecting primal binaries
    plasmid_bin_path: Option<PathBuf>,
}

/// Result of a successful build
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Output template image path (e.g. qcow2).
    pub template_path: PathBuf,
    /// Size of the template file in bytes.
    pub size_bytes: u64,
    /// Wall-clock time spent on the build.
    pub build_duration: Duration,
    /// Outcome of post-build verification checks.
    pub verification: VerificationResult,
}

impl ImageBuilder {
    /// Create a new manifest-driven image builder
    ///
    /// Deep debt solution: All builds are now manifest-driven, ensuring
    /// reproducibility and declarative configuration.
    pub fn from_manifest(manifest: TemplateManifest) -> Self {
        let timeout = Duration::from_secs(manifest.resources.timeout_secs);
        Self {
            manifest,
            timeout,
            state: BuildState::Idle,
            plasmid_bin_path: None,
        }
    }

    /// Set build timeout (override manifest default)
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Set a `plasmidBin` path so primal binaries are baked into the image.
    ///
    /// When set, the builder will copy binaries from `<path>/primals/<arch>/`
    /// into `/opt/biomeos/bin/` during the build, making gate images ship
    /// with pre-deployed primals.
    pub fn with_plasmid_bin(mut self, path: PathBuf) -> Self {
        self.plasmid_bin_path = Some(path);
        self
    }

    /// Returns the plasmidBin path, if configured.
    pub fn plasmid_bin_path(&self) -> Option<&PathBuf> {
        self.plasmid_bin_path.as_ref()
    }

    /// Get current build state
    pub fn state(&self) -> &BuildState {
        &self.state
    }

    /// Get the template manifest
    pub fn manifest(&self) -> &TemplateManifest {
        &self.manifest
    }

    /// Get the VM name from manifest
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Build a COSMIC desktop image
    ///
    /// DEPRECATED: Use the manifest-driven `build()` method instead.
    /// This method exists for backward compatibility.
    #[instrument(skip(self))]
    #[deprecated(note = "Use manifest-driven build() method instead")]
    pub async fn build_cosmic_desktop(&mut self, ssh_public_key: String) -> Result<BuildResult> {
        let start_time = std::time::Instant::now();

        info!("Starting COSMIC desktop build: {}", self.manifest.name);
        self.transition_to(BuildState::Starting);

        // Create cloud-init configuration from manifest
        let cloud_init = self.create_cloud_init(ssh_public_key);

        // Create builder VM
        self.transition_to(BuildState::CreatingVm);
        let (vm_handle, vm_guard) = self
            .create_builder_vm(cloud_init)
            .await
            .context("Failed to create builder VM")?;

        // VmGuard will clean up VM if we return early (panic, timeout, error)
        // We'll preserve it explicitly at the end if build succeeds

        // Monitor the build process with timeout
        self.transition_to(BuildState::Monitoring);
        let monitor_result = timeout(self.timeout, self.monitor_build_process(&vm_handle)).await;

        match monitor_result {
            Ok(Ok(())) => {
                info!("Build monitoring completed successfully");
            }
            Ok(Err(e)) => {
                self.transition_to(BuildState::Failed {
                    reason: e.to_string(),
                });
                return Err(e).context("Build process failed");
            }
            Err(_) => {
                self.transition_to(BuildState::Failed {
                    reason: "Build timeout".to_string(),
                });
                bail!("Build timeout after {:?}", self.timeout);
            }
        }

        // Verify installation
        self.transition_to(BuildState::Verifying);
        let verification = self
            .verify_installation(&vm_handle)
            .await
            .context("Installation verification failed")?;

        if !verification.passed {
            self.transition_to(BuildState::Failed {
                reason: "Verification failed".to_string(),
            });
            bail!("Verification failed: {:?}", verification);
        }

        // Create template
        self.transition_to(BuildState::Finalizing);
        let template_path = self
            .finalize_template(&vm_handle)
            .await
            .context("Template finalization failed")?;

        let size_bytes = tokio::fs::metadata(&template_path).await?.len();

        self.transition_to(BuildState::Complete);

        let build_duration = start_time.elapsed();
        info!("Build completed in {:?}", build_duration);

        // Build succeeded - preserve the VM (prevent cleanup)
        vm_guard.preserve();
        info!("✅ VM preserved (cleanup disabled)");

        Ok(BuildResult {
            template_path,
            size_bytes,
            build_duration,
            verification,
        })
    }

    /// Transition to a new state
    fn transition_to(&mut self, new_state: BuildState) {
        info!("State transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
    }

    /// Phase 1A: Extract common pattern - get username with observability
    ///
    /// Returns the username from the manifest, with proper logging:
    /// - If manifest has users: Returns first user's name  
    /// - If no users: Returns "ubuntu" with warning (may fail on non-Ubuntu systems)
    fn get_username_with_fallback(&self) -> &str {
        if let Some(user) = self.manifest.users.first() {
            debug!("Using username from manifest: {}", user.name);
            &user.name
        } else {
            warn!(
                "No users in manifest, defaulting to 'ubuntu' - \
                 this may fail on non-Ubuntu systems. \
                 Add a user to your manifest for reliability."
            );
            "ubuntu"
        }
    }

    /// Monitor the build process
    ///
    /// # Evolution #19: Username from Manifest (Deep Debt Fix)
    ///
    /// FIXED: Hardcoded "ubuntu" username caused auth failures.
    /// NOW: Uses username from manifest (same pattern as senescence monitor).
    #[instrument(skip(self, vm))]
    async fn monitor_build_process(&mut self, vm: &VmHandle) -> Result<()> {
        info!("Monitoring build process...");

        // Phase 1A: Use helper method with proper logging
        let username = self.get_username_with_fallback();
        info!("Using SSH user: {}", username);

        loop {
            sleep(Duration::from_secs(5)).await;

            // Check if VM is still running
            if !vm.is_running().await? {
                info!("VM has powered off");
                break;
            }

            // Try to get cloud-init status
            match vm.get_cloud_init_status(username).await {
                Ok(status) => {
                    if status.finished {
                        info!("Cloud-init completed successfully");
                        self.transition_to(BuildState::Complete);
                        break;
                    } else if !status.errors.is_empty() {
                        error!("Cloud-init errors: {:?}", status.errors);
                        bail!("Cloud-init failed: {:?}", status.errors);
                    }
                    info!("Cloud-init running: {}", status.status);
                }
                Err(e) => {
                    // SSH might not be ready yet
                    warn!("Could not get cloud-init status: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Verify the installation against manifest expectations
    ///
    /// Uses the comprehensive verification system to check that all
    /// packages, services, and configurations from the manifest are
    /// correctly applied.
    #[instrument(skip(self, vm))]
    async fn verify_installation(&self, vm: &VmHandle) -> Result<VerificationResult> {
        info!(
            "Verifying installation for template: {}",
            self.manifest.name
        );

        // Use the comprehensive verification system with our manifest
        let result = verification::verify_installation(vm, &self.manifest).await?;

        if result.passed {
            info!("✅ Verification passed: {}", result.summary());
        } else {
            warn!("⚠️ Verification had failures: {}", result.summary());
            for check in result.failed_checks() {
                warn!("  Failed: {} - {:?}", check.name, check.details);
            }
        }

        Ok(result)
    }

    /// Finalize the template
    ///
    /// Deep debt: Uses capability discovery to find the actual image path
    /// instead of hardcoding /var/lib/libvirt/images
    #[instrument(skip(self, vm))]
    async fn finalize_template(&self, vm: &VmHandle) -> Result<PathBuf> {
        info!("Finalizing template...");

        // Shutdown VM if it's still running
        info!("Shutting down VM...");
        // Stop VM using Backend trait
        use benchscale::backend::Backend;
        let _ = vm.backend().delete_node(vm.node().name.as_str()).await; // Ignore errors if already stopped

        // Wait for shutdown
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Deep debt solution: Use capability discovery to find actual image path
        // The backend discovered this at runtime, we just query it
        let template_path = vm
            .backend()
            .capabilities()
            .storage
            .images_dir
            .join(format!("{}.qcow2", vm.node().name));

        info!("Template created at: {}", template_path.display());
        info!("✅ Template finalized!");

        Ok(template_path)
    }
}

// VmHandle is now provided by vm_handle module
