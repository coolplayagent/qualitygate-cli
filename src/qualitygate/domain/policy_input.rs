//! Typed policy input failures shared by configuration and snapshot loading.

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyInputError {
    LocalConfigurationMissing { path: String },
    SnapshotConfigurationMissing { path: String },
}

impl fmt::Display for PolicyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalConfigurationMissing { path } => {
                write!(formatter, "Policy configuration not found: {path}")
            }
            Self::SnapshotConfigurationMissing { path } => {
                write!(
                    formatter,
                    "Selected policy snapshot has no qualitygate configuration: {path}"
                )
            }
        }
    }
}

impl std::error::Error for PolicyInputError {}

/// A reliable follow-up for an incomplete early report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NextStep {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub message: String,
}
