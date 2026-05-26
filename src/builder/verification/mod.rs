// SPDX-License-Identifier: AGPL-3.0-or-later
//! VM verification system for agentReagents
//!
//! Verifies that packages, services, and configurations from the manifest
//! are correctly applied to the built VM.

mod package;
mod types;

pub use types::*;

use crate::builder::vm_handle::VmHandle;
use crate::templates::TemplateManifest as Manifest;
use anyhow::Result;
use std::fmt::Write;
use tracing::{debug, info, warn};

/// Verify VM installation against manifest
pub async fn verify_installation(vm: &VmHandle, manifest: &Manifest) -> Result<VerificationResult> {
    info!("Starting verification for VM: {}", vm.name());
    let mut checks = Vec::new();

    let username = manifest
        .users
        .first()
        .map_or("ubuntu", |u| u.name.as_str());

    info!("Using SSH user: {}", username);

    checks.extend(verify_packages(vm, manifest, username).await?);
    checks.extend(verify_commands(vm, manifest, username).await?);
    checks.extend(verify_system_health(vm, username).await?);
    checks.extend(verify_manifest_requirements(vm, manifest, username).await?);

    let result = VerificationResult::from_checks(checks);

    if result.passed {
        info!(
            "Verification passed: {}/{} checks",
            result.passed_count, result.total
        );
    } else {
        warn!(
            "Verification failed: {}/{} checks passed",
            result.passed_count, result.total
        );
        for check in result.failed_checks() {
            warn!("  Failed: {} - {:?}", check.name, check.details);
        }
    }

    Ok(result)
}

/// Verify the explicit `verification:` section from the manifest YAML.
async fn verify_manifest_requirements(
    vm: &VmHandle,
    manifest: &Manifest,
    username: &str,
) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();
    let v = &manifest.verification;

    for pkg in &v.required_packages {
        let check_name = format!("Required package: {}", pkg);
        match package::verify_package_robust(vm, username, pkg).await {
            Ok(result) => {
                let details = if result.installed {
                    result
                        .details
                        .version
                        .as_ref()
                        .map(|ver| format!("Version: {}", ver))
                } else {
                    Some("Not installed".to_string())
                };
                checks.push(VerificationCheck {
                    name: check_name,
                    passed: result.installed,
                    details,
                });
            }
            Err(e) => {
                checks.push(VerificationCheck {
                    name: check_name,
                    passed: false,
                    details: Some(format!("Verification error: {}", e)),
                });
            }
        }
    }

    for svc in &v.required_services {
        let check_name = format!("Required service: {}", svc);
        let cmd = format!(
            "systemctl is-enabled {} 2>&1",
            types::shell_escape(svc)
        );
        let (passed, details) = match vm.ssh_exec(username, &cmd).await {
            Ok(output) => {
                let trimmed = output.trim();
                if trimmed == "enabled" || trimmed == "static" || trimmed == "alias" {
                    (true, Some(format!("Status: {}", trimmed)))
                } else {
                    (false, Some(format!("Status: {}", trimmed)))
                }
            }
            Err(e) => (false, Some(format!("Check failed: {}", e))),
        };
        checks.push(VerificationCheck {
            name: check_name,
            passed,
            details,
        });
    }

    for file_path in &v.required_files {
        let check_name = format!("Required file: {}", file_path);
        let cmd = format!(
            "test -e {} && echo exists || echo missing",
            types::shell_escape(file_path)
        );
        let (passed, details) = match vm.ssh_exec(username, &cmd).await {
            Ok(output) => {
                let trimmed = output.trim();
                (trimmed == "exists", Some(trimmed.to_string()))
            }
            Err(e) => (false, Some(format!("Check failed: {}", e))),
        };
        checks.push(VerificationCheck {
            name: check_name,
            passed,
            details,
        });
    }

    for vcmd in &v.verification_commands {
        let check_name = vcmd
            .description
            .clone()
            .unwrap_or_else(|| format!("Command: {}", vcmd.command));
        let (passed, details) = match vm.ssh_exec(username, &vcmd.command).await {
            Ok(output) => {
                if vcmd.expected_exit_code == 0 {
                    (true, Some(output.lines().next().unwrap_or("").to_string()))
                } else {
                    (
                        false,
                        Some(format!(
                            "Expected exit code {}, got 0",
                            vcmd.expected_exit_code
                        )),
                    )
                }
            }
            Err(e) => {
                if vcmd.expected_exit_code != 0 {
                    (
                        true,
                        Some(format!(
                            "Non-zero exit (expected {})",
                            vcmd.expected_exit_code
                        )),
                    )
                } else {
                    (false, Some(format!("Command failed: {}", e)))
                }
            }
        };
        checks.push(VerificationCheck {
            name: check_name,
            passed,
            details,
        });
    }

    if !v.required_packages.is_empty()
        || !v.required_services.is_empty()
        || !v.required_files.is_empty()
        || !v.verification_commands.is_empty()
    {
        info!(
            "Manifest verification: {} packages, {} services, {} files, {} commands",
            v.required_packages.len(),
            v.required_services.len(),
            v.required_files.len(),
            v.verification_commands.len(),
        );
    }

    Ok(checks)
}

/// Verify packages from build steps using robust multi-method verification.
async fn verify_packages(
    vm: &VmHandle,
    manifest: &Manifest,
    username: &str,
) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

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

    info!(
        "Verifying {} packages with robust multi-method verification",
        packages.len()
    );

    for pkg in &packages {
        let check_name = format!("Package: {}", pkg);

        match package::verify_package_robust(vm, username, pkg).await {
            Ok(result) => {
                let details = if result.installed {
                    match result.method {
                        VerificationMethod::InstalledByDependency(ref dep) => {
                            Some(format!("Installed as dependency of {}", dep))
                        }
                        _ => result
                            .details
                            .version
                            .as_ref()
                            .map(|v| format!("Version: {}", v)),
                    }
                } else {
                    let mut msg = String::from("Not installed. ");

                    if !result.details.alternatives_checked.is_empty() {
                        let _ = write!(
                            msg,
                            "Variants checked: {}. ",
                            result.details.alternatives_checked.join(", ")
                        );
                    }

                    if let Some(ref raw) = result.details.raw_output
                        && !raw.is_empty()
                        && let Some(first_line) = raw.lines().next() {
                            let _ = write!(msg, "Info: {}", first_line.trim());
                        }

                    Some(msg)
                };

                checks.push(VerificationCheck {
                    name: check_name,
                    passed: result.installed,
                    details,
                });
            }
            Err(e) => {
                warn!("Verification system error for {}: {}", pkg, e);
                checks.push(VerificationCheck {
                    name: check_name,
                    passed: false,
                    details: Some(format!("Verification system error: {}", e)),
                });
            }
        }
    }

    Ok(checks)
}

/// Verify commands executed successfully by checking expected files/state.
async fn verify_commands(
    vm: &VmHandle,
    manifest: &Manifest,
    username: &str,
) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying command execution results");

    let indicators = vec![
        (
            "Cloud-init complete".to_string(),
            "test -f /var/lib/cloud/instance/boot-finished".to_string(),
        ),
        (
            "User home exists".to_string(),
            format!("test -d /home/{}", username),
        ),
        (
            "SSH authorized_keys".to_string(),
            format!("test -f /home/{}/.ssh/authorized_keys", username),
        ),
    ];

    for (name, command) in &indicators {
        let result = vm.ssh_exec(username, command).await;
        let (passed, details) = match result {
            Ok(_) => (true, None),
            Err(e) => (false, Some(format!("Check failed: {}", e))),
        };
        checks.push(VerificationCheck {
            name: name.clone(),
            passed,
            details,
        });
    }

    if manifest.name.to_lowercase().contains("desktop") {
        let desktop_check = vm
            .ssh_exec(
                username,
                "dpkg -l 2>/dev/null | grep -Ec '(xorg|wayland|gdm|sddm|lightdm)' || rpm -qa 2>/dev/null | grep -Ec '(xorg|wayland|gdm|sddm)' || echo 0",
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

/// Verify system health and responsiveness.
async fn verify_system_health(vm: &VmHandle, username: &str) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying system health");

    let uptime_result = vm.ssh_exec(username, "uptime").await;
    checks.push(VerificationCheck {
        name: "System responsive".to_string(),
        passed: uptime_result.is_ok(),
        details: uptime_result.err().map(|e| e.to_string()),
    });

    let df_result = vm
        .ssh_exec(
            username,
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

    let mem_result = vm
        .ssh_exec(
            username,
            "free -m | grep Mem | awk '{print $3/$2 * 100.0}'",
        )
        .await;

    let (passed, details) = match mem_result {
        Ok(output) => {
            let usage: f32 = output.trim().parse().unwrap_or(100.0);
            if usage < 95.0 {
                (true, Some(format!("Memory usage: {:.1}%", usage)))
            } else {
                (
                    false,
                    Some(format!("Memory usage too high: {:.1}%", usage)),
                )
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

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("simple"), "'simple'");
    }

    #[test]
    fn test_shell_escape_with_quote() {
        assert_eq!(shell_escape("with'quote"), "'with'\\''quote'");
    }

    #[test]
    fn test_shell_escape_with_space() {
        assert_eq!(shell_escape("with space"), "'with space'");
    }

    #[test]
    fn test_shell_escape_complex() {
        assert_eq!(
            shell_escape("it's a 'complex' case"),
            "'it'\\''s a '\\''complex'\\'' case'"
        );
    }

    #[test]
    fn test_package_verification_result_installed() {
        let result = PackageVerificationResult {
            package: "test-package".to_string(),
            installed: true,
            method: VerificationMethod::DpkgQuery,
            details: PackageDetails {
                actual_name: Some("test-package".to_string()),
                version: Some("1.2.3".to_string()),
                dpkg_status: Some("install ok installed".to_string()),
                ..Default::default()
            },
        };

        assert!(result.installed);
        assert!(matches!(result.method, VerificationMethod::DpkgQuery));
        assert_eq!(result.details.version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_package_verification_result_dependency() {
        let result = PackageVerificationResult {
            package: "libgtk-3-0".to_string(),
            installed: true,
            method: VerificationMethod::InstalledByDependency("gnome-shell".to_string()),
            details: PackageDetails {
                required_by: vec!["gnome-shell".to_string()],
                ..Default::default()
            },
        };

        assert!(result.installed);
        assert!(matches!(
            result.method,
            VerificationMethod::InstalledByDependency(_)
        ));
        assert_eq!(result.details.required_by.len(), 1);
    }

    #[test]
    fn test_package_details_default() {
        let details = PackageDetails::default();
        assert!(details.actual_name.is_none());
        assert!(details.version.is_none());
        assert!(details.dpkg_status.is_none());
        assert!(details.required_by.is_empty());
        assert!(details.alternatives_checked.is_empty());
        assert!(details.raw_output.is_none());
    }

    #[test]
    fn verification_result_empty_and_summary() {
        let empty = VerificationResult::from_checks(vec![]);
        assert_eq!(empty.total, 0);
        assert!(empty.passed);
        assert!(empty.summary().contains("0 checks"));

        let all_fail = VerificationResult::from_checks(vec![VerificationCheck {
            name: "a".to_string(),
            passed: false,
            details: Some("nope".to_string()),
        }]);
        assert!(!all_fail.passed);
        assert!(all_fail.summary().contains("failed"));
    }

    #[test]
    fn verification_method_variants_debug() {
        let m = VerificationMethod::AptCache;
        let s = format!("{m:?}");
        assert!(s.contains("AptCache"));
        let dep = VerificationMethod::InstalledByDependency("pkg".into());
        assert!(format!("{dep:?}").contains("InstalledByDependency"));
        assert!(format!("{:?}", VerificationMethod::AllFailed).contains("AllFailed"));
    }

    #[test]
    fn verification_result_serde_roundtrip() {
        let r = VerificationResult::from_checks(vec![
            VerificationCheck {
                name: "a".to_string(),
                passed: true,
                details: Some("ok".to_string()),
            },
            VerificationCheck {
                name: "b".to_string(),
                passed: false,
                details: None,
            },
        ]);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: VerificationResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.total, r.total);
        assert_eq!(back.passed_count, r.passed_count);
        assert_eq!(back.failed_count, r.failed_count);
        assert!(!back.passed);
    }

    #[test]
    fn failed_checks_returns_only_failures() {
        let r = VerificationResult::from_checks(vec![
            VerificationCheck {
                name: "ok".to_string(),
                passed: true,
                details: None,
            },
            VerificationCheck {
                name: "bad".to_string(),
                passed: false,
                details: Some("x".to_string()),
            },
        ]);
        let f = r.failed_checks();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "bad");
    }

    #[test]
    fn summary_all_passed_includes_total() {
        let r = VerificationResult::from_checks(vec![VerificationCheck {
            name: "only".to_string(),
            passed: true,
            details: None,
        }]);
        let s = r.summary();
        assert!(s.contains("All"));
        assert!(s.contains("1 checks"));
    }
}
