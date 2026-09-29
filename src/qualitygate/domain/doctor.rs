//! Preflight evidence is deliberately separate from delivery gate evidence.

use super::{Artifact, NextStep, PolicyEvidence, SnapshotIdentity, runtime};
use serde::Serialize;

pub const DETAIL_LIMIT: usize = 100;
pub const PROBE_SECONDS: u64 = 120;

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub scope: &'static str,
    pub runtime: Option<runtime::Capabilities>,
    pub profile: String,
    pub probe_tools: bool,
    pub complete: bool,
    pub snapshot: Option<SnapshotIdentity>,
    pub policy: Option<PolicyEvidence>,
    pub selection: Option<super::check_scope::ScopeEvidence>,
    pub planned_checks: Vec<String>,
    pub pending_delivery_checks: Vec<String>,
    pub budgets: Vec<SnapshotBudget>,
    pub checks: Vec<Check>,
    pub diagnostics: Vec<Diagnostic>,
    pub not_executed: Vec<NotExecuted>,
    pub next_steps: Vec<NextStep>,
}

impl Report {
    pub fn new(profile: String, probe_tools: bool) -> Self {
        Self {
            schema_version: 1,
            scope: "preflight",
            runtime: None,
            profile,
            probe_tools,
            complete: false,
            snapshot: None,
            policy: None,
            selection: None,
            planned_checks: Vec::new(),
            pending_delivery_checks: Vec::new(),
            budgets: Vec::new(),
            checks: Vec::new(),
            diagnostics: Vec::new(),
            not_executed: Vec::new(),
            next_steps: Vec::new(),
        }
    }

    pub fn exit_code(&self) -> u8 {
        if self.complete && self.diagnostics.is_empty() {
            0
        } else {
            2
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub checks: Vec<String>,
    pub field: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Diagnostic {
    pub fn new(code: &str, message: impl Into<String>, checks: Vec<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            checks,
            field: None,
            line: None,
            column: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub id: String,
    pub required_env: Vec<Environment>,
    pub executable: Option<Artifact>,
    pub probes: Vec<Probe>,
}

#[derive(Debug, Serialize)]
pub struct Environment {
    pub name: String,
    pub present: bool,
}

#[derive(Debug, Serialize)]
pub struct Probe {
    pub id: String,
    pub executable: Option<Artifact>,
    pub attempted: bool,
    pub complete: bool,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct NotExecuted {
    pub check: String,
    pub operation: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SnapshotBudget {
    pub side: String,
    pub reference: String,
    pub inventory_digest: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub max_files: usize,
    pub max_bytes: usize,
    pub max_file_bytes: usize,
    pub oversized_file_count: usize,
    pub oversized_files: Vec<super::snapshot_budget::OversizedFile>,
    pub unsupported_entry_count: usize,
    pub unsupported_entries: Vec<String>,
    pub excluded_file_count: usize,
    pub excluded_paths: Vec<String>,
    pub protected_exclusion_count: usize,
    pub protected_exclusions: Vec<String>,
    pub details_truncated: bool,
    pub recommended_max_file_mib: Option<u64>,
}

impl SnapshotBudget {
    pub fn ready(&self) -> bool {
        self.file_count <= self.max_files
            && self.total_bytes <= self.max_bytes as u64
            && self.oversized_file_count == 0
            && self.unsupported_entry_count == 0
            && self.protected_exclusion_count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_cannot_succeed_with_incomplete_or_blocked_evidence() {
        let mut report = Report::new("full".into(), false);
        assert_eq!(report.exit_code(), 2);
        report.complete = true;
        assert_eq!(report.exit_code(), 0);
        report.diagnostics.push(Diagnostic::new(
            "environment.missing",
            "Missing declared prerequisite",
            vec!["test".into()],
        ));
        assert_eq!(report.exit_code(), 2);
        assert_eq!(serde_json::to_value(report).unwrap()["scope"], "preflight");
    }
}
