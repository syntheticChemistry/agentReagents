// Installation verification

use serde::{Serialize, Deserialize};

/// Result of installation verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub cosmic_installed: bool,
    pub cosmic_package_count: usize,
    pub greeter_enabled: bool,
    pub rustdesk_installed: bool,
    pub ssh_accessible: bool,
    pub errors: Vec<String>,
}

impl VerificationResult {
    /// Create a new verification result with all checks failed
    pub fn failed() -> Self {
        Self {
            cosmic_installed: false,
            cosmic_package_count: 0,
            greeter_enabled: false,
            rustdesk_installed: false,
            ssh_accessible: false,
            errors: vec![],
        }
    }

    /// Check if all critical verifications passed
    pub fn is_success(&self) -> bool {
        self.cosmic_installed 
            && self.cosmic_package_count >= 5 
            && self.greeter_enabled
            && self.errors.is_empty()
    }

    /// Add an error to the verification result
    pub fn add_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }

    /// Get a summary of the verification
    pub fn summary(&self) -> String {
        if self.is_success() {
            format!(
                "✅ Verification passed: {} COSMIC packages, greeter enabled{}",
                self.cosmic_package_count,
                if self.rustdesk_installed { ", RustDesk installed" } else { "" }
            )
        } else {
            let mut parts = Vec::new();
            
            if !self.cosmic_installed {
                parts.push("COSMIC not installed".to_string());
            }
            if self.cosmic_package_count < 5 {
                parts.push(format!("Only {} COSMIC packages", self.cosmic_package_count));
            }
            if !self.greeter_enabled {
                parts.push("Greeter not enabled".to_string());
            }
            if !self.errors.is_empty() {
                parts.push(format!("{} errors", self.errors.len()));
            }
            
            format!("❌ Verification failed: {}", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_success() {
        let result = VerificationResult {
            cosmic_installed: true,
            cosmic_package_count: 10,
            greeter_enabled: true,
            rustdesk_installed: true,
            ssh_accessible: true,
            errors: vec![],
        };
        
        assert!(result.is_success());
    }

    #[test]
    fn test_verification_failure_no_cosmic() {
        let result = VerificationResult {
            cosmic_installed: false,
            cosmic_package_count: 0,
            greeter_enabled: false,
            rustdesk_installed: false,
            ssh_accessible: true,
            errors: vec![],
        };
        
        assert!(!result.is_success());
    }

    #[test]
    fn test_verification_failure_with_errors() {
        let mut result = VerificationResult {
            cosmic_installed: true,
            cosmic_package_count: 10,
            greeter_enabled: true,
            rustdesk_installed: false,
            ssh_accessible: true,
            errors: vec![],
        };
        
        result.add_error("Test error");
        assert!(!result.is_success());
    }

    #[test]
    fn test_verification_summary() {
        let result = VerificationResult {
            cosmic_installed: true,
            cosmic_package_count: 10,
            greeter_enabled: true,
            rustdesk_installed: true,
            ssh_accessible: true,
            errors: vec![],
        };
        
        let summary = result.summary();
        assert!(summary.contains("✅"));
        assert!(summary.contains("10"));
    }
}

