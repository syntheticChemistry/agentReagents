//! Build state tracking for VM image creation
//!
//! Provides a state machine for tracking build progress with network awareness.

use serde::{Deserialize, Serialize};

/// State of the image build process
///
/// Enhanced with network-aware states for resilient VM provisioning
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BuildState {
    /// Initial state, not started
    Idle,

    /// Starting the build process
    Starting,

    /// Creating the builder VM
    CreatingVm,

    /// Establishing network connectivity (NEW)
    NetworkEstablishment,

    /// Network verified and stable (NEW)
    NetworkVerified,

    /// Monitoring the build process
    Monitoring,

    /// Cloud-init initialization phase
    CloudInitInit,

    /// Installing system packages
    InstallingPackages {
        progress: f32, // 0.0 to 1.0
    },

    /// Installing COSMIC desktop
    InstallingCosmic { progress: f32 },

    /// Installing RustDesk
    InstallingRustDesk,

    /// Cloud-init completed
    CloudInitComplete,

    /// Verifying installation
    Verifying,

    /// Re-verifying network after critical operations (NEW)
    VerifyingNetwork,

    /// Finalizing template (sparsify, move, etc.)
    Finalizing,

    /// Build complete
    Complete,

    /// Build failed
    Failed { reason: String },

    /// Network connectivity lost (NEW)
    NetworkLost { reason: String },
}

impl BuildState {
    /// Check if this state requires network connectivity (NEW)
    ///
    /// Returns true for states where network access is critical
    pub fn requires_network(&self) -> bool {
        matches!(
            self,
            BuildState::NetworkEstablishment
                | BuildState::NetworkVerified
                | BuildState::Monitoring
                | BuildState::CloudInitInit
                | BuildState::InstallingPackages { .. }
                | BuildState::InstallingCosmic { .. }
                | BuildState::InstallingRustDesk
                | BuildState::CloudInitComplete
                | BuildState::Verifying
                | BuildState::VerifyingNetwork
        )
    }

    /// Check if the build is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            BuildState::Complete | BuildState::Failed { .. } | BuildState::NetworkLost { .. }
        )
    }

    /// Check if the build failed
    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            BuildState::Failed { .. } | BuildState::NetworkLost { .. }
        )
    }

    /// Check if the build is complete
    pub fn is_complete(&self) -> bool {
        matches!(self, BuildState::Complete)
    }

    /// Get a human-readable description of the state
    pub fn description(&self) -> String {
        match self {
            BuildState::Idle => "Idle".to_string(),
            BuildState::Starting => "Starting build process".to_string(),
            BuildState::CreatingVm => "Creating builder VM".to_string(),
            BuildState::NetworkEstablishment => "Establishing network".to_string(),
            BuildState::NetworkVerified => "Network verified".to_string(),
            BuildState::Monitoring => "Monitoring build".to_string(),
            BuildState::CloudInitInit => "Initializing cloud-init".to_string(),
            BuildState::InstallingPackages { progress } => {
                format!("Installing packages ({:.0}%)", progress * 100.0)
            }
            BuildState::InstallingCosmic { progress } => {
                format!("Installing COSMIC desktop ({:.0}%)", progress * 100.0)
            }
            BuildState::InstallingRustDesk => "Installing RustDesk".to_string(),
            BuildState::CloudInitComplete => "Cloud-init complete".to_string(),
            BuildState::Verifying => "Verifying installation".to_string(),
            BuildState::VerifyingNetwork => "Verifying network".to_string(),
            BuildState::Finalizing => "Finalizing template".to_string(),
            BuildState::Complete => "Build complete".to_string(),
            BuildState::Failed { reason } => format!("Build failed: {}", reason),
            BuildState::NetworkLost { reason } => format!("Network lost: {}", reason),
        }
    }

    /// Get overall progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        match self {
            BuildState::Idle => 0.0,
            BuildState::Starting => 0.05,
            BuildState::CreatingVm => 0.10,
            BuildState::NetworkEstablishment => 0.12,
            BuildState::NetworkVerified => 0.14,
            BuildState::Monitoring => 0.15,
            BuildState::CloudInitInit => 0.20,
            BuildState::InstallingPackages { progress } => 0.20 + (progress * 0.30),
            BuildState::InstallingCosmic { progress } => 0.50 + (progress * 0.30),
            BuildState::InstallingRustDesk => 0.80,
            BuildState::CloudInitComplete => 0.85,
            BuildState::Verifying => 0.90,
            BuildState::VerifyingNetwork => 0.92,
            BuildState::Finalizing => 0.95,
            BuildState::Complete => 1.0,
            BuildState::Failed { .. } => 0.0,
            BuildState::NetworkLost { .. } => 0.0,
        }
    }
}

/// Progress information for the build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildProgress {
    pub state: BuildState,
    pub progress: f32,
    pub description: String,
    pub elapsed_secs: u64,
    /// Network health status (NEW)
    pub network_healthy: bool,
}

impl BuildProgress {
    pub fn new(state: BuildState, elapsed_secs: u64) -> Self {
        let progress = state.progress();
        let description = state.description();

        Self {
            state,
            progress,
            description,
            elapsed_secs,
            network_healthy: false,
        }
    }

    /// Update network health status (NEW)
    pub fn set_network_healthy(&mut self, healthy: bool) {
        self.network_healthy = healthy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_terminal() {
        assert!(BuildState::Complete.is_terminal());
        assert!(BuildState::Failed {
            reason: "test".to_string()
        }
        .is_terminal());
        assert!(BuildState::NetworkLost {
            reason: "timeout".to_string()
        }
        .is_terminal());
        assert!(!BuildState::Starting.is_terminal());
    }

    #[test]
    fn test_state_progress() {
        assert_eq!(BuildState::Idle.progress(), 0.0);
        assert_eq!(BuildState::Complete.progress(), 1.0);
        assert!(BuildState::InstallingPackages { progress: 0.5 }.progress() > 0.2);
        assert!(BuildState::InstallingPackages { progress: 0.5 }.progress() < 0.6);
    }

    #[test]
    fn test_state_description() {
        let desc = BuildState::InstallingPackages { progress: 0.5 }.description();
        assert!(desc.contains("50%"));
    }

    #[test]
    fn test_requires_network() {
        assert!(!BuildState::Idle.requires_network());
        assert!(!BuildState::Starting.requires_network());
        assert!(BuildState::NetworkEstablishment.requires_network());
        assert!(BuildState::NetworkVerified.requires_network());
        assert!(BuildState::Monitoring.requires_network());
        assert!(BuildState::Verifying.requires_network());
    }

    #[test]
    fn test_network_lost_state() {
        let state = BuildState::NetworkLost {
            reason: "timeout".to_string(),
        };
        assert!(state.is_terminal());
        assert!(state.is_failed());
        assert!(!state.is_complete());
        assert!(state.description().contains("Network lost"));
    }
}
