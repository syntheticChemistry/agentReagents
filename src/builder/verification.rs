//! VM verification system for agentReagents
//!
//! This module provides comprehensive verification of VM builds,
//! ensuring that all packages, services, and configurations are
//! correctly applied.

use crate::builder::vm_handle::VmHandle;
use crate::templates::TemplateManifest as Manifest;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Result of a verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    /// Name of the check
    pub name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Optional details or error message
    pub details: Option<String>,
}

/// Overall verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether all checks passed
    pub passed: bool,
    /// Individual check results
    pub checks: Vec<VerificationCheck>,
    /// Total checks performed
    pub total: usize,
    /// Number of passed checks
    pub passed_count: usize,
    /// Number of failed checks
    pub failed_count: usize,
}

impl VerificationResult {
    /// Create a new verification result from checks
    pub fn from_checks(checks: Vec<VerificationCheck>) -> Self {
        let total = checks.len();
        let passed_count = checks.iter().filter(|c| c.passed).count();
        let failed_count = total - passed_count;
        let passed = failed_count == 0;

        Self {
            passed,
            checks,
            total,
            passed_count,
            failed_count,
        }
    }

    /// Get failed checks
    pub fn failed_checks(&self) -> Vec<&VerificationCheck> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        if self.passed {
            format!("✅ All {} checks passed", self.total)
        } else {
            format!(
                "❌ {}/{} checks passed ({} failed)",
                self.passed_count, self.total, self.failed_count
            )
        }
    }
}

/// Verify VM installation against manifest
pub async fn verify_installation(vm: &VmHandle, manifest: &Manifest) -> Result<VerificationResult> {
    info!("Starting verification for VM: {}", vm.name());
    let mut checks = Vec::new();

    // 1. Verify packages are installed
    checks.extend(verify_packages(vm, manifest).await?);

    // 2. Verify commands executed successfully (check for expected files/state)
    checks.extend(verify_commands(vm, manifest).await?);

    // 3. Verify system is accessible and responsive
    checks.extend(verify_system_health(vm).await?);

    let result = VerificationResult::from_checks(checks);

    if result.passed {
        info!(
            "✅ Verification passed: {}/{} checks",
            result.passed_count, result.total
        );
    } else {
        warn!(
            "❌ Verification failed: {}/{} checks passed",
            result.passed_count, result.total
        );
        for check in result.failed_checks() {
            warn!("  Failed: {} - {:?}", check.name, check.details);
        }
    }

    Ok(result)
}

/// Verify packages are installed
async fn verify_packages(vm: &VmHandle, manifest: &Manifest) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    // Get packages from build steps
    let packages: Vec<String> = manifest
        .build_steps
        .iter()
        .filter_map(|step| {
            if let crate::templates::BuildStep::InstallPackages { packages } = step {
                Some(packages.clone())
            } else {
                None
            }
        })
        .flatten()
        .collect();

    debug!("Verifying {} packages", packages.len());

    for package in &packages {
        let check_name = format!("Package: {}", package);

        // Use dpkg to check if package is installed
        let result = vm
            .ssh_exec(
                "ubuntu",
                &format!("dpkg -l {} 2>/dev/null | grep -q '^ii'", package),
            )
            .await;

        let (passed, details) = match result {
            Ok(_) => (true, None),
            Err(e) => (false, Some(format!("Not installed or not found: {}", e))),
        };

        checks.push(VerificationCheck {
            name: check_name,
            passed,
            details,
        });
    }

    Ok(checks)
}

/// Verify commands executed successfully
async fn verify_commands(vm: &VmHandle, manifest: &Manifest) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying command execution results");

    // Check for common indicators of successful setup
    let indicators = vec![
        (
            "Cloud-init complete",
            "test -f /var/lib/cloud/instance/boot-finished",
        ),
        ("User home exists", "test -d /home/ubuntu"),
        (
            "SSH authorized_keys",
            "test -f /home/ubuntu/.ssh/authorized_keys",
        ),
    ];

    for (name, command) in indicators {
        let result = vm.ssh_exec("ubuntu", command).await;

        let (passed, details) = match result {
            Ok(_) => (true, None),
            Err(e) => (false, Some(format!("Check failed: {}", e))),
        };

        checks.push(VerificationCheck {
            name: name.to_string(),
            passed,
            details,
        });
    }

    // If manifest name specifies a desktop, verify display-related packages
    if manifest.name.to_lowercase().contains("desktop")
        || manifest.name.to_lowercase().contains("cosmic")
    {
        let desktop_check = vm
            .ssh_exec(
                "ubuntu",
                "dpkg -l | grep -E '(xorg|wayland|cosmic)' | wc -l",
            )
            .await;

        let (passed, details) = match desktop_check {
            Ok(output) => {
                let count: usize = output.trim().parse().unwrap_or(0);
                if count > 0 {
                    (true, Some(format!("{} desktop packages found", count)))
                } else {
                    (false, Some("No desktop packages found".to_string()))
                }
            }
            Err(e) => (false, Some(format!("Desktop check failed: {}", e))),
        };

        checks.push(VerificationCheck {
            name: "Desktop Environment".to_string(),
            passed,
            details,
        });
    }

    Ok(checks)
}

/// Verify system health and responsiveness
async fn verify_system_health(vm: &VmHandle) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying system health");

    // Check system is responsive
    let uptime_result = vm.ssh_exec("ubuntu", "uptime").await;
    checks.push(VerificationCheck {
        name: "System responsive".to_string(),
        passed: uptime_result.is_ok(),
        details: uptime_result.err().map(|e| e.to_string()),
    });

    // Check disk space
    let df_result = vm
        .ssh_exec(
            "ubuntu",
            "df -h / | tail -1 | awk '{print $5}' | sed 's/%//'",
        )
        .await;

    let (passed, details) = match df_result {
        Ok(output) => {
            let usage: u32 = output.trim().parse().unwrap_or(100);
            if usage < 90 {
                (true, Some(format!("Disk usage: {}%", usage)))
            } else {
                (false, Some(format!("Disk usage too high: {}%", usage)))
            }
        }
        Err(e) => (false, Some(format!("Disk check failed: {}", e))),
    };

    checks.push(VerificationCheck {
        name: "Disk space".to_string(),
        passed,
        details,
    });

    // Check memory
    let mem_result = vm
        .ssh_exec("ubuntu", "free -m | grep Mem | awk '{print $3/$2 * 100.0}'")
        .await;

    let (passed, details) = match mem_result {
        Ok(output) => {
            let usage: f32 = output.trim().parse().unwrap_or(100.0);
            if usage < 95.0 {
                (true, Some(format!("Memory usage: {:.1}%", usage)))
            } else {
                (false, Some(format!("Memory usage too high: {:.1}%", usage)))
            }
        }
        Err(e) => (false, Some(format!("Memory check failed: {}", e))),
    };

    checks.push(VerificationCheck {
        name: "Memory available".to_string(),
        passed,
        details,
    });

    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_result_from_checks() {
        let checks = vec![
            VerificationCheck {
                name: "Test 1".to_string(),
                passed: true,
                details: None,
            },
            VerificationCheck {
                name: "Test 2".to_string(),
                passed: false,
                details: Some("Failed".to_string()),
            },
            VerificationCheck {
                name: "Test 3".to_string(),
                passed: true,
                details: None,
            },
        ];

        let result = VerificationResult::from_checks(checks);

        assert_eq!(result.total, 3);
        assert_eq!(result.passed_count, 2);
        assert_eq!(result.failed_count, 1);
        assert!(!result.passed);
        assert_eq!(result.failed_checks().len(), 1);
    }

    #[test]
    fn test_verification_result_all_passed() {
        let checks = vec![
            VerificationCheck {
                name: "Test 1".to_string(),
                passed: true,
                details: None,
            },
            VerificationCheck {
                name: "Test 2".to_string(),
                passed: true,
                details: None,
            },
        ];

        let result = VerificationResult::from_checks(checks);

        assert_eq!(result.total, 2);
        assert_eq!(result.passed_count, 2);
        assert_eq!(result.failed_count, 0);
        assert!(result.passed);
        assert_eq!(result.failed_checks().len(), 0);
    }
}
