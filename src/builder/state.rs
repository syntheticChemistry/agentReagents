// SPDX-License-Identifier: AGPL-3.0-or-later
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
        /// Approximate progress from 0.0 to 1.0 within this phase.
        progress: f32,
    },

    /// Installing COSMIC desktop
    InstallingCosmic {
        /// Approximate progress from 0.0 to 1.0 within this phase.
        progress: f32,
    },

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
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },

    /// Network connectivity lost (NEW)
    NetworkLost {
        /// Human-readable explanation (e.g. timeout).
        reason: String,
    },
}

impl BuildState {
    /// Check if this state requires network connectivity (NEW)
    ///
    /// Returns true for states where network access is critical
    pub const fn requires_network(&self) -> bool {
        matches!(
            self,
            Self::NetworkEstablishment
                | Self::NetworkVerified
                | Self::Monitoring
                | Self::CloudInitInit
                | Self::InstallingPackages { .. }
                | Self::InstallingCosmic { .. }
                | Self::InstallingRustDesk
                | Self::CloudInitComplete
                | Self::Verifying
                | Self::VerifyingNetwork
        )
    }

    /// Check if the build is in a terminal state
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Failed { .. } | Self::NetworkLost { .. }
        )
    }

    /// Check if the build failed
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::NetworkLost { .. })
    }

    /// Check if the build is complete
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Get a human-readable description of the state
    pub fn description(&self) -> String {
        match self {
            Self::Idle => "Idle".to_string(),
            Self::Starting => "Starting build process".to_string(),
            Self::CreatingVm => "Creating builder VM".to_string(),
            Self::NetworkEstablishment => "Establishing network".to_string(),
            Self::NetworkVerified => "Network verified".to_string(),
            Self::Monitoring => "Monitoring build".to_string(),
            Self::CloudInitInit => "Initializing cloud-init".to_string(),
            Self::InstallingPackages { progress } => {
                format!("Installing packages ({:.0}%)", progress * 100.0)
            }
            Self::InstallingCosmic { progress } => {
                format!("Installing COSMIC desktop ({:.0}%)", progress * 100.0)
            }
            Self::InstallingRustDesk => "Installing RustDesk".to_string(),
            Self::CloudInitComplete => "Cloud-init complete".to_string(),
            Self::Verifying => "Verifying installation".to_string(),
            Self::VerifyingNetwork => "Verifying network".to_string(),
            Self::Finalizing => "Finalizing template".to_string(),
            Self::Complete => "Build complete".to_string(),
            Self::Failed { reason } => format!("Build failed: {}", reason),
            Self::NetworkLost { reason } => format!("Network lost: {}", reason),
        }
    }

    /// Get overall progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        match self {
            Self::Starting => 0.05,
            Self::CreatingVm => 0.10,
            Self::NetworkEstablishment => 0.12,
            Self::NetworkVerified => 0.14,
            Self::Monitoring => 0.15,
            Self::CloudInitInit => 0.20,
            Self::InstallingPackages { progress } => 0.20 + (progress * 0.30),
            Self::InstallingCosmic { progress } => 0.50 + (progress * 0.30),
            Self::InstallingRustDesk => 0.80,
            Self::CloudInitComplete => 0.85,
            Self::Verifying => 0.90,
            Self::VerifyingNetwork => 0.92,
            Self::Finalizing => 0.95,
            Self::Complete => 1.0,
            Self::Idle | Self::Failed { .. } | Self::NetworkLost { .. } => 0.0,
        }
    }
}

/// Progress information for the build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildProgress {
    /// Current high-level build state.
    pub state: BuildState,
    /// Overall progress from 0.0 to 1.0.
    pub progress: f32,
    /// Short human-readable status line.
    pub description: String,
    /// Seconds since the build started (wall clock).
    pub elapsed_secs: u64,
    /// Network health status (NEW)
    pub network_healthy: bool,
}

impl BuildProgress {
    /// Builds a snapshot from the current state and elapsed time.
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
    pub const fn set_network_healthy(&mut self, healthy: bool) {
        self.network_healthy = healthy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_terminal() {
        assert!(BuildState::Complete.is_terminal());
        assert!(
            BuildState::Failed {
                reason: "test".to_string()
            }
            .is_terminal()
        );
        assert!(
            BuildState::NetworkLost {
                reason: "timeout".to_string()
            }
            .is_terminal()
        );
        assert!(!BuildState::Starting.is_terminal());
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "Literal progress bounds in unit tests")]
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

    #[test]
    fn build_progress_new_sets_network_flag_and_progress() {
        let state = BuildState::InstallingCosmic { progress: 0.25 };
        let mut p = BuildProgress::new(state.clone(), 42);
        assert_eq!(p.elapsed_secs, 42);
        assert!(p.description.contains("COSMIC"));
        assert!((p.progress - state.progress()).abs() < f32::EPSILON);
        p.set_network_healthy(true);
        assert!(p.network_healthy);
    }

    #[test]
    fn failed_and_idle_flags() {
        assert!(
            BuildState::Failed {
                reason: "x".to_string()
            }
            .is_failed()
        );
        assert!(!BuildState::Complete.is_failed());
        assert!(BuildState::Complete.is_complete());
        assert!(!BuildState::Monitoring.is_complete());
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "Literal progress bounds in unit tests")]
    fn progress_monotonic_phases() {
        assert_eq!(BuildState::Finalizing.progress(), 0.95);
        assert_eq!(BuildState::Idle.progress(), 0.0);
    }
}
