use super::*;
use crate::domain::{Diagnostic, Recheck, Summary};

fn passed(id: &str) -> CheckResult {
    let mut result = CheckResult::pending(id, true, Severity::Error);
    result.complete();
    result
}

#[test]
fn empty_or_only_skipped_never_claims_validation() {
    assert_eq!(evaluate(&[], &[], &[]).decision, Decision::Incomplete);
    let mut result = passed("optional");
    result.skip("No source-declaration policy exists");
    assert_eq!(evaluate(&[result], &[], &[]).decision, Decision::Incomplete);
}

#[test]
fn missing_required_result_and_unknown_applicability_block() {
    let results = [passed("format")];
    assert_eq!(
        evaluate(&results, &["test".into()], &[]).decision,
        Decision::Incomplete
    );
    let pending = CheckResult::pending("test", true, Severity::Warning);
    assert_eq!(
        evaluate(&[results[0].clone(), pending], &[], &[]).decision,
        Decision::Incomplete
    );
}

#[test]
fn complete_does_not_mean_no_violations() {
    let mut failed = passed("test");
    failed.verdict = Some(Verdict::Fail);
    let gate = evaluate(&[failed.clone()], &[], &[]);
    assert!(gate.complete);
    assert_eq!(gate.decision.exit_code(), 1);
    failed.severity = Severity::Warning;
    assert_eq!(evaluate(&[failed], &[], &[]).decision.exit_code(), 0);
}

#[test]
fn incomplete_wins_over_failure_without_losing_failure() {
    let mut failed = passed("test");
    failed.verdict = Some(Verdict::Fail);
    let gate = evaluate(&[failed], &["coverage".into()], &[]);
    assert_eq!(gate.decision.exit_code(), 2);
    assert!(gate.blockers.contains(&"test".into()));
}

#[test]
fn optional_failure_to_execute_is_visible_without_blocking_completed_checks() {
    let mut optional = CheckResult::pending("suggest", false, Severity::Warning);
    optional.block(ExecutionStatus::TimedOut, "deadline exceeded");
    assert_eq!(
        evaluate(&[passed("build"), optional], &[], &[]).decision,
        Decision::Pass
    );
}

#[test]
fn inconsistent_and_duplicate_results_cannot_pass() {
    let result = passed("test");
    assert_eq!(
        evaluate(&[result.clone(), result.clone()], &[], &[]).decision,
        Decision::Incomplete
    );
    let mut invalid = result;
    invalid.applicability = Applicability::Unknown;
    assert_eq!(
        evaluate(&[invalid], &[], &[]).decision,
        Decision::Incomplete
    );
    let mut skipped = passed("no-policy");
    skipped.skip(" ");
    assert_eq!(
        evaluate(&[passed("build"), skipped], &[], &[]).decision,
        Decision::Incomplete
    );
}

#[test]
fn invalid_snapshot_or_policy_always_blocks() {
    let gate = evaluate(&[passed("build")], &[], &["Snapshot changed".into()]);
    assert_eq!(gate.decision, Decision::Incomplete);
    assert_eq!(gate.blockers, ["Snapshot changed"]);
}

#[test]
fn diagnostics_drive_verdict_and_summary_counts_checks_separately() {
    let mut failed = passed("test");
    failed.diagnostics.push(Diagnostic {
        id: "d1".into(),
        fingerprint: "fp1".into(),
        file: None,
        range: None,
        message: "failed".into(),
        evidence: serde_json::Value::Null,
        fix: "repair".into(),
        recheck: Recheck::default(),
    });
    assert_eq!(
        evaluate(&[failed.clone()], &[], &[]).decision,
        Decision::Incomplete
    );
    failed.complete();
    let mut skipped = passed("skip");
    skipped.skip("No matching language");
    let summary = Summary::from_checks(&[
        passed("build"),
        failed,
        skipped,
        CheckResult::pending("later", false, Severity::Info),
    ]);
    assert_eq!(
        (
            summary.checks_total,
            summary.pass,
            summary.fail,
            summary.skipped,
            summary.incomplete,
            summary.diagnostics_total
        ),
        (4, 1, 1, 1, 1, 1)
    );
    let wire = serde_json::to_string(&summary).unwrap();
    let decoded: Summary = serde_json::from_str(&wire).unwrap();
    assert_eq!(decoded.checks_total, 4);
}
