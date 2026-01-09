// agentReagents Image Builder
// Modern idiomatic Rust implementation replacing bash scripts

mod cloud_init_monitor;
mod executor;
pub mod network;
mod post_boot; // NEW: Post-boot step executor
mod state;
mod verification;
pub mod vm_handle;
pub mod vm_reboot; // EVOLUTION #9: Deep reboot diagnostics

pub use cloud_init_monitor::{CloudInitStage, CloudInitStatus};
pub use network::NetworkMonitor;
pub use state::{BuildProgress, BuildState};
pub use verification::VerificationResult;
pub use vm_handle::{CloudInitStatusInfo, VmHandle};

use benchscale::backend::senescence::SenescenceMonitor;
// Phase 2B: Configuration system
use benchscale::config::{BenchScaleConfig, MonitoringConfig, TimeoutConfig};
use crate::templates::{TemplateManifest, PostBootStep};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{sleep, timeout, Duration};
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
        }
    }

    /// Set build timeout (override manifest default)
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
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
        self.transition_to(BuildState::Starting)?;

        // Create cloud-init configuration from manifest
        let cloud_init = self.create_cloud_init(ssh_public_key)?;

        // Create builder VM
        self.transition_to(BuildState::CreatingVm)?;
        let (vm_handle, mut _vm_guard) = self
            .create_builder_vm(cloud_init)
            .await
            .context("Failed to create builder VM")?;
        
        // VmGuard will clean up VM if we return early (panic, timeout, error)
        // We'll preserve it explicitly at the end if build succeeds

        // Monitor the build process with timeout
        self.transition_to(BuildState::Monitoring)?;
        let monitor_result = timeout(self.timeout, self.monitor_build_process(&vm_handle)).await;

        match monitor_result {
            Ok(Ok(_)) => {
                info!("Build monitoring completed successfully");
            }
            Ok(Err(e)) => {
                self.transition_to(BuildState::Failed {
                    reason: e.to_string(),
                })?;
                return Err(e).context("Build process failed");
            }
            Err(_) => {
                self.transition_to(BuildState::Failed {
                    reason: "Build timeout".to_string(),
                })?;
                bail!("Build timeout after {:?}", self.timeout);
            }
        }

        // Verify installation
        self.transition_to(BuildState::Verifying)?;
        let verification = self
            .verify_installation(&vm_handle)
            .await
            .context("Installation verification failed")?;

        if !verification.passed {
            self.transition_to(BuildState::Failed {
                reason: "Verification failed".to_string(),
            })?;
            bail!("Verification failed: {:?}", verification);
        }

        // Create template
        self.transition_to(BuildState::Finalizing)?;
        let template_path = self
            .finalize_template(&vm_handle)
            .await
            .context("Template finalization failed")?;

        let size_bytes = tokio::fs::metadata(&template_path).await?.len();

        self.transition_to(BuildState::Complete)?;

        let build_duration = start_time.elapsed();
        info!("Build completed in {:?}", build_duration);
        
        // Build succeeded - preserve the VM (prevent cleanup)
        _vm_guard.preserve();
        info!("✅ VM preserved (cleanup disabled)");

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

    /// Extract packages from post_boot_steps that should be installed via cloud-init
    ///
    /// SUDO-FREE ARCHITECTURE: Move standard apt packages from post-boot (sudo) to cloud-init (native root).
    ///
    /// # Strategy
    ///
    /// - **Standard packages** (in repos): Install via cloud-init's native package manager
    /// - **Custom binaries** (RustDesk, ionChannel): Keep in post-boot for download/install
    ///
    /// This eliminates sudo entirely for package installation.
    fn extract_cloud_init_packages(&self) -> Vec<String> {
        use crate::templates::PostBootStep;
        
        let mut packages = Vec::new();
        
        for step in &self.manifest.post_boot_steps {
            if let PostBootStep::InstallPackages { packages: pkg_list, .. } = step {
                for pkg in pkg_list {
                    // Only extract standard apt packages
                    // Custom binaries (.deb files, downloads) stay in post-boot
                    if Self::is_standard_apt_package(pkg) {
                        packages.push(pkg.clone());
                    }
                }
            }
        }
        
        info!("Extracted {} packages for cloud-init installation", packages.len());
        packages
    }

    /// Determine if a package should be installed via cloud-init (true) or post-boot (false)
    ///
    /// # HYBRID APPROACH (Optimized for Speed + Visibility)
    ///
    /// ## Cloud-init packages (infrastructure only, fast ~30s)
    /// - openssh-server, curl, wget, net-tools
    /// - Small, essential packages with no dependencies
    ///
    /// ## Post-boot packages (everything else, visible progress)
    /// - Desktop environments (xorg, ubuntu-desktop-minimal, gdm3)
    /// - Applications (firefox, gnome-terminal, etc.)
    /// - Libraries (lib*, pkg-config, etc.)
    /// - Custom binaries (RustDesk, ionChannel)
    ///
    /// **Why**: Cloud-init is slow for large packages (15-20 min) with no progress visibility.
    /// Post-boot gives real-time monitoring and is 2-3x faster.
    fn is_standard_apt_package(pkg: &str) -> bool {
        // ONLY these lightweight infrastructure packages go to cloud-init
        let cloud_init_packages = [
            "openssh-server",
            "curl",
            "wget",
            "net-tools",
        ];
        
        cloud_init_packages.contains(&pkg.to_lowercase().as_str())
    }

    /// Filter post_boot_steps to remove packages that were installed via cloud-init
    ///
    /// SUDO-FREE ARCHITECTURE: Standard packages are now installed in cloud-init,
    /// so we skip them in post-boot to avoid redundant apt calls.
    fn filter_post_boot_steps(&self) -> Vec<PostBootStep> {
        use crate::templates::PostBootStep;
        
        let mut filtered_steps = Vec::new();
        
        for step in &self.manifest.post_boot_steps {
            match step {
                PostBootStep::InstallPackages { packages, retry, timeout_secs, description } => {
                    // Filter out packages that were moved to cloud-init
                    let remaining_packages: Vec<String> = packages
                        .iter()
                        .filter(|pkg| !Self::is_standard_apt_package(pkg))
                        .cloned()
                        .collect();
                    
                    if !remaining_packages.is_empty() {
                        // Keep the step but with only custom packages
                        filtered_steps.push(PostBootStep::InstallPackages {
                            packages: remaining_packages,
                            retry: *retry,
                            timeout_secs: *timeout_secs,
                            description: description.clone(),
                        });
                    } else {
                        info!("Skipping post-boot InstallPackages step (all packages moved to cloud-init)");
                    }
                }
                // All other steps pass through unchanged
                other => filtered_steps.push(other.clone()),
            }
        }
        
        filtered_steps
    }

    /// Create cloud-init configuration from manifest (manifest-driven, idiomatic)
    ///
    /// Deep debt solution: This generates cloud-init entirely from the manifest,
    /// eliminating hardcoding and enabling full declarative configuration.
    fn create_cloud_init(&self, ssh_public_key: String) -> Result<benchscale::CloudInit> {
        use benchscale::CloudInit;

        let mut builder = CloudInit::builder();

        // SUDO-FREE EVOLUTION: Configure apt for non-interactive package installation
        // This eliminates the need for sudo in post-boot scripts entirely
        builder = builder.with_noninteractive_apt();

        // MISE EN PLACE: Configure local package mirror for airgap operation
        // This enables 10-50x faster builds and airgap deployments
        // TEMPORARY: Disabled to isolate idiomatic Rust fix testing
        // builder = builder.with_local_mirror("http://192.168.122.1:8080");

        // Add users from manifest (not hardcoded!)
        if self.manifest.users.is_empty() {
            // Fallback: create default user if none specified
            warn!("No users defined in manifest, creating default 'builder' user");
            builder = builder.add_user("builder", ssh_public_key.clone());
        } else {
            for user in &self.manifest.users {
                builder = builder.add_user(&user.name, ssh_public_key.clone());
            }
        }

        // SUDO-FREE EVOLUTION: Extract packages from post_boot_steps and install via cloud-init
        // This moves standard apt packages from post-boot (sudo) to cloud-init (native root)
        let cloud_init_packages = self.extract_cloud_init_packages();
        if !cloud_init_packages.is_empty() {
            info!("Adding {} packages to cloud-init for sudo-free installation", cloud_init_packages.len());
            builder = builder.packages(cloud_init_packages);
        }

        // Process build steps from manifest using enum pattern matching (idiomatic Rust)
        use crate::templates::BuildStep;

        for step in &self.manifest.build_steps {
            match step {
                BuildStep::InstallPackages { packages } => {
                    for package in packages {
                        builder = builder.package(package);
                    }
                }
                BuildStep::RunCommand {
                    command,
                    description: _,
                } => {
                    builder = builder.runcmd(vec![command.clone()]);
                }
                BuildStep::EnableService { service } => {
                    builder = builder.runcmd(vec![
                        format!("systemctl enable {}", service),
                        format!("systemctl start {}", service),
                    ]);
                }
                BuildStep::CreateFile {
                    path,
                    content,
                    mode: _,
                } => {
                    // Use heredoc for clean multiline content
                    let cmd = format!(
                        "cat > {} <<'EOFAGENTREAGENTS'\n{}\nEOFAGENTREAGENTS",
                        path, content
                    );
                    builder = builder.runcmd(vec![cmd]);
                }
                BuildStep::WaitCloudInit { .. } => {
                    // Handled by monitoring, not cloud-init generation
                }
                BuildStep::DownloadFile { url, dest } => {
                    // Check if we have a local file to inject instead of downloading
                    // Look for local file in packages/ or debs/ directories
                    let local_file = self.find_local_package(url);

                    if let Some(local_path) = local_file {
                        info!(
                            "Using local file instead of downloading: {}",
                            local_path.display()
                        );
                        // Note: File injection happens via write_file in cloud-init
                        // For now, we still download but log the local option
                        builder = builder.runcmd(vec![
                            format!("# Local file available at: {}", local_path.display()),
                            format!(
                                "curl -fsSL -o {} {} || cp {} {} || true",
                                dest,
                                url,
                                local_path.display(),
                                dest
                            ),
                        ]);
                    } else {
                        builder = builder.runcmd(vec![format!("curl -fsSL -o {} {}", dest, url)]);
                    }
                }
                BuildStep::AddRepository { name, url, key_url } => {
                    let mut cmds = vec![];

                    // Add key if provided
                    if let Some(key_url) = key_url {
                        cmds.push(format!(
                            "curl -fsSL {} | gpg --dearmor -o /etc/apt/keyrings/{}.gpg",
                            key_url, name
                        ));
                    }

                    // Add repository (assume noble/24.04 for now, should be configurable)
                    cmds.push(format!(
                        "echo 'deb [signed-by=/etc/apt/keyrings/{}.gpg] {} noble main' | tee /etc/apt/sources.list.d/{}.list",
                        name, url, name
                    ));
                    cmds.push("apt-get update".to_string());

                    builder = builder.runcmd(cmds);
                }
                BuildStep::Reboot { .. } => {
                    builder = builder.runcmd(vec!["reboot".to_string()]);
                }
            }
        }

        Ok(builder.build())
    }

    /// Create cloud-init configuration for COSMIC desktop using benchScale builder
    /// OLD: Create cloud-init YAML string (deprecated)
    fn _create_cosmic_cloud_init_yaml_deprecated(&self, ssh_public_key: String) -> Result<String> {
        let cloud_init = format!(
            r#"#cloud-config
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
"#,
            ssh_public_key
        );

        Ok(cloud_init)
    }

    /// Create the builder VM using benchScale with manifest configuration
    ///
    /// Deep debt: Uses runtime capability discovery via benchScale backend
    /// instead of hardcoded paths and IPs.
    ///
    /// Returns both the VmHandle and a VmGuard for automatic cleanup on failure.
    async fn create_builder_vm(&self, cloud_init: benchscale::CloudInit) -> Result<(VmHandle, benchscale::backend::libvirt::VmGuard)> {
        use benchscale::backend::LibvirtBackend;
        use std::path::Path;
        use std::time::Duration;

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

        // Create VM using benchScale with manifest-defined resources
        let mut node = backend
            .create_desktop_vm(
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
                self.manifest.resources.static_ip.clone(), // DEEP DEBT: Pass static IP from manifest
            )
            .await
            .context("Failed to create desktop VM")?;

        info!("VM created: {} (pool IP: {})", node.name, node.ip_address);

        // Create VmGuard for automatic cleanup on failure
        // Evolution #17: Capability-based path discovery (no hardcoded /var/lib/libvirt)
        // DEEP DEBT SOLUTION: VM will be automatically cleaned up if builder panics or fails
        // Note: We create a new connection for the guard since benchScale uses Arc<Mutex<Connect>>
        let images_dir = backend.capabilities().storage.images_dir.clone();
        let guard = benchscale::backend::libvirt::VmGuard::new(
            self.manifest.name.clone(),
            backend.raw_connection()?, // Get the raw connection from backend
            images_dir // Evolution #17: Pass discovered path, not hardcoded
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
            info!("Evolution #22: MAC address {} will be tracked for IP changes", mac);
        }
        
        // Phase 2B: Use configuration system for monitoring behavior
        // Evolution #21: Configurable max_failures for cloud-init (30min tolerance)
        // Evolution #22: MAC address tracking for periodic IP re-discovery
        let config = BenchScaleConfig {
            monitoring: MonitoringConfig::for_cloud_init_packages(),
            timeouts: TimeoutConfig::default(),
            ..Default::default()
        };
        
        let monitor = Arc::new(
            SenescenceMonitor::from_config(vm_name, actual_ip.clone(), mac_address, &config.monitoring)
        );
        
        // Start background monitoring
        // Phase 1A: Use helper method with proper logging
        let username = self.get_username_with_fallback().to_string();
        
        let monitor_handle = monitor.clone().start_monitoring(username.clone()).await;
        
        // Wait for cloud-init with progress callbacks
        // Phase 2B: Use config for timeout
        let cloud_init_timeout = config.timeouts.cloud_init();
        info!("Waiting for cloud-init with {}min timeout", cloud_init_timeout.as_secs() / 60);
        println!("   ⏱️  Timeout: {}min | Monitoring: ping, SSH, cloud-init status", cloud_init_timeout.as_secs() / 60);
        
        match monitor.wait_for_cloud_init(cloud_init_timeout, |metrics| {
            // Progress callback - called periodically
            println!("   📊 Health: {:?} | Ping: {} | SSH: {} | Uptime: {}s | Failures: {}", 
                metrics.health, 
                if metrics.ping_ok { "✓" } else { "✗" },
                if metrics.ssh_ok { "✓" } else { "✗" },
                metrics.uptime.as_secs(),
                metrics.consecutive_failures
            );
            
            if let Some(ref cloud_init) = metrics.cloud_init {
                println!("   🔧 Cloud-init: {} {}", 
                    cloud_init.status,
                    cloud_init.detail.as_deref().unwrap_or("")
                );
            }
        }).await {
            Ok(_) => {
                info!("✅ Cloud-init completed successfully");
                println!("✅ Cloud-init completed successfully!");
            }
            Err(e) => {
                warn!("⚠️ Cloud-init monitoring ended: {} (VM may still be usable)", e);
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
            info!("Evolution #22: Using updated IP {} for verification (was {})", current_ip, actual_ip);
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
            info!("Filtered post-boot steps: {} steps remaining (packages moved to cloud-init)", filtered_steps.len());
            
            post_boot::execute_post_boot_steps(&vm_handle, &filtered_steps, username)
                .await
                .context("Failed to execute post-boot steps")?;
            println!("✅ Post-boot synthesis complete!");
        }

        Ok((vm_handle, guard))
    }

    /// Phase 1A: Extract common pattern - get username with observability
    /// 
    /// Returns the username from the manifest, with proper logging:
    /// - If manifest has users: Returns first user's name  
    /// - If no users: Returns "ubuntu" with warning (may fail on non-Ubuntu systems)
    fn get_username_with_fallback(&self) -> &str {
        match self.manifest.users.first() {
            Some(user) => {
                debug!("Using username from manifest: {}", user.name);
                &user.name
            }
            None => {
                warn!(
                    "No users in manifest, defaulting to 'ubuntu' - \
                     this may fail on non-Ubuntu systems. \
                     Add a user to your manifest for reliability."
                );
                "ubuntu"
            }
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
    ///
    /// TODO: This will be used by the full manifest-driven build() method
    #[allow(dead_code)]
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

    /// Find local package file for injection instead of downloading
    ///
    /// Looks in packages/ and debs/ directories for matching files.
    /// Returns the path if found, None otherwise.
    fn find_local_package(&self, url: &str) -> Option<PathBuf> {
        // Extract filename from URL
        let filename = url.rsplit('/').next()?;

        // Search in common package directories
        let search_paths = vec![
            PathBuf::from("packages"),
            PathBuf::from("debs/remote-desktop"),
            PathBuf::from("../packages"),
            PathBuf::from("../debs/remote-desktop"),
        ];

        for base in search_paths {
            let candidate = base.join(filename);
            if candidate.exists() {
                info!("Found local package: {}", candidate.display());
                return Some(candidate);
            }
        }

        None
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
