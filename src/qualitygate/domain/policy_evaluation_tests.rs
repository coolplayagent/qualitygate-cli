use super::*;
use crate::domain::{
    evolution::{Actor, ActorKind},
    *,
};
use serde_json::json;

fn report() -> Report {
    let mut check = CheckResult::pending("rule", true, Severity::Error);
    check.complete();
    check.verdict = Some(Verdict::Fail);
    check.metadata.insert("rule_definition".into(), json!({}));
    let checks = vec![check];
    Report {
        context: None,
        schema_version: 1,
        run_id: "test".into(),
        scope: "task".into(),
        profile: "full".into(),
        evaluator_digest: "evaluator".into(),
        environment_digest: "environment".into(),
        snapshot: SnapshotIdentity {
            mode: "diff".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "snapshot".into(),
            merge_request: None,
        },
        policy: PolicyEvidence {
            source: "policy".into(),
            resolved_commit: None,
            config_digest: "config".into(),
            rules_digest: "rules".into(),
            task_contract_digest: Some("task".into()),
            task_contract_source: None,
            trust: "test".into(),
            changes: Vec::new(),
            source_reviews: BTreeMap::new(),
        },
        plan: PlanSummary {
            task_id: Some("task".into()),
            execution_order: vec!["rule".into()],
            required_checks: vec!["rule".into()],
            pending_delivery_checks: Vec::new(),
            acceptance: BTreeMap::new(),
            acceptance_descriptions: BTreeMap::new(),
        },
        gate: evaluate(&checks, &["rule".into()], &[]),
        summary: Summary::from_checks(&checks),
        checks,
        verification: VerificationBoundary::default(),
    }
}

#[test]
fn paired_producer_comparison_ignores_temporary_paths_but_rejects_binary_and_version_changes() {
    let mut left = report();
    left.checks[0].metadata.insert(
        "command_executable".into(),
        json!({"digest":"same","path":"first"}),
    );
    left.checks[0].metadata.insert(
        "tools".into(),
        json!([{"id":"compiler","executable":{"digest":"compiler"},"version":"1","inputs":{}}]),
    );
    let mut right = left.clone();
    right.checks[0]
        .metadata
        .get_mut("command_executable")
        .unwrap()["path"] = json!("second");
    assert!(matched_producers(
        &producer_environment(&left),
        &producer_environment(&right)
    ));
    right.checks[0].metadata.get_mut("tools").unwrap()[0]["version"] = json!("2");
    assert!(!matched_producers(
        &producer_environment(&left),
        &producer_environment(&right)
    ));
    right = left.clone();
    right.checks[0]
        .metadata
        .get_mut("command_executable")
        .unwrap()["digest"] = json!("changed");
    assert!(!matched_producers(
        &producer_environment(&left),
        &producer_environment(&right)
    ));
}

fn cases() -> Vec<EvaluatedCase> {
    [CaseKind::Replay, CaseKind::HeldOut, CaseKind::Anchor]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let mut baseline = Observation::from_report(
                &report(),
                &BTreeMap::from([("rule".into(), Expected::Pass)]),
                10,
            );
            baseline.report_ref = Some("baseline".into());
            let candidate = Observation::from_report(
                &report(),
                &BTreeMap::from([("rule".into(), Expected::Fail)]),
                12,
            );
            EvaluatedCase {
                id: index.to_string(),
                kind,
                base: "base".into(),
                head: "head".into(),
                snapshot_digest: Some("snapshot".into()),
                task_digest: "task".into(),
                baseline,
                candidate,
            }
        })
        .collect()
}

#[test]
fn independent_oracles_accept_intentional_failures_but_preserve_incomplete_and_inventory_gaps() {
    let mut report = report();
    assert!(report.gate.complete);
    assert_eq!(report.gate.decision, Decision::Fail);
    let expected = BTreeMap::from([("rule".into(), Expected::Fail)]);
    let observation = Observation::from_report(&report, &expected, 12);
    assert!(observation.complete && observation.mismatches.is_empty());
    assert_eq!(
        (
            observation.selected_rules,
            observation.activated_rules,
            observation.completed_checks
        ),
        (1, 1, 1)
    );
    assert_eq!(
        Observation::from_report(&report, &BTreeMap::new(), 0)
            .mismatches
            .len(),
        1
    );
    assert_eq!(
        Observation::from_report(
            &report,
            &BTreeMap::from([("rule".into(), Expected::Absent)]),
            0
        )
        .mismatches
        .len(),
        1
    );
    report.checks[0].block(ExecutionStatus::TimedOut, "timeout");
    report.gate = evaluate(&report.checks, &["rule".into()], &[]);
    assert!(!Observation::from_report(&report, &expected, 0).complete);
    report.plan.pending_delivery_checks.push("missing".into());
    report.gate.complete = true;
    assert!(!Observation::from_report(&report, &expected, 0).complete);
}

#[test]
fn completeness_regressions_and_minimum_improvements_are_separate_decisions() {
    let mut cases = cases();
    assert_eq!(decide(&cases, 3, 1, &[]).0, Conclusion::Pass);
    assert_eq!(decide(&cases, 3, 4, &[]).0, Conclusion::Block);
    cases[1].candidate.mismatches.push("regression".into());
    assert_eq!(decide(&cases, 3, 1, &[]).0, Conclusion::Block);
    cases[0].candidate = Observation::incomplete("missing".into());
    assert_eq!(decide(&cases, 3, 1, &[]).0, Conclusion::Incomplete);
    assert_eq!(decide(&cases[..2], 3, 1, &[]).0, Conclusion::Incomplete);
    assert_eq!(decide(&[], 0, 0, &[]).0, Conclusion::Incomplete);
    assert_eq!(
        decide(
            &super::tests::cases(),
            3,
            0,
            &["protected input changed".into()]
        )
        .0,
        Conclusion::Incomplete
    );
    for (conclusion, code) in [
        (Conclusion::Pass, 0),
        (Conclusion::Block, 1),
        (Conclusion::Incomplete, 2),
    ] {
        assert_eq!(conclusion.exit_code(), code);
    }
}

#[test]
fn effectiveness_separates_oracle_improvement_from_unknown_downstream_use_and_cost() {
    let mut run = EvaluationRun {
        schema_version: 1,
        candidate_id: "candidate".into(),
        candidate_revision: "revision".into(),
        candidate_policy: "candidate".into(),
        baseline_policy: "baseline".into(),
        suite_digest: "suite".into(),
        trust_digest: "trust".into(),
        evaluator_epoch: "epoch".into(),
        evaluator_digest: "evaluator".into(),
        environment_digest: "environment".into(),
        budget: EvaluationBudget {
            snapshot_max_mib: 16,
            snapshot_max_file_mib: 2,
            snapshot_jobs: 2,
            snapshot_timeout_seconds: 10,
            max_live_snapshot_mib: 64,
            max_parallel: 4,
            case_timeout_seconds: 10,
            total_timeout_seconds: 30,
        },
        jobs: 4,
        generation_actor: Actor {
            id: "agent".into(),
            kind: ActorKind::Agent,
        },
        evaluation_actor: Actor {
            id: "operator".into(),
            kind: ActorKind::Human,
        },
        started_at: 1,
        ended_at: 2,
        cases: cases(),
        conclusion: Conclusion::Pass,
        reasons: Vec::new(),
        verification: VerificationBoundary::default(),
    };
    let result = crate::domain::policy_effectiveness::measure("ref".into(), &run);
    assert_eq!(result.oracle_improvement, Some(3));
    assert_eq!(result.baseline.duration_ms, 30);
    assert_eq!(result.candidate.duration_ms, 36);
    assert_eq!(result.complete_pairs, 3);
    assert!(
        result.downstream_benefit.is_none()
            && result.review_effort_seconds.is_none()
            && result.context_tokens.is_none()
    );
    run.conclusion = Conclusion::Incomplete;
    run.cases[0].candidate.complete = false;
    let result = crate::domain::policy_effectiveness::measure("ref".into(), &run);
    assert!(result.oracle_improvement.is_none());
    assert_eq!(result.incomplete_pairs, 1);
}
