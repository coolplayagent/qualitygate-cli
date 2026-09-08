//! Completeness and violation decisions are independent of display filtering.

use super::{Applicability, CheckResult, ExecutionStatus, Severity, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Pass,
    Fail,
    Incomplete,
}

impl Decision {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Fail => 1,
            Self::Incomplete => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    pub complete: bool,
    pub decision: Decision,
    pub blockers: Vec<String>,
}

/// Evaluates actual results against the independent required-check plan.
pub fn evaluate(checks: &[CheckResult], required: &[String], invalid: &[String]) -> Gate {
    let mut gaps = invalid.to_vec();
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    let mut completed = 0;
    for check in checks {
        if !seen.insert(check.id.as_str()) {
            gaps.push(format!("Duplicate check result: {}", check.id));
        }
        let valid_completed = check.applicability == Applicability::Applicable
            && check.execution.status == ExecutionStatus::Completed
            && matches!(check.verdict, Some(Verdict::Pass | Verdict::Fail));
        let valid_skip = check.applicability == Applicability::NotApplicable
            && check.execution.status == ExecutionStatus::NotRun
            && check.verdict == Some(Verdict::Skipped)
            && check
                .execution
                .reason
                .as_ref()
                .is_some_and(|reason| !reason.trim().is_empty())
            && check.diagnostics.is_empty();
        let inconsistent = check.verdict.is_some() && !valid_completed && !valid_skip
            || check.verdict == Some(Verdict::Pass) && !check.diagnostics.is_empty();
        if inconsistent {
            gaps.push(format!("Invalid check state: {}", check.id));
        }
        if valid_completed {
            completed += 1;
        }
        if (check.required || required.contains(&check.id)) && !valid_completed && !valid_skip {
            gaps.push(format!("Required check incomplete: {}", check.id));
        }
        if valid_completed
            && check.verdict == Some(Verdict::Fail)
            && check.severity == Severity::Error
        {
            failures.push(check.id.clone());
        }
    }
    for id in required {
        if !seen.contains(id.as_str()) {
            gaps.push(format!("Required check missing: {id}"));
        }
    }
    if completed == 0 {
        gaps.push("No check produced a usable result".into());
    }
    let decision = if !gaps.is_empty() {
        Decision::Incomplete
    } else if !failures.is_empty() {
        Decision::Fail
    } else {
        Decision::Pass
    };
    gaps.extend(failures);
    Gate {
        complete: decision != Decision::Incomplete,
        decision,
        blockers: gaps,
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
