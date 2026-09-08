//! Versioned execution results; a successful tool exit alone is not a gate pass.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotIdentity {
    pub mode: String,
    pub base: String,
    pub head: String,
    pub content_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_request: Option<MergeRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub provider: String,
    pub url: String,
    pub repository: String,
    pub number: u64,
    pub source_branch: String,
    pub target_branch: String,
    pub source_head: String,
    pub target_head: String,
    pub merge_base: String,
    pub api_response_digests: Vec<String>,
}

impl MergeRequest {
    pub fn same_comparison(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.repository == other.repository
            && self.number == other.number
            && self.source_head == other.source_head
            && self.target_head == other.target_head
            && self.merge_base == other.merge_base
            && self.source_branch == other.source_branch
            && self.target_branch == other.target_branch
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvidence {
    pub source: String,
    pub config_digest: String,
    pub rules_digest: String,
    pub task_contract_digest: Option<String>,
    pub trust: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub required_checks: Vec<String>,
    pub pending_delivery_checks: Vec<String>,
    pub acceptance: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub run_id: String,
    pub scope: String,
    pub profile: String,
    pub snapshot: SnapshotIdentity,
    pub policy: PolicyEvidence,
    pub plan: PlanSummary,
    pub gate: super::Gate,
    pub checks: Vec<CheckResult>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Applicable,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Completed,
    NotRun,
    Blocked,
    TimedOut,
    ToolError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Recheck {
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: String,
    pub fingerprint: String,
    pub file: Option<String>,
    pub range: Option<Range>,
    pub message: String,
    pub evidence: serde_json::Value,
    pub fix: String,
    pub recheck: Recheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub status: ExecutionStatus,
    pub reason: Option<String>,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub started_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub digest: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub rule_version: u32,
    pub required: bool,
    pub severity: Severity,
    pub applicability: Applicability,
    pub execution: Execution,
    pub verdict: Option<Verdict>,
    pub matched_entities: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl CheckResult {
    /// Creates an unexecuted result. Callers must supply evidence before passing.
    pub fn pending(id: impl Into<String>, required: bool, severity: Severity) -> Self {
        Self {
            id: id.into(),
            rule_version: 1,
            required,
            severity,
            applicability: Applicability::Unknown,
            execution: Execution {
                status: ExecutionStatus::NotRun,
                reason: Some("Check has not run".into()),
                argv: Vec::new(),
                cwd: None,
                started_at_ms: None,
                duration_ms: None,
                exit_code: None,
                artifacts: Vec::new(),
            },
            verdict: None,
            matched_entities: 0,
            diagnostics: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// A completed check may still fail its acceptance conditions.
    pub fn complete(&mut self) {
        self.applicability = Applicability::Applicable;
        self.execution.status = ExecutionStatus::Completed;
        self.execution.reason = None;
        self.verdict = Some(if self.diagnostics.is_empty() {
            Verdict::Pass
        } else {
            Verdict::Fail
        });
    }

    /// Capability or execution gaps never become a skipped/pass verdict.
    pub fn block(&mut self, status: ExecutionStatus, reason: impl Into<String>) {
        self.execution.status = status;
        self.execution.reason = Some(reason.into());
        self.verdict = None;
    }

    /// A skip requires a positive non-applicability explanation.
    pub fn skip(&mut self, reason: impl Into<String>) {
        self.applicability = Applicability::NotApplicable;
        self.execution.status = ExecutionStatus::NotRun;
        self.execution.reason = Some(reason.into());
        self.verdict = Some(Verdict::Skipped);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub checks_total: usize,
    pub pass: usize,
    pub fail: usize,
    pub skipped: usize,
    pub incomplete: usize,
    pub diagnostics_total: usize,
    pub diagnostics_filtered: usize,
}

impl Summary {
    pub fn from_checks(checks: &[CheckResult]) -> Self {
        let mut summary = Self {
            checks_total: checks.len(),
            ..Self::default()
        };
        for check in checks {
            match check.verdict {
                Some(Verdict::Pass) => summary.pass += 1,
                Some(Verdict::Fail) => summary.fail += 1,
                Some(Verdict::Skipped) => summary.skipped += 1,
                None => summary.incomplete += 1,
            }
            summary.diagnostics_total += check.diagnostics.len();
        }
        summary
    }
}
