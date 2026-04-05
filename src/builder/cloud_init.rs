// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cloud-init configuration generation (user-data and related manifest-driven setup).

use crate::templates::{BuildStep, PostBootStep};
use benchscale::CloudInit;
use std::path::PathBuf;
use tracing::{info, warn};

use super::ImageBuilder;

impl ImageBuilder {
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
        let mut packages = Vec::new();

        for step in &self.manifest.post_boot_steps {
            if let PostBootStep::InstallPackages {
                packages: pkg_list, ..
            } = step
            {
                for pkg in pkg_list {
                    // Only extract standard apt packages
                    // Custom binaries (.deb files, downloads) stay in post-boot
                    if Self::is_standard_apt_package(pkg) {
                        packages.push(pkg.clone());
                    }
                }
            }
        }

        info!(
            "Extracted {} packages for cloud-init installation",
            packages.len()
        );
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
        let cloud_init_packages = ["openssh-server", "curl", "wget", "net-tools"];

        cloud_init_packages.contains(&pkg.to_lowercase().as_str())
    }

    /// Filter post_boot_steps to remove packages that were installed via cloud-init
    ///
    /// SUDO-FREE ARCHITECTURE: Standard packages are now installed in cloud-init,
    /// so we skip them in post-boot to avoid redundant apt calls.
    pub(crate) fn filter_post_boot_steps(&self) -> Vec<PostBootStep> {
        let mut filtered_steps = Vec::new();

        for step in &self.manifest.post_boot_steps {
            match step {
                PostBootStep::InstallPackages {
                    packages,
                    retry,
                    timeout_secs,
                    description,
                } => {
                    // Filter out packages that were moved to cloud-init
                    let remaining_packages: Vec<String> = packages
                        .iter()
                        .filter(|pkg| !Self::is_standard_apt_package(pkg))
                        .cloned()
                        .collect();

                    if remaining_packages.is_empty() {
                        info!(
                            "Skipping post-boot InstallPackages step (all packages moved to cloud-init)"
                        );
                    } else {
                        // Keep the step but with only custom packages
                        filtered_steps.push(PostBootStep::InstallPackages {
                            packages: remaining_packages,
                            retry: *retry,
                            timeout_secs: *timeout_secs,
                            description: description.clone(),
                        });
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
    pub(super) fn create_cloud_init(&self, ssh_public_key: String) -> CloudInit {
        let mut builder = CloudInit::builder();

        // SUDO-FREE EVOLUTION: Configure apt for non-interactive package installation
        // This eliminates the need for sudo in post-boot scripts entirely
        builder = builder.with_noninteractive_apt();

        // Local mirror support (airgap operation) is available via
        // `builder.with_local_mirror(url)` when a package mirror is configured.

        // Add users from manifest (not hardcoded!)
        if self.manifest.users.is_empty() {
            // Fallback: create default user if none specified
            warn!("No users defined in manifest, creating default 'builder' user");
            builder = builder.add_user("builder", ssh_public_key);
        } else {
            for user in &self.manifest.users {
                builder = builder.add_user(&user.name, ssh_public_key.clone());
            }
        }

        // SUDO-FREE EVOLUTION: Extract packages from post_boot_steps and install via cloud-init
        // This moves standard apt packages from post-boot (sudo) to cloud-init (native root)
        let cloud_init_packages = self.extract_cloud_init_packages();
        if !cloud_init_packages.is_empty() {
            info!(
                "Adding {} packages to cloud-init for sudo-free installation",
                cloud_init_packages.len()
            );
            builder = builder.packages(cloud_init_packages);
        }

        // Process build steps from manifest using enum pattern matching (idiomatic Rust)
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
                    let local_file = find_local_package(url);

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

        builder.build()
    }

    /// Create cloud-init configuration for COSMIC desktop using benchScale builder
    /// OLD: Create cloud-init YAML string (deprecated)
    fn _create_cosmic_cloud_init_yaml_deprecated(ssh_public_key: &str) -> String {
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

        cloud_init
    }
}

/// Find local package file for injection instead of downloading
///
/// Looks in packages/ and debs/ directories for matching files.
/// Returns the path if found, None otherwise.
fn find_local_package(url: &str) -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::super::ImageBuilder;
    use crate::templates::{
        PostBootStep, ResourceConfig, TemplateManifest, UserConfig, VerificationConfig,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn base_manifest() -> TemplateManifest {
        TemplateManifest {
            name: "t".to_string(),
            version: "1.0.0".to_string(),
            base_image: "base.img".to_string(),
            description: None,
            resources: ResourceConfig {
                memory_mb: 2048,
                vcpus: 2,
                disk_gb: 30,
                timeout_secs: 2400,
                static_ip: None,
            },
            pci_passthrough: vec![],
            users: vec![UserConfig {
                name: "u".to_string(),
                password: None,
                groups: vec![],
                ssh_authorized_keys: vec![],
            }],
            build_steps: vec![],
            post_boot_steps: vec![],
            verification: VerificationConfig {
                required_packages: vec![],
                required_services: vec![],
                required_files: vec![],
                verification_commands: vec![],
            },
            metadata: HashMap::default(),
            created: None,
            checksum: None,
        }
    }

    #[test]
    fn is_standard_apt_package_only_infrastructure() {
        assert!(ImageBuilder::is_standard_apt_package("curl"));
        assert!(ImageBuilder::is_standard_apt_package("CURL"));
        assert!(!ImageBuilder::is_standard_apt_package("firefox"));
    }

    #[test]
    fn extract_cloud_init_packages_from_post_boot() {
        let mut m = base_manifest();
        m.post_boot_steps = vec![
            PostBootStep::InstallPackages {
                packages: vec!["curl".to_string(), "ubuntu-desktop".to_string()],
                retry: false,
                timeout_secs: 100,
                description: None,
            },
            PostBootStep::RunCommand {
                command: "true".to_string(),
                description: None,
                timeout_secs: 1,
            },
        ];
        let b = ImageBuilder::from_manifest(m);
        let pkgs = b.extract_cloud_init_packages();
        assert_eq!(pkgs, vec!["curl".to_string()]);
    }

    #[test]
    fn filter_post_boot_removes_standard_apt_packages() {
        let mut m = base_manifest();
        m.post_boot_steps = vec![PostBootStep::InstallPackages {
            packages: vec!["wget".to_string(), "vim".to_string()],
            retry: true,
            timeout_secs: 50,
            description: Some("d".to_string()),
        }];
        let b = ImageBuilder::from_manifest(m);
        let filtered = b.filter_post_boot_steps();
        assert_eq!(filtered.len(), 1);
        match &filtered[0] {
            PostBootStep::InstallPackages { packages, .. } => {
                assert_eq!(packages, &vec!["vim".to_string()]);
            }
            _ => panic!("expected InstallPackages"),
        }
    }

    #[test]
    fn find_local_package_returns_none_when_not_present() {
        assert!(super::find_local_package("https://example.com/missing.deb").is_none());
    }

    #[test]
    fn find_local_package_resolves_under_packages_dir() {
        let _lock = CWD_LOCK.lock().expect("lock");
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let old = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(tmp.path()).expect("chdir");
        std::fs::create_dir_all("packages").expect("mkdir");
        std::fs::write("packages/foo.deb", b"x").expect("write");

        let url = "https://example.com/foo.deb";
        let p = super::find_local_package(url).expect("local");
        assert!(p.ends_with("packages/foo.deb"));

        std::env::set_current_dir(old).expect("restore cwd");
    }

    #[test]
    fn deprecated_cosmic_yaml_template_includes_key_and_cosmic() {
        let y = ImageBuilder::_create_cosmic_cloud_init_yaml_deprecated("ssh-rsa AAAABASE64");
        assert!(y.contains("#cloud-config"));
        assert!(y.contains("ssh-rsa AAAABASE64"));
        assert!(y.contains("COSMIC"));
        assert!(y.contains("power_state:"));
    }
}
