// SPDX-License-Identifier: AGPL-3.0-only
//! VM verification system for agentReagents
//!
//! This module provides comprehensive verification of VM builds,
//! ensuring that all packages, services, and configurations are
//! correctly applied.
//!
//! **Evolution #23: Robust Package Verification**
//! Multi-method verification with detailed diagnostics to eliminate false negatives.

use crate::builder::vm_handle::VmHandle;
use crate::templates::TemplateManifest as Manifest;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
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

/// Evolution #23: Enhanced package verification result
#[derive(Debug, Clone)]
pub struct PackageVerificationResult {
    /// Package name queried
    pub package: String,
    /// Whether package is installed
    pub installed: bool,
    /// Method that succeeded (or last method tried)
    pub method: VerificationMethod,
    /// Detailed diagnostics
    pub details: PackageDetails,
}

/// Methods used for package verification
#[derive(Debug, Clone)]
pub enum VerificationMethod {
    /// dpkg-query (most reliable)
    DpkgQuery,
    /// dpkg -l with output parsing
    DpkgList,
    /// apt-cache policy
    AptCache,
    /// Found as dependency of another package
    InstalledByDependency(String),
    /// All methods failed
    AllFailed,
}

/// Detailed package information
#[derive(Debug, Clone, Default)]
pub struct PackageDetails {
    /// Actual package name (may differ from queried)
    pub actual_name: Option<String>,
    /// Package version
    pub version: Option<String>,
    /// Installation status from dpkg
    pub dpkg_status: Option<String>,
    /// Packages that require this one
    pub required_by: Vec<String>,
    /// Alternative package names that were checked
    pub alternatives_checked: Vec<String>,
    /// Raw diagnostic output (for debugging)
    pub raw_output: Option<String>,
}

/// Safely escape a string for use in shell commands
///
/// Wraps the string in single quotes and escapes any existing single quotes.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Evolution #23: Robust package verification with multiple methods
///
/// This function tries multiple verification methods in sequence:
/// 1. dpkg-query (most reliable, gives structured output)
/// 2. dpkg -l (standard method, good for most packages)
/// 3. apt-cache policy (shows apt perspective)
/// 4. Check if installed as dependency (for virtual/meta packages)
///
/// If all methods fail, comprehensive diagnostics are gathered.
async fn verify_package_robust(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    debug!("Robust verification for package: {}", package);
    
    // Method 1: dpkg-query (most reliable)
    if let Ok(result) = check_dpkg_query(vm, username, package).await {
        debug!("Package {} verified via dpkg-query", package);
        return Ok(result);
    }
    
    // Method 2: dpkg -l with output capture
    if let Ok(result) = check_dpkg_list(vm, username, package).await {
        debug!("Package {} verified via dpkg -l", package);
        return Ok(result);
    }
    
    // Method 3: apt-cache policy
    if let Ok(result) = check_apt_cache(vm, username, package).await {
        debug!("Package {} verified via apt-cache", package);
        return Ok(result);
    }
    
    // Method 4: Check if installed by dependency
    if let Ok(result) = check_installed_by_dependency(vm, username, package).await {
        debug!("Package {} found as dependency", package);
        return Ok(result);
    }
    
    // All methods failed - gather comprehensive diagnostics
    warn!("All verification methods failed for package: {}", package);
    let details = gather_diagnostics(vm, username, package).await?;
    
    Ok(PackageVerificationResult {
        package: package.to_string(),
        installed: false,
        method: VerificationMethod::AllFailed,
        details,
    })
}

/// Method 1: Verify using dpkg-query (most reliable)
async fn check_dpkg_query(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    // Try with exact name first
    let cmd = format!(
        "dpkg-query -W -f='${{Status}}|${{Version}}|${{Package}}' {} 2>&1",
        shell_escape(package)
    );
    
    let output = vm.ssh_exec(username, &cmd).await?;
    
    // Parse output: "install ok installed|1.2.3|package-name"
    if output.contains("install ok installed") {
        let parts: Vec<&str> = output.split('|').collect();
        return Ok(PackageVerificationResult {
            package: package.to_string(),
            installed: true,
            method: VerificationMethod::DpkgQuery,
            details: PackageDetails {
                actual_name: parts.get(2).map(|s| s.trim().to_string()),
                version: parts.get(1).map(|s| s.trim().to_string()),
                dpkg_status: Some(parts.first().map_or("", |s| s.trim()).to_string()),
                ..Default::default()
            },
        });
    }
    
    // If failed, try with :amd64 suffix (common architecture suffix)
    if !package.contains(':') {
        let arch_package = format!("{}:amd64", package);
        let cmd = format!(
            "dpkg-query -W -f='${{Status}}|${{Version}}|${{Package}}' {} 2>&1",
            shell_escape(&arch_package)
        );
        
        if let Ok(output) = vm.ssh_exec(username, &cmd).await
            && output.contains("install ok installed") {
                let parts: Vec<&str> = output.split('|').collect();
                return Ok(PackageVerificationResult {
                    package: package.to_string(),
                    installed: true,
                    method: VerificationMethod::DpkgQuery,
                    details: PackageDetails {
                        actual_name: Some(arch_package),
                        version: parts.get(1).map(|s| s.trim().to_string()),
                        dpkg_status: Some(parts.first().map_or("", |s| s.trim()).to_string()),
                        ..Default::default()
                    },
                });
            }
    }
    
    Err(anyhow!("Package not in 'install ok installed' state"))
}

/// Method 2: Verify using dpkg -l with output parsing
async fn check_dpkg_list(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    // Try exact name first
    let cmd = format!("dpkg -l {} 2>&1", shell_escape(package));
    let output = vm.ssh_exec(username, &cmd).await?;
    
    // Parse dpkg -l output - look for lines starting with "ii"
    for line in output.lines() {
        if line.starts_with("ii") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                // Check if the package name matches (may have architecture suffix)
                let pkg_name = parts[1];
                if pkg_name == package || pkg_name.starts_with(&format!("{}:", package)) {
                    return Ok(PackageVerificationResult {
                        package: package.to_string(),
                        installed: true,
                        method: VerificationMethod::DpkgList,
                        details: PackageDetails {
                            actual_name: Some(pkg_name.to_string()),
                            version: Some(parts[2].to_string()),
                            dpkg_status: Some("ii".to_string()),
                            raw_output: Some(line.to_string()),
                            ..Default::default()
                        },
                    });
                }
            }
        }
    }
    
    // If not found with exact name, try with wildcard to catch architecture suffixes
    if !package.contains(':') && !package.contains('*') {
        let wildcard_cmd = format!("dpkg -l '{}:*' 2>&1", shell_escape(package));
        if let Ok(wildcard_output) = vm.ssh_exec(username, &wildcard_cmd).await {
            for line in wildcard_output.lines() {
                if line.starts_with("ii") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        return Ok(PackageVerificationResult {
                            package: package.to_string(),
                            installed: true,
                            method: VerificationMethod::DpkgList,
                            details: PackageDetails {
                                actual_name: Some(parts[1].to_string()),
                                version: Some(parts[2].to_string()),
                                dpkg_status: Some("ii".to_string()),
                                raw_output: Some(line.to_string()),
                                ..Default::default()
                            },
                        });
                    }
                }
            }
        }
    }
    
    Err(anyhow!("No 'ii' status line found in dpkg -l output"))
}

/// Method 3: Verify using apt-cache policy
async fn check_apt_cache(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    let cmd = format!("apt-cache policy {} 2>&1", shell_escape(package));
    let output = vm.ssh_exec(username, &cmd).await?;
    
    // Look for "Installed: <version>" line
    for line in output.lines() {
        if line.trim().starts_with("Installed:") {
            let version = line.split(':').nth(1).map(str::trim);
            if version != Some("(none)") && version.is_some() {
                return Ok(PackageVerificationResult {
                    package: package.to_string(),
                    installed: true,
                    method: VerificationMethod::AptCache,
                    details: PackageDetails {
                        actual_name: Some(package.to_string()),
                        version: version.map(std::string::ToString::to_string),
                        raw_output: Some(output),
                        ..Default::default()
                    },
                });
            }
        }
    }
    
    Err(anyhow!("apt-cache shows package as not installed"))
}

/// Method 4: Check if package is installed via dependency
///
/// This is particularly useful for virtual packages or packages that
/// are installed as dependencies but might not match the exact name queried.
async fn check_installed_by_dependency(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    // Find packages that depend on this package
    let cmd = format!(
        "apt-cache rdepends --installed {} 2>&1 | tail -n +3 | head -20",
        shell_escape(package)
    );
    
    let output = vm.ssh_exec(username, &cmd).await?;
    
    let dependents: Vec<String> = output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('|'))
        .collect();
    
    if dependents.is_empty() {
        return Err(anyhow!("No installed reverse dependencies found"));
    }
    
    // Check if any of the dependents are actually installed
    for dependent in &dependents {
        // Quick check using dpkg-query
        let check_cmd = format!("dpkg-query -W {} 2>/dev/null", shell_escape(dependent));
        if vm.ssh_exec(username, &check_cmd).await.is_ok() {
            debug!("Found package {} installed as dependency of {}", package, dependent);
            return Ok(PackageVerificationResult {
                package: package.to_string(),
                installed: true,
                method: VerificationMethod::InstalledByDependency(dependent.clone()),
                details: PackageDetails {
                    actual_name: Some(package.to_string()),
                    dpkg_status: Some("installed-by-dependency".to_string()),
                    required_by: vec![dependent.clone()],
                    raw_output: Some(format!("Required by: {}", dependent)),
                    ..Default::default()
                },
            });
        }
    }
    
    Err(anyhow!("No installed dependents found"))
}

/// Gather comprehensive diagnostics for failed verification
async fn gather_diagnostics(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageDetails> {
    let mut alternatives = Vec::new();
    let mut raw_output = String::new();
    
    // Try common package name variants (including architecture suffixes)
    let mut variants = vec![
        package.replace('-', "_"),
        format!("{}-dev", package),
        format!("lib{}", package),
    ];
    
    // Add architecture suffix if not already present
    if !package.contains(':') {
        variants.insert(0, format!("{}:amd64", package));
        variants.push(format!("{}:all", package));
    }
    
    for variant in &variants {
        if variant != package {
            alternatives.push(variant.clone());
            
            // Quick check if variant exists
            let cmd = format!("dpkg-query -W {} 2>&1", shell_escape(variant));
            if let Ok(output) = vm.ssh_exec(username, &cmd).await {
                let first_line = output.lines().next().unwrap_or("");
                let _ = writeln!(raw_output, "Variant {}: {}", variant, first_line);
                
                if first_line.contains(&format!("{}\t", variant)) || output.contains("install ok installed") {
                    let _ = writeln!(raw_output, "✓ Found as variant: {}", variant);
                }
            }
        }
    }
    
    // Try a wildcard search to see what's actually installed
    let wildcard_cmd = format!("dpkg-query -W '{}*' 2>&1 | head -5", shell_escape(package));
    if let Ok(wildcard_output) = vm.ssh_exec(username, &wildcard_cmd).await
        && !wildcard_output.is_empty() && !wildcard_output.contains("no packages found") {
            let _ = write!(
                raw_output,
                "\nInstalled packages matching {}*:\n{}\n",
                package,
                wildcard_output
            );
        }
    
    // Get general package search results
    let search_cmd = format!("apt-cache search {} | head -5 2>&1", shell_escape(package));
    if let Ok(search_output) = vm.ssh_exec(username, &search_cmd).await
        && !search_output.is_empty() {
            let _ = write!(raw_output, "\nSimilar packages:\n{}\n", search_output);
        }
    
    Ok(PackageDetails {
        actual_name: None,
        version: None,
        dpkg_status: None,
        required_by: Vec::new(),
        alternatives_checked: alternatives,
        raw_output: Some(raw_output),
    })
}

/// Verify VM installation against manifest
pub async fn verify_installation(vm: &VmHandle, manifest: &Manifest) -> Result<VerificationResult> {
    info!("Starting verification for VM: {}", vm.name());
    let mut checks = Vec::new();

    // Get the username from manifest (first user, or default to "ubuntu")
    let username = manifest
        .users
        .first()
        .map_or("ubuntu", |u| u.name.as_str());
    
    info!("Using SSH user: {}", username);

    // 1. Verify packages are installed
    checks.extend(verify_packages(vm, manifest, username).await?);

    // 2. Verify commands executed successfully (check for expected files/state)
    checks.extend(verify_commands(vm, manifest, username).await?);

    // 3. Verify system is accessible and responsive
    checks.extend(verify_system_health(vm, username).await?);

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
///
/// **Evolution #23**: Uses robust multi-method verification to eliminate false negatives
async fn verify_packages(vm: &VmHandle, manifest: &Manifest, username: &str) -> Result<Vec<VerificationCheck>> {
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

    info!(
        "Verifying {} packages with robust multi-method verification",
        packages.len()
    );

    for package in &packages {
        let check_name = format!("Package: {}", package);

        // Evolution #23: Use robust verification with multiple methods
        match verify_package_robust(vm, username, package).await {
            Ok(result) => {
                let details = if result.installed {
                    // Success: Show verification method and version
                    match result.method {
                        VerificationMethod::InstalledByDependency(ref dep) => {
                            Some(format!("Installed as dependency of {}", dep))
                        }
                        _ => {
                            result.details.version.as_ref().map(|v| format!("Version: {}", v))
                        }
                    }
                } else {
                    // Failure: Show comprehensive diagnostics
                    let mut msg = String::from("Not installed. ");
                    
                    if !result.details.alternatives_checked.is_empty() {
                        let _ = write!(
                            msg,
                            "Variants checked: {}. ",
                            result.details.alternatives_checked.join(", ")
                        );
                    }
                    
                    if let Some(ref raw) = result.details.raw_output
                        && !raw.is_empty() {
                            // Include first line of diagnostics
                            if let Some(first_line) = raw.lines().next() {
                                let _ = write!(msg, "Info: {}", first_line.trim());
                            }
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
                // Verification system error (rare)
                warn!("Verification system error for {}: {}", package, e);
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

/// Verify commands executed successfully
async fn verify_commands(vm: &VmHandle, manifest: &Manifest, username: &str) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying command execution results");

    // Check for common indicators of successful setup
    let indicators = vec![
        (
            "Cloud-init complete".to_string(),
            "test -f /var/lib/cloud/instance/boot-finished".to_string(),
        ),
        ("User home exists".to_string(), format!("test -d /home/{}", username)),
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

    // If manifest name specifies a desktop, verify display-related packages
    if manifest.name.to_lowercase().contains("desktop")
        || manifest.name.to_lowercase().contains("cosmic")
    {
        let desktop_check = vm
            .ssh_exec(
                username,
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
async fn verify_system_health(vm: &VmHandle, username: &str) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();

    debug!("Verifying system health");

    // Check system is responsive
    let uptime_result = vm.ssh_exec(username, "uptime").await;
    checks.push(VerificationCheck {
        name: "System responsive".to_string(),
        passed: uptime_result.is_ok(),
        details: uptime_result.err().map(|e| e.to_string()),
    });

    // Check disk space
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

    // Check memory
    let mem_result = vm
        .ssh_exec(username, "free -m | grep Mem | awk '{print $3/$2 * 100.0}'")
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

    // Evolution #23: Tests for robust verification

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
}
