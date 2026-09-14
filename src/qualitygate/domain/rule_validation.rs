//! Serializable rule-authoring outcomes, with invalid inputs distinct from I/O gaps.

use super::Decision;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RuleIssue {
    pub stage: String,
    pub instance_path: String,
    pub schema_path: Option<String>,
    pub message: String,
}

impl RuleIssue {
    pub fn new(stage: &str, message: impl AsRef<str>) -> Self {
        Self {
            stage: stage.into(),
            instance_path: String::new(),
            schema_path: None,
            message: message.as_ref().chars().take(1024).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleFileValidation {
    pub file: String,
    pub rule_id: Option<String>,
    pub valid: bool,
    pub issues: Vec<RuleIssue>,
}

#[derive(Debug, Serialize)]
pub struct RuleValidationReport {
    pub schema_version: u32,
    pub schema_id: String,
    pub schema_digest: String,
    pub complete: bool,
    pub decision: Decision,
    pub files: Vec<RuleFileValidation>,
    pub issues: Vec<RuleIssue>,
    pub review_trust: String,
}
