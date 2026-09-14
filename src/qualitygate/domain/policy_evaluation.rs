//! Independent acceptance oracles for paired policy evaluation.

use super::{Report, Verdict, VerificationBoundary, evolution::Actor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "policy_evaluation_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    Pass,
    Block,
    Incomplete,
}

impl Conclusion {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Block => 1,
            Self::Incomplete => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    Replay,
    HeldOut,
    Anchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    Pass,
    Fail,
    Skipped,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBudget {
    pub snapshot_max_mib: u32,
    pub snapshot_jobs: u16,
    pub snapshot_timeout_seconds: u32,
    pub max_live_snapshot_mib: u32,
    pub max_parallel: u16,
    pub case_timeout_seconds: u32,
    pub total_timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub report_ref: Option<String>,
    pub complete: bool,
    pub mismatches: Vec<String>,
    pub selected_rules: usize,
    pub activated_rules: usize,
    pub completed_checks: usize,
    pub findings: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl Observation {
    pub fn from_report(
        report: &Report,
        expected: &BTreeMap<String, Expected>,
        duration_ms: u64,
    ) -> Self {
        let mut mismatches = Vec::new();
        for (id, expected) in expected {
            let observed = report.checks.iter().find(|check| &check.id == id);
            let actual = observed
                .and_then(|check| check.verdict)
                .map(|verdict| match verdict {
                    Verdict::Pass => Expected::Pass,
                    Verdict::Fail => Expected::Fail,
                    Verdict::Skipped => Expected::Skipped,
                })
                .unwrap_or(Expected::Absent);
            if actual != *expected || *expected == Expected::Absent && observed.is_some() {
                mismatches.push(format!("{id}: expected {expected:?}, observed {actual:?}"));
            }
        }
        for check in &report.checks {
            if !expected.contains_key(&check.id) {
                mismatches.push(format!(
                    "{}: no protected expectation for executed check",
                    check.id
                ));
            }
        }
        Self {
            report_ref: None,
            complete: report.gate.complete && report.plan.pending_delivery_checks.is_empty(),
            mismatches,
            selected_rules: report
                .checks
                .iter()
                .filter(|check| check.metadata.contains_key("rule_definition"))
                .count(),
            activated_rules: report
                .checks
                .iter()
                .filter(|check| {
                    check.metadata.contains_key("rule_definition")
                        && check.applicability == super::Applicability::Applicable
                })
                .count(),
            completed_checks: report
                .checks
                .iter()
                .filter(|check| check.execution.status == super::ExecutionStatus::Completed)
                .count(),
            findings: report
                .checks
                .iter()
                .map(|check| check.diagnostics.len())
                .sum(),
            duration_ms,
            error: None,
        }
    }

    pub fn incomplete(error: String) -> Self {
        Self {
            report_ref: None,
            complete: false,
            mismatches: Vec::new(),
            selected_rules: 0,
            activated_rules: 0,
            completed_checks: 0,
            findings: 0,
            duration_ms: 0,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatedCase {
    pub id: String,
    pub kind: CaseKind,
    pub base: String,
    pub head: String,
    pub snapshot_digest: Option<String>,
    pub task_digest: String,
    pub baseline: Observation,
    pub candidate: Observation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRun {
    pub schema_version: u32,
    pub candidate_id: String,
    pub candidate_revision: String,
    pub candidate_policy: String,
    pub baseline_policy: String,
    pub suite_digest: String,
    pub trust_digest: String,
    pub evaluator_epoch: String,
    pub evaluator_digest: String,
    pub environment_digest: String,
    pub budget: EvaluationBudget,
    pub jobs: u16,
    pub generation_actor: Actor,
    pub evaluation_actor: Actor,
    pub started_at: u64,
    pub ended_at: u64,
    pub cases: Vec<EvaluatedCase>,
    pub conclusion: Conclusion,
    pub reasons: Vec<String>,
    pub verification: VerificationBoundary,
}

/// Compare actual producer identities for shared checks without comparing temporary paths or timings.
pub fn producer_environment(report: &Report) -> BTreeMap<String, serde_json::Value> {
    report.checks.iter().map(|check| {
        let tools = check.metadata.get("tools").and_then(serde_json::Value::as_array)
            .into_iter().flatten().map(|tool| serde_json::json!({"id":tool["id"],
                "executable_digest":tool["executable"]["digest"],"version":tool["version"],"inputs":tool["inputs"]})).collect::<Vec<_>>();
        (check.id.clone(), serde_json::json!({"command_executable":check.metadata.get("command_executable").map(|identity| &identity["digest"]),"tools":tools}))
    }).collect()
}

pub fn matched_producers(
    left: &BTreeMap<String, serde_json::Value>,
    right: &BTreeMap<String, serde_json::Value>,
) -> bool {
    left.iter()
        .all(|(id, observed)| right.get(id).is_none_or(|other| observed == other))
}

/// Compare complete observations to the independent oracle, including intentional negative cases.
pub fn decide(
    cases: &[EvaluatedCase],
    expected_count: usize,
    min_improvements: usize,
    invalid: &[String],
) -> (Conclusion, Vec<String>) {
    let mut gaps = invalid.to_vec();
    if cases.len() != expected_count || cases.is_empty() {
        gaps.push("Not every protected case was evaluated".into());
    }
    if !cases.iter().any(|case| case.kind == CaseKind::Replay)
        || !cases.iter().any(|case| case.kind == CaseKind::HeldOut)
        || !cases.iter().any(|case| case.kind == CaseKind::Anchor)
    {
        gaps.push(
            "Paired replay, independent held-out evidence and anchor calibration are required"
                .into(),
        );
    }
    for case in cases {
        if !case.baseline.complete || !case.candidate.complete || case.snapshot_digest.is_none() {
            gaps.push(format!("{}: paired execution is incomplete", case.id));
        }
    }
    if !gaps.is_empty() {
        return (Conclusion::Incomplete, gaps);
    }
    let mut blockers = Vec::new();
    let mut improvements = 0;
    for case in cases {
        if !case.candidate.mismatches.is_empty() {
            blockers.push(format!(
                "{}: candidate differs from protected expectations: {}",
                case.id,
                case.candidate.mismatches.join("; ")
            ));
        }
        if case.candidate.mismatches.len() < case.baseline.mismatches.len() {
            improvements += 1;
        }
    }
    if improvements < min_improvements {
        blockers.push(format!(
            "Observed {improvements} improved cases; acceptance requires {min_improvements}"
        ));
    }
    if blockers.is_empty() {
        (Conclusion::Pass, Vec::new())
    } else {
        (Conclusion::Block, blockers)
    }
}
