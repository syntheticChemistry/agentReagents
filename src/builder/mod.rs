// agentReagents Image Builder
// Modern idiomatic Rust implementation replacing bash scripts

mod state;
mod cloud_init_monitor;
mod verification;
mod executor;
pub mod vm_handle;

pub use state::{BuildState, BuildProgress};
pub use cloud_init_monitor::{CloudInitStatus, CloudInitStage};
pub use verification::VerificationResult;
pub use vm_handle::{VmHandle, CloudInitStatusInfo};

use executor::{execute_build_steps, verify_from_manifest};

use anyhow::{Result, Context, bail};
use benchscale::backend::{Backend, LibvirtBackend};
use benchscale::CloudInit as BenchScaleCloudInit;
use std::path::PathBuf;
use tokio::time::{sleep, Duration, timeout};
use tracing::{info, warn, error, instrument};
// Serialization handled by sub-modules

/// Image builder for creating VM templates with desktop environments
pub struct ImageBuilder {
    name: String,
    base_image: PathBuf,
    memory_mb: usize,
    vcpus: usize,
    disk_size_gb: usize,
    timeout: Duration,
    state: BuildState,
}

/// Result of a successful build
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub template_path: PathBuf,
    pub size_bytes: u64,
    pub build_duration: Duration,
    pub verification: VerificationResult,
}

impl ImageBuilder {
    /// Create a new image builder
    pub fn new(name: impl Into<String>, base_image: PathBuf) -> Self {
        Self {
            name: name.into(),
            base_image,
            memory_mb: 4096,
            vcpus: 2,
            disk_size_gb: 30,
            timeout: Duration::from_secs(40 * 60), // 40 minutes
            state: BuildState::Idle,
        }
    }

    /// Set memory in MB
    pub fn memory(mut self, mb: usize) -> Self {
        self.memory_mb = mb;
        self
    }

    /// Set number of vCPUs
    pub fn vcpus(mut self, count: usize) -> Self {
        self.vcpus = count;
        self
    }

    /// Set disk size in GB
    pub fn disk_size(mut self, gb: usize) -> Self {
        self.disk_size_gb = gb;
        self
    }

    /// Set build timeout
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Get current build state
    pub fn state(&self) -> &BuildState {
        &self.state
    }

    /// Build a COSMIC desktop image
    #[instrument(skip(self))]
    pub async fn build_cosmic_desktop(
        &mut self,
        ssh_public_key: String,
    ) -> Result<BuildResult> {
        let start_time = std::time::Instant::now();
        
        info!("Starting COSMIC desktop build: {}", self.name);
        self.transition_to(BuildState::Starting)?;

        // Create cloud-init configuration
        let cloud_init = self.create_cosmic_cloud_init(ssh_public_key)?;
        
        // Create builder VM
        self.transition_to(BuildState::CreatingVm)?;
        let vm_handle = self.create_builder_vm(cloud_init).await
            .context("Failed to create builder VM")?;
        
        // Monitor the build process with timeout
        self.transition_to(BuildState::Monitoring)?;
        let monitor_result = timeout(
            self.timeout,
            self.monitor_build_process(&vm_handle)
        ).await;

        match monitor_result {
            Ok(Ok(_)) => {
                info!("Build monitoring completed successfully");
            }
            Ok(Err(e)) => {
                self.transition_to(BuildState::Failed { 
                    reason: e.to_string() 
                })?;
                return Err(e).context("Build process failed");
            }
            Err(_) => {
                self.transition_to(BuildState::Failed { 
                    reason: "Build timeout".to_string() 
                })?;
                bail!("Build timeout after {:?}", self.timeout);
            }
        }

        // Verify installation
        self.transition_to(BuildState::Verifying)?;
        let verification = self.verify_installation(&vm_handle).await
            .context("Installation verification failed")?;
        
        if !verification.is_success() {
            self.transition_to(BuildState::Failed { 
                reason: "Verification failed".to_string() 
            })?;
            bail!("Verification failed: {:?}", verification);
        }

        // Create template
        self.transition_to(BuildState::Finalizing)?;
        let template_path = self.finalize_template(&vm_handle).await
            .context("Template finalization failed")?;
        
        let size_bytes = tokio::fs::metadata(&template_path).await?.len();
        
        self.transition_to(BuildState::Complete)?;
        
        let build_duration = start_time.elapsed();
        info!("Build completed in {:?}", build_duration);

        Ok(BuildResult {
            template_path,
            size_bytes,
            build_duration,
            verification,
        })
    }

    /// Transition to a new state
    fn transition_to(&mut self, new_state: BuildState) -> Result<()> {
        info!("State transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
        Ok(())
    }

    /// Create cloud-init configuration for COSMIC desktop
    fn create_cosmic_cloud_init(&self, ssh_public_key: String) -> Result<String> {
        // TODO: Use type-safe cloud-init builder from benchScale
        // For now, generate YAML directly
        
        let cloud_init = format!(r#"#cloud-config
users:
  - name: cosmic
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - {}

chpasswd:
  list: |
    cosmic:cosmic2025
  expire: false

package_update: true
package_upgrade: true

packages:
  - build-essential
  - git
  - curl
  - wget
  - vim
  - libwayland-client0
  - libwayland-server0
  - xwayland
  - software-properties-common
  - gnupg2
  - ca-certificates
  - openssh-server
  - avahi-daemon
  - net-tools
  - dbus-x11
  - pipewire
  - wireplumber

runcmd:
  - echo "Adding System76 COSMIC repository..."
  - curl -fsSL https://apt.system76.com/signing-key.asc | gpg --dearmor -o /etc/apt/keyrings/system76.gpg
  - echo "deb [signed-by=/etc/apt/keyrings/system76.gpg] https://apt.system76.com/cosmic noble main" | tee /etc/apt/sources.list.d/system76-cosmic.list
  - apt-get update
  - echo "Installing COSMIC Desktop..."
  - DEBIAN_FRONTEND=noninteractive apt-get install -y cosmic-session cosmic-greeter cosmic-comp cosmic-panel cosmic-launcher cosmic-applets cosmic-settings cosmic-files cosmic-term cosmic-edit
  - systemctl enable cosmic-greeter
  - systemctl set-default graphical.target
  - systemctl enable ssh
  - systemctl start ssh
  - apt-get autoremove -y
  - apt-get clean
  - sync

power_state:
  mode: poweroff
  timeout: 2400
  condition: true

final_message: |
  COSMIC installation complete!
  System will power off.
"#, ssh_public_key);

        Ok(cloud_init)
    }

    /// Create the builder VM (placeholder - will integrate with benchScale)
    async fn create_builder_vm(&self, _cloud_init: String) -> Result<VmHandle> {
        // TODO: Integrate with benchScale VM creation
        todo!("Integrate with benchScale VM creation")
    }

    /// Monitor the build process
    #[instrument(skip(self, vm))]
    async fn monitor_build_process(&mut self, vm: &VmHandle) -> Result<()> {
        info!("Monitoring build process...");
        
        loop {
            sleep(Duration::from_secs(5)).await;
            
            // Check if VM is still running
            if !vm.is_running().await? {
                info!("VM has powered off");
                break;
            }
            
            // Try to get cloud-init status
            match vm.get_cloud_init_status("ubuntu").await {
                Ok(status) => {
                    if status.finished {
                        info!("Cloud-init completed successfully");
                        self.transition_to(BuildState::Complete)?;
                        break;
                    } else if !status.errors.is_empty() {
                        error!("Cloud-init errors: {:?}", status.errors);
                        bail!("Cloud-init failed: {:?}", status.errors);
                    } else {
                        info!("Cloud-init running: {}", status.status);
                    }
                }
                Err(e) => {
                    // SSH might not be ready yet
                    warn!("Could not get cloud-init status: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// Update build state based on cloud-init status
    fn update_state_from_cloud_init(&mut self, status: &CloudInitStatus) -> Result<()> {
        let new_state = match status {
            CloudInitStatus::Running { stage } => {
                match stage {
                    CloudInitStage::Init => BuildState::CloudInitInit,
                    CloudInitStage::Config => BuildState::InstallingPackages { progress: 0.3 },
                    CloudInitStage::Final => BuildState::Finalizing,
                    _ => return Ok(()), // Don't transition for other stages
                }
            }
            CloudInitStatus::Done => BuildState::CloudInitComplete,
            CloudInitStatus::Error { .. } => return Ok(()), // Error handled elsewhere
        };
        
        self.transition_to(new_state)
    }

    /// Verify the installation
    #[instrument(skip(self, _vm))]
    async fn verify_installation(&self, _vm: &VmHandle) -> Result<VerificationResult> {
        info!("Verifying installation...");
        
        // TODO: Implement verification
        // - Check COSMIC packages installed
        // - Check cosmic-greeter enabled
        // - Check RustDesk if applicable
        
        todo!("Implement verification")
    }

    /// Finalize the template
    #[instrument(skip(self, _vm))]
    async fn finalize_template(&self, _vm: &VmHandle) -> Result<PathBuf> {
        info!("Finalizing template...");
        
        // TODO: Implement template finalization
        // - Run virt-sparsify
        // - Move to final location
        // - Set permissions
        
        todo!("Implement template finalization")
    }
}

// VmHandle is now provided by vm_handle module

