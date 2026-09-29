//! Offline runtime contracts. Capability identifiers include their semantic version.

use super::Artifact;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CAPABILITIES: &[&str] = &[
    "runtime.capabilities.v1",
    "config.requires.v1",
    "command.required-env.v1",
    "doctor.static.v1",
    "doctor.tool-probes.v1",
    "snapshot.worktree.v1",
    "snapshot.staged.v1",
    "snapshot.diff.v1",
    "snapshot.exclusions.v1",
    "snapshot.file-budget.v1",
    "rules.file-contracts.v1",
    "reports.ratchet.v1",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cli_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

impl Requirements {
    pub fn is_empty(&self) -> bool {
        self.min_cli_version.is_none() && self.capabilities.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub schema_version: u32,
    pub cli_version: String,
    pub executable: Artifact,
    pub platform: Platform,
    pub capabilities: Vec<String>,
    pub protocols: BTreeMap<String, u32>,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub snapshot_default_file_bytes: usize,
    pub snapshot_max_file_bytes: usize,
    pub snapshot_default_bytes: usize,
    pub snapshot_max_bytes: usize,
    pub snapshot_max_files: usize,
    pub snapshot_max_jobs: usize,
    pub snapshot_max_seconds: u64,
    pub configuration_bytes: usize,
    pub process_stream_bytes: usize,
    pub tool_version_bytes: usize,
    pub tool_probe_max_seconds: u64,
    pub doctor_probe_seconds: u64,
    pub diagnostic_details: usize,
}

/// Typed early failures preserve the original parser text and available location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ConfigurationError {}
