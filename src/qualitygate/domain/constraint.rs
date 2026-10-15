//! Source-bound clause inventory. A mapping or adoption record is not execution evidence.
use super::{CheckResult, ExecutionStatus, SourceReview, Verdict};
use serde::{Deserialize, Serialize};

pub const CHECK_ID: &str = "constraint-coverage";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub document: String,
    pub section: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    pub schema_version: u32,
    pub id: String,
    pub version: u32,
    pub source: Source,
    pub statement: String,
    pub required: bool,
    pub realization: Realization,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adoption: Option<SourceReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Realization {
    Checks {
        mechanism: Mechanism,
        checks: Vec<String>,
        /// Review references, not a claim that examples were executed in this run.
        positive_examples: Vec<String>,
        negative_examples: Vec<String>,
    },
    Unsupported {
        reason: String,
        evidence_owner: String,
    },
}

/// Author classification; execution strength is determined by the referenced checks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism {
    Regex,
    Ast,
    Operator,
    Tool,
    AgentPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Unsupported,
    Unadopted,
    StaleAdoption,
    SourceInvalid,
    Pending,
    Failed,
    Verified,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coverage {
    pub constraint: Constraint,
    pub expected_binding_digest: String,
    pub status: CoverageStatus,
    pub reasons: Vec<String>,
}

/// Called only after source and adoption validation by the input owner.
/// Every mapped check is inspected before deciding; failure cannot hide a gap.
pub fn execution(check_ids: &[String], results: &[CheckResult]) -> (CoverageStatus, Vec<String>) {
    let mut gaps = Vec::new();
    let mut failures = Vec::new();
    let mut applicable = false;
    for id in check_ids {
        let matches: Vec<_> = results.iter().filter(|check| &check.id == id).collect();
        let [check] = matches.as_slice() else {
            gaps.push(format!("Check {id} has {} results", matches.len()));
            continue;
        };
        if check.applicability == super::Applicability::NotApplicable
            && check.execution.status == ExecutionStatus::NotRun
            && check.verdict == Some(Verdict::Skipped)
            && check.diagnostics.is_empty()
            && check
                .execution
                .reason
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
        {
            continue;
        }
        applicable = true;
        if check.execution.status != ExecutionStatus::Completed
            || check.applicability != super::Applicability::Applicable
            || !matches!(check.verdict, Some(Verdict::Pass | Verdict::Fail))
            || (check.verdict == Some(Verdict::Pass) && !check.diagnostics.is_empty())
        {
            gaps.push(format!("Check {id} did not produce complete evidence"));
        } else if check.verdict == Some(Verdict::Fail) {
            failures.push(format!("Check {id} reported a violation"));
        }
    }
    if check_ids.is_empty() {
        gaps.push("No checks mapped".into());
    }
    if !gaps.is_empty() {
        gaps.extend(failures);
        (CoverageStatus::Pending, gaps)
    } else if !failures.is_empty() {
        (CoverageStatus::Failed, failures)
    } else if applicable {
        (CoverageStatus::Verified, Vec::new())
    } else {
        (
            CoverageStatus::NotApplicable,
            vec!["All mapped checks have explicit non-applicability evidence".into()],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Applicability, Severity};

    #[test]
    fn empty_missing_duplicate_and_incomplete_results_never_become_vacuous_success() {
        let ids = vec!["a".into(), "b".into()];
        let mut a = CheckResult::pending("a", true, Severity::Error);
        a.complete();
        let mut b = CheckResult::pending("b", true, Severity::Error);
        b.complete();
        assert_eq!(
            execution(&ids, &[a.clone(), b.clone()]).0,
            CoverageStatus::Verified
        );
        assert_eq!(execution(&[], &[]).0, CoverageStatus::Pending);
        assert_eq!(execution(&ids, &[a.clone()]).0, CoverageStatus::Pending);
        assert_eq!(
            execution(&ids, &[a.clone(), a.clone(), b.clone()]).0,
            CoverageStatus::Pending
        );
        b.verdict = Some(Verdict::Fail);
        assert_eq!(
            execution(&ids, &[a.clone(), b.clone()]).0,
            CoverageStatus::Failed
        );
        a.block(ExecutionStatus::TimedOut, "deadline");
        let (status, reasons) = execution(&ids, &[a.clone(), b.clone()]);
        assert_eq!(status, CoverageStatus::Pending);
        assert_eq!(reasons.len(), 2, "retain both gap and violation");
        a.complete();
        a.applicability = Applicability::Unknown;
        assert_eq!(execution(&ids, &[a, b]).0, CoverageStatus::Pending);
    }

    #[test]
    fn only_explicit_nonapplicability_is_accepted_and_inconsistent_pass_is_rejected() {
        let ids = vec!["a".into()];
        let mut result = CheckResult::pending("a", true, Severity::Error);
        result.skip("No applicable changed entities in complete facts");
        assert_eq!(
            execution(&ids, &[result.clone()]).0,
            CoverageStatus::NotApplicable
        );
        result.execution.reason = Some(" ".into());
        assert_eq!(
            execution(&ids, &[result.clone()]).0,
            CoverageStatus::Pending
        );
        result.complete();
        result.diagnostics.push(crate::domain::Diagnostic {
            id: "finding".into(),
            fingerprint: "finding".into(),
            file: None,
            range: None,
            message: "retained violation".into(),
            evidence: serde_json::json!({}),
            fix: String::new(),
            recheck: Default::default(),
        });
        assert_eq!(execution(&ids, &[result]).0, CoverageStatus::Pending);
    }
}
