// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verification domain types

use serde::{Deserialize, Serialize};

/// Result of a single verification check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    /// Name/label for this check.
    pub name: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Optional diagnostic details.
    pub details: Option<String>,
}

/// Aggregate verification result across all checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether all checks passed.
    pub passed: bool,
    /// Individual check results.
    pub checks: Vec<VerificationCheck>,
    /// Total number of checks run.
    pub total: usize,
    /// How many checks passed.
    pub passed_count: usize,
    /// How many checks failed.
    pub failed_count: usize,
}

impl VerificationResult {
    /// Build from a list of individual checks, computing aggregates.
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

    /// Returns only the checks that failed.
    pub fn failed_checks(&self) -> Vec<&VerificationCheck> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }

    /// Human-readable one-line summary.
    pub fn summary(&self) -> String {
        if self.passed {
            format!("All {} checks passed", self.total)
        } else {
            format!(
                "{}/{} checks passed ({} failed)",
                self.passed_count, self.total, self.failed_count
            )
        }
    }
}

/// Result of package verification with method and diagnostics.
#[derive(Debug, Clone)]
pub struct PackageVerificationResult {
    /// Package name that was verified.
    pub package: String,
    /// Whether the package is installed.
    pub installed: bool,
    /// Which method confirmed the install.
    pub method: VerificationMethod,
    /// Detailed diagnostic info from the verification.
    pub details: PackageDetails,
}

/// Which method successfully verified the package.
#[derive(Debug, Clone)]
pub enum VerificationMethod {
    /// Verified via `dpkg-query`.
    DpkgQuery,
    /// Verified via `dpkg -L`.
    DpkgList,
    /// Verified via `apt-cache`.
    AptCache,
    /// Installed as a dependency of another package.
    InstalledByDependency(String),
    /// All verification methods failed.
    AllFailed,
}

/// Detailed package information gathered during verification.
#[derive(Debug, Clone, Default)]
pub struct PackageDetails {
    /// Actual package name (may differ from requested due to aliases).
    pub actual_name: Option<String>,
    /// Installed version string.
    pub version: Option<String>,
    /// dpkg status line (e.g. "install ok installed").
    pub dpkg_status: Option<String>,
    /// Packages that depend on this one.
    pub required_by: Vec<String>,
    /// Alternative package names that were checked.
    pub alternatives_checked: Vec<String>,
    /// Raw output from the verification command.
    pub raw_output: Option<String>,
}

/// Wrap a string in single quotes, escaping embedded quotes for shell safety.
pub fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
