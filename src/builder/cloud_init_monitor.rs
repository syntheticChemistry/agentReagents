// Cloud-init monitoring and status checking

use serde::{Serialize, Deserialize};

/// Cloud-init execution status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CloudInitStatus {
    /// Cloud-init is currently running
    Running {
        stage: CloudInitStage,
    },
    
    /// Cloud-init completed successfully
    Done,
    
    /// Cloud-init encountered an error
    Error {
        message: String,
    },
}

impl CloudInitStatus {
    /// Check if cloud-init is complete
    pub fn is_done(&self) -> bool {
        matches!(self, CloudInitStatus::Done)
    }

    /// Check if cloud-init has an error
    pub fn is_error(&self) -> bool {
        matches!(self, CloudInitStatus::Error { .. })
    }

    /// Check if cloud-init is still running
    pub fn is_running(&self) -> bool {
        matches!(self, CloudInitStatus::Running { .. })
    }
}

/// Cloud-init execution stages
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
            "done" => CloudInitStatus::Done,
            "running" => {
                // Try to determine stage from detail
                let stage = json.detail
                    .as_ref()
                    .and_then(|d| parse_stage_from_detail(d))
                    .unwrap_or(CloudInitStage::Unknown("running".to_string()));
                
                CloudInitStatus::Running { stage }
            }
            "error" => {
                let message = if json.errors.is_empty() {
                    json.detail.unwrap_or_else(|| "Unknown error".to_string())
                } else {
                    json.errors.join("; ")
                };
                CloudInitStatus::Error { message }
            }
            "disabled" => CloudInitStatus::Error {
                message: "Cloud-init is disabled".to_string(),
            },
            other => CloudInitStatus::Error {
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
}

