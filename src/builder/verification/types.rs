// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verification domain types

use serde::{Deserialize, Serialize};

/// Result of a single verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub name: String,
    pub passed: bool,
    pub details: Option<String>,
}

/// Aggregate verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub checks: Vec<VerificationCheck>,
    pub total: usize,
    pub passed_count: usize,
    pub failed_count: usize,
}

impl VerificationResult {
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

    pub fn failed_checks(&self) -> Vec<&VerificationCheck> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }

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

/// Result of package verification with method and diagnostics
#[derive(Debug, Clone)]
pub struct PackageVerificationResult {
    pub package: String,
    pub installed: bool,
    pub method: VerificationMethod,
    pub details: PackageDetails,
}

/// Which method successfully verified the package
#[derive(Debug, Clone)]
pub enum VerificationMethod {
    DpkgQuery,
    DpkgList,
    AptCache,
    InstalledByDependency(String),
    AllFailed,
}

/// Detailed package information gathered during verification
#[derive(Debug, Clone, Default)]
pub struct PackageDetails {
    pub actual_name: Option<String>,
    pub version: Option<String>,
    pub dpkg_status: Option<String>,
    pub required_by: Vec<String>,
    pub alternatives_checked: Vec<String>,
    pub raw_output: Option<String>,
}

/// Wrap a string in single quotes, escaping embedded quotes for shell safety.
pub fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
