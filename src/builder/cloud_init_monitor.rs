// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cloud-init monitoring and JSON status parsing.

use serde::{Deserialize, Serialize};

/// Cloud-init execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudInitStatus {
    /// Cloud-init is currently running
    Running {
        /// Parsed stage from `cloud-init status` detail.
        stage: CloudInitStage,
    },

    /// Cloud-init completed successfully
    Done,

    /// Cloud-init encountered an error
    Error {
        /// Error summary from status output or stderr.
        message: String,
    },
}

impl CloudInitStatus {
    /// Check if cloud-init is complete
    pub const fn is_done(&self) -> bool {
        matches!(self, Self::Done)
    }

    /// Check if cloud-init has an error
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Check if cloud-init is still running
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// Cloud-init execution stages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudInitStage {
    /// Local stage (early boot)
    Init,

    /// Config stage (network up, running config modules)
    Config,

    /// Final stage (running final modules)
    Final,

    /// Modules config stage
    ModulesConfig,

    /// Modules final stage
    ModulesFinal,

    /// Unknown/other stage
    Unknown(String),
}

/// Cloud-init status as returned by `cloud-init status --format=json`
#[derive(Debug, Deserialize)]
pub struct CloudInitStatusJson {
    pub status: String,
    pub detail: Option<String>,
    pub errors: Vec<String>,
    #[serde(default)]
    pub recoverable_errors: serde_json::Value,
}

impl From<CloudInitStatusJson> for CloudInitStatus {
    fn from(json: CloudInitStatusJson) -> Self {
        match json.status.as_str() {
            "done" => Self::Done,
            "running" => {
                // Try to determine stage from detail
                let stage = json
                    .detail
                    .as_ref()
                    .and_then(|d| parse_stage_from_detail(d))
                    .unwrap_or_else(|| CloudInitStage::Unknown("running".to_string()));

                Self::Running { stage }
            }
            "error" => {
                let message = if json.errors.is_empty() {
                    json.detail.unwrap_or_else(|| "Unknown error".to_string())
                } else {
                    json.errors.join("; ")
                };
                Self::Error { message }
            }
            "disabled" => Self::Error {
                message: "Cloud-init is disabled".to_string(),
            },
            other => Self::Error {
                message: format!("Unknown cloud-init status: {}", other),
            },
        }
    }
}

/// Parse cloud-init stage from detail string
fn parse_stage_from_detail(detail: &str) -> Option<CloudInitStage> {
    let detail_lower = detail.to_lowercase();

    if detail_lower.contains("init") {
        Some(CloudInitStage::Init)
    } else if detail_lower.contains("config") {
        Some(CloudInitStage::Config)
    } else if detail_lower.contains("final") {
        Some(CloudInitStage::Final)
    } else if detail_lower.contains("modules-config") {
        Some(CloudInitStage::ModulesConfig)
    } else if detail_lower.contains("modules-final") {
        Some(CloudInitStage::ModulesFinal)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_done_status() {
        let json = CloudInitStatusJson {
            status: "done".to_string(),
            detail: None,
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };

        let status: CloudInitStatus = json.into();
        assert!(status.is_done());
    }

    #[test]
    fn test_parse_running_status() {
        let json = CloudInitStatusJson {
            status: "running".to_string(),
            detail: Some("Running in stage: init".to_string()),
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };

        let status: CloudInitStatus = json.into();
        assert!(status.is_running());

        if let CloudInitStatus::Running { stage } = status {
            assert_eq!(stage, CloudInitStage::Init);
        } else {
            panic!("Expected Running status");
        }
    }

    #[test]
    fn test_parse_error_status() {
        let json = CloudInitStatusJson {
            status: "error".to_string(),
            detail: None,
            errors: vec!["Failed to install packages".to_string()],
            recoverable_errors: serde_json::Value::Null,
        };

        let status: CloudInitStatus = json.into();
        assert!(status.is_error());
    }

    #[test]
    fn test_parse_disabled_status() {
        let json = CloudInitStatusJson {
            status: "disabled".to_string(),
            detail: Some("disabled by marker file".to_string()),
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };

        let status: CloudInitStatus = json.into();
        assert!(status.is_error());
    }

    #[test]
    fn parse_running_detail_matches_stage_heuristics() {
        // `parse_stage_from_detail` checks substrings in order: init, config, final, then
        // modules-* — so e.g. "modules-config" matches `config` first.
        let json = CloudInitStatusJson {
            status: "running".to_string(),
            detail: Some("early init local".to_string()),
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };
        let status: CloudInitStatus = json.into();
        match status {
            CloudInitStatus::Running { stage } => assert_eq!(stage, CloudInitStage::Init),
            _ => panic!("expected Running"),
        }

        let json2 = CloudInitStatusJson {
            status: "running".to_string(),
            detail: Some("no known keywords".to_string()),
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };
        let st2: CloudInitStatus = json2.into();
        match st2 {
            CloudInitStatus::Running { stage } => {
                assert!(matches!(stage, CloudInitStage::Unknown(_)));
            }
            _ => panic!("expected Running"),
        }
    }

    #[test]
    fn parse_unknown_status_string_becomes_error() {
        let json = CloudInitStatusJson {
            status: "weird".to_string(),
            detail: None,
            errors: vec![],
            recoverable_errors: serde_json::Value::Null,
        };
        let status: CloudInitStatus = json.into();
        assert!(status.is_error());
    }

    #[test]
    fn cloud_init_status_predicates() {
        assert!(CloudInitStatus::Done.is_done());
        assert!(!CloudInitStatus::Done.is_running());
        let err = CloudInitStatus::Error {
            message: "m".to_string(),
        };
        assert!(err.is_error());
    }
}
