// SPDX-License-Identifier: AGPL-3.0-or-later
//! Template manifest definitions
//!
//! Defines the structure for template manifests that describe
//! how to build VM templates in a reproducible way.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Template manifest - describes how to build a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateManifest {
    /// Template name
    pub name: String,

    /// Template version (semver)
    pub version: String,

    /// Base image to build from
    pub base_image: String,

    /// Description
    pub description: Option<String>,

    /// VM resources
    pub resources: ResourceConfig,

    /// PCI devices to pass through via VFIO (optional)
    /// Each device must be bound to vfio-pci on the host before VM creation.
    #[serde(default)]
    pub pci_passthrough: Vec<PciPassthroughConfig>,

    /// Users to create (for cloud-init)
    #[serde(default)]
    pub users: Vec<UserConfig>,

    /// Build steps (cloud-init only - keep minimal!)
    pub build_steps: Vec<BuildStep>,

    /// Post-boot steps (executed via SSH after cloud-init completes)
    /// This is where desktop packages, complex software, and configuration should go
    #[serde(default)]
    pub post_boot_steps: Vec<PostBootStep>,

    /// Verification steps
    pub verification: VerificationConfig,

    /// Metadata (optional)
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Created timestamp (filled by builder)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,

    /// SHA256 checksum (filled by builder)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// VM resource configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Memory in MB
    pub memory_mb: usize,

    /// Number of vCPUs
    pub vcpus: usize,

    /// Disk size in GB
    pub disk_gb: usize,

    /// Build timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Static IP address (optional, e.g., "192.168.122.20")
    /// If not provided, benchScale will allocate from IP pool
    #[serde(default)]
    pub static_ip: Option<String>,
}

/// PCI device for VFIO passthrough into the VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciPassthroughConfig {
    /// PCI bus/device/function (e.g., "0000:4d:00.0")
    pub bdf: String,

    /// Prevent Function Level Reset on VM shutdown so GPU hardware
    /// state (WPR, HBM2 training, falcon firmware) survives the
    /// VM-to-host transition. Only useful for reagent-capture flows.
    #[serde(default)]
    pub no_flr: bool,
}

const fn default_timeout() -> u64 {
    2400 // 40 minutes
}

/// User configuration for cloud-init
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Username
    pub name: String,

    /// Password (optional, will be hashed by cloud-init)
    pub password: Option<String>,

    /// Groups (optional)
    #[serde(default)]
    pub groups: Vec<String>,

    /// SSH authorized keys (optional, can be overridden at build time)
    #[serde(default)]
    pub ssh_authorized_keys: Vec<String>,
}

/// Build step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildStep {
    /// Wait for cloud-init to complete
    WaitCloudInit {
        /// Max seconds to wait before failing the step.
        #[serde(default = "default_cloud_init_timeout")]
        timeout_secs: u64,
    },

    /// Add APT repository
    AddRepository {
        /// Short name for `sources.list.d` and keyring files.
        name: String,
        /// `deb` line URL (without `deb [arch=...]` prefix if using signed-by).
        url: String,
        /// Optional URL to the repository signing key.
        key_url: Option<String>,
    },

    /// Install packages
    InstallPackages {
        /// APT package names to install during cloud-init.
        packages: Vec<String>,
    },

    /// Run shell command
    RunCommand {
        /// Shell command to run as root during cloud-init.
        command: String,
        /// Optional log label for operators.
        description: Option<String>,
    },

    /// Enable systemd service
    EnableService {
        /// systemd unit name (e.g. `ssh`).
        service: String,
    },

    /// Create file with content
    CreateFile {
        /// Absolute path on the guest.
        path: String,
        /// File contents (written via heredoc in generated cloud-init).
        content: String,
        /// Optional octal mode string.
        mode: Option<String>,
    },

    /// Download file
    DownloadFile {
        /// HTTP(S) URL to fetch.
        url: String,
        /// Destination path on the guest.
        dest: String,
    },

    /// Reboot VM
    Reboot {
        /// Seconds to wait after requesting reboot before continuing.
        #[serde(default = "default_reboot_wait")]
        wait_secs: u64,
    },
}

const fn default_cloud_init_timeout() -> u64 {
    600 // 10 minutes
}

const fn default_reboot_wait() -> u64 {
    120 // 2 minutes
}

/// Post-boot step definition
///
/// These steps are executed via SSH AFTER cloud-init completes.
/// This is the proper way to handle "heat-sensitive" operations like:
/// - Desktop environment installation
/// - Complex package dependencies
/// - Application installations
/// - System configuration that conflicts with cloud-init
///
/// Think of cloud-init as "autoclaving" and post-boot as "adding compounds after cooling"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PostBootStep {
    /// Install packages via APT (with retry and timeout)
    InstallPackages {
        /// APT package names to install over SSH.
        packages: Vec<String>,
        /// Whether to retry the apt invocation on transient errors.
        #[serde(default)]
        retry: bool,
        /// SSH command timeout in seconds.
        #[serde(default = "default_package_timeout")]
        timeout_secs: u64,
        /// Optional operator-facing description.
        #[serde(default)]
        description: Option<String>,
    },

    /// Run a shell command
    RunCommand {
        /// Shell command to run (typically as the manifest user with sudo).
        command: String,
        /// Optional log label.
        #[serde(default)]
        description: Option<String>,
        /// Timeout for the remote command in seconds.
        #[serde(default)]
        timeout_secs: u64,
    },

    /// Create a file with content
    CreateFile {
        /// Absolute path on the guest.
        path: String,
        /// File contents.
        content: String,
        /// Octal permission string (e.g. `0644`).
        #[serde(default = "default_file_mode")]
        mode: String,
        /// Optional `user:group` for ownership.
        #[serde(default)]
        owner: Option<String>,
    },

    /// Copy a file from host to VM
    CopyFile {
        /// Host path relative to the builder working directory or absolute.
        source: String,
        /// Destination path on the guest.
        destination: String,
        /// Octal permission string for the copied file.
        #[serde(default = "default_file_mode")]
        mode: String,
    },

    /// Fetch a file or directory from VM to host (artifact extraction).
    ///
    /// Uses SCP to pull `remote_path` on the guest to `local_path` on the
    /// host.  Set `recursive` to true for directories.
    FetchFile {
        /// Absolute path on the guest.
        remote_path: String,
        /// Destination path on the host (absolute or relative to builder CWD).
        local_path: String,
        /// Whether to copy recursively (for directories).
        #[serde(default)]
        recursive: bool,
    },

    /// Enable a systemd service
    EnableService {
        /// systemd unit name.
        service: String,
        /// Whether to start the unit immediately after enable.
        #[serde(default)]
        start: bool,
    },

    /// Reboot the VM and wait for it to come back
    Reboot {
        /// Seconds to wait for SSH to return after reboot.
        #[serde(default = "default_reboot_wait")]
        wait_secs: u64,
    },
}

const fn default_package_timeout() -> u64 {
    1800 // 30 minutes for complex packages
}

fn default_file_mode() -> String {
    "0644".to_string()
}

/// Verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Packages that must be installed
    #[serde(default)]
    pub required_packages: Vec<String>,

    /// Services that must be enabled
    #[serde(default)]
    pub required_services: Vec<String>,

    /// Files that must exist
    #[serde(default)]
    pub required_files: Vec<String>,

    /// Commands to run for verification
    #[serde(default)]
    pub verification_commands: Vec<VerificationCommand>,
}

/// Verification command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCommand {
    /// Command to run
    pub command: String,

    /// Expected exit code (default: 0)
    #[serde(default)]
    pub expected_exit_code: i32,

    /// Description
    pub description: Option<String>,
}

impl TemplateManifest {
    /// Load manifest from YAML file
    pub fn from_yaml_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = serde_yaml::from_str(&content)?;
        Ok(manifest)
    }

    /// Save manifest to YAML file
    pub fn to_yaml_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }

    /// Validate manifest
    pub fn validate(&self) -> anyhow::Result<()> {
        // Check name
        if self.name.is_empty() {
            anyhow::bail!("Template name cannot be empty");
        }

        // Check version (basic semver check)
        if !self.version.contains('.') {
            anyhow::bail!("Version must be in semver format (e.g., 1.0.0)");
        }

        // Check base image
        if self.base_image.is_empty() {
            anyhow::bail!("Base image cannot be empty");
        }

        // Check resources
        if self.resources.memory_mb < 512 {
            anyhow::bail!("Memory must be at least 512 MB");
        }

        if self.resources.vcpus < 1 {
            anyhow::bail!("Must have at least 1 vCPU");
        }

        if self.resources.disk_gb < 10 {
            anyhow::bail!("Disk size must be at least 10 GB");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_validation() {
        let mut manifest = TemplateManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            base_image: "ubuntu-24.04.img".to_string(),
            description: None,
            resources: ResourceConfig {
                memory_mb: 2048,
                vcpus: 2,
                disk_gb: 30,
                timeout_secs: 2400,
                static_ip: None,
            },
            pci_passthrough: vec![],
            users: vec![],
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
        };

        assert!(manifest.validate().is_ok());

        // Test invalid memory
        manifest.resources.memory_mb = 256;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_name_version_base_and_resources() {
        let mut manifest = TemplateManifest {
            name: String::new(),
            version: "1.0.0".to_string(),
            base_image: "x.img".to_string(),
            description: None,
            resources: ResourceConfig {
                memory_mb: 2048,
                vcpus: 2,
                disk_gb: 30,
                timeout_secs: 2400,
                static_ip: None,
            },
            pci_passthrough: vec![],
            users: vec![],
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
        };
        assert!(manifest.validate().is_err());

        manifest.name = "n".to_string();
        manifest.version = "bad".to_string();
        assert!(manifest.validate().is_err());

        manifest.version = "1.0.0".to_string();
        manifest.base_image = String::new();
        assert!(manifest.validate().is_err());

        manifest.base_image = "b.img".to_string();
        manifest.resources.vcpus = 0;
        assert!(manifest.validate().is_err());

        manifest.resources.vcpus = 1;
        manifest.resources.disk_gb = 5;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn yaml_roundtrip_minimal_manifest() {
        let manifest = TemplateManifest {
            name: "t".to_string(),
            version: "2.1.0".to_string(),
            base_image: "ubuntu.img".to_string(),
            description: Some("d".to_string()),
            resources: ResourceConfig {
                memory_mb: 1024,
                vcpus: 1,
                disk_gb: 20,
                timeout_secs: 100,
                static_ip: Some("10.0.0.5".to_string()),
            },
            pci_passthrough: vec![],
            users: vec![UserConfig {
                name: "u".to_string(),
                password: None,
                groups: vec!["sudo".to_string()],
                ssh_authorized_keys: vec![],
            }],
            build_steps: vec![BuildStep::WaitCloudInit { timeout_secs: 120 }],
            post_boot_steps: vec![PostBootStep::RunCommand {
                command: "true".to_string(),
                description: None,
                timeout_secs: 10,
            }],
            verification: VerificationConfig {
                required_packages: vec!["curl".to_string()],
                required_services: vec![],
                required_files: vec![],
                verification_commands: vec![VerificationCommand {
                    command: "true".to_string(),
                    expected_exit_code: 0,
                    description: None,
                }],
            },
            metadata: HashMap::from([("k".to_string(), "v".to_string())]),
            created: None,
            checksum: None,
        };
        let yaml = serde_yaml::to_string(&manifest).expect("to yaml");
        let back: TemplateManifest = serde_yaml::from_str(&yaml).expect("from yaml");
        assert_eq!(back.name, manifest.name);
        assert_eq!(back.resources.static_ip, manifest.resources.static_ip);
        assert_eq!(back.build_steps.len(), 1);
        assert_eq!(back.post_boot_steps.len(), 1);
    }
}
