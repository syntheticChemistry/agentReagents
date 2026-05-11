// SPDX-License-Identifier: AGPL-3.0-or-later
//! Multi-method package verification strategies
//!
//! Tries dpkg-query, dpkg -l, apt-cache policy, and reverse-dependency
//! checks in sequence to eliminate false negatives.

use super::types::*;
use crate::builder::vm_handle::VmHandle;
use anyhow::{Result, anyhow};
use std::fmt::Write;
use tracing::{debug, warn};

/// Robust package verification with multiple fallback methods.
///
/// 1. `dpkg-query` — structured output, most reliable
/// 2. `dpkg -l` — standard method
/// 3. `apt-cache policy` — apt perspective
/// 4. Reverse-dependency check — virtual/meta packages
pub async fn verify_package_robust(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    debug!("Robust verification for package: {}", package);

    if let Ok(result) = check_dpkg_query(vm, username, package).await {
        debug!("Package {} verified via dpkg-query", package);
        return Ok(result);
    }

    if let Ok(result) = check_dpkg_list(vm, username, package).await {
        debug!("Package {} verified via dpkg -l", package);
        return Ok(result);
    }

    if let Ok(result) = check_apt_cache(vm, username, package).await {
        debug!("Package {} verified via apt-cache", package);
        return Ok(result);
    }

    if let Ok(result) = check_installed_by_dependency(vm, username, package).await {
        debug!("Package {} found as dependency", package);
        return Ok(result);
    }

    warn!("All verification methods failed for package: {}", package);
    let details = gather_diagnostics(vm, username, package).await?;

    Ok(PackageVerificationResult {
        package: package.to_string(),
        installed: false,
        method: VerificationMethod::AllFailed,
        details,
    })
}

async fn check_dpkg_query(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    let cmd = format!(
        "dpkg-query -W -f='${{Status}}|${{Version}}|${{Package}}' {} 2>&1",
        shell_escape(package)
    );

    let output = vm.ssh_exec(username, &cmd).await?;

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

    if !package.contains(':') {
        let arch_package = format!("{}:amd64", package);
        let cmd = format!(
            "dpkg-query -W -f='${{Status}}|${{Version}}|${{Package}}' {} 2>&1",
            shell_escape(&arch_package)
        );

        if let Ok(output) = vm.ssh_exec(username, &cmd).await
            && output.contains("install ok installed")
        {
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

async fn check_dpkg_list(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    let cmd = format!("dpkg -l {} 2>&1", shell_escape(package));
    let output = vm.ssh_exec(username, &cmd).await?;

    for line in output.lines() {
        if line.starts_with("ii") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
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

async fn check_apt_cache(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
    let cmd = format!("apt-cache policy {} 2>&1", shell_escape(package));
    let output = vm.ssh_exec(username, &cmd).await?;

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

async fn check_installed_by_dependency(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageVerificationResult> {
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

    for dependent in &dependents {
        let check_cmd = format!("dpkg-query -W {} 2>/dev/null", shell_escape(dependent));
        if vm.ssh_exec(username, &check_cmd).await.is_ok() {
            debug!(
                "Found package {} installed as dependency of {}",
                package, dependent
            );
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

async fn gather_diagnostics(
    vm: &VmHandle,
    username: &str,
    package: &str,
) -> Result<PackageDetails> {
    let mut alternatives = Vec::new();
    let mut raw_output = String::new();

    let mut variants = vec![
        package.replace('-', "_"),
        format!("{}-dev", package),
        format!("lib{}", package),
    ];

    if !package.contains(':') {
        variants.insert(0, format!("{}:amd64", package));
        variants.push(format!("{}:all", package));
    }

    for variant in &variants {
        if variant != package {
            alternatives.push(variant.clone());
            let cmd = format!("dpkg-query -W {} 2>&1", shell_escape(variant));
            if let Ok(output) = vm.ssh_exec(username, &cmd).await {
                let first_line = output.lines().next().unwrap_or("");
                let _ = writeln!(raw_output, "Variant {}: {}", variant, first_line);

                if first_line.contains(&format!("{}\t", variant))
                    || output.contains("install ok installed")
                {
                    let _ = writeln!(raw_output, "Found as variant: {}", variant);
                }
            }
        }
    }

    let wildcard_cmd = format!("dpkg-query -W '{}*' 2>&1 | head -5", shell_escape(package));
    if let Ok(wildcard_output) = vm.ssh_exec(username, &wildcard_cmd).await
        && !wildcard_output.is_empty()
        && !wildcard_output.contains("no packages found")
    {
        let _ = write!(
            raw_output,
            "\nInstalled packages matching {}*:\n{}\n",
            package, wildcard_output
        );
    }

    let search_cmd = format!("apt-cache search {} | head -5 2>&1", shell_escape(package));
    if let Ok(search_output) = vm.ssh_exec(username, &search_cmd).await
        && !search_output.is_empty()
    {
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
