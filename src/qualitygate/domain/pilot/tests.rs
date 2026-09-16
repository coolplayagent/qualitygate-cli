use super::*;
use crate::domain::{CheckResult, Diagnostic, Report, Severity, Summary, evaluate};
use serde_json::{Value, json};

fn d(n: usize) -> String {
    format!("sha256:{n:064x}")
}
fn manifest() -> Manifest {
    serde_json::from_value(json!({"schema_version":1,"id":"fixture","protocol":{
        "project":"fixture","sampling":"all declared tasks","owner":"owner","reviewer":"reviewer","archive":"fixture archive",
        "sealed_at":1,"start_at":2,"end_at":604802,"task_count":1,"days":7,"max_attempts":3,"max_seconds":1800,
        "review_fraction_min":1.0,"monetary_cap":"fixture USD 5",
        "thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,"completion_min":0.95,
            "review_reduction_min":0.1,"full_p95_ratio_max":1.2,"cost_ratio_max":1.0}},"assignments":[],"observations":[]})).unwrap()
}
fn append(m: &mut Manifest, reports: &mut Reports, index: usize, workflow: Workflow, fail: bool) {
    let mut a = Assignment {
        input_id: "input".into(),
        id: format!("run-{index}"),
        task_id: "task".into(),
        task_kind: TaskKind::BugFix,
        origin: Origin::Real,
        cohort: Cohort {
            agent_version: "codex-fixture".into(),
            harness_digest: d(1),
            requested_model: "fixture-model".into(),
            actual_model: Some("fixture-model".into()),
            reasoning_effort: "medium".into(),
            workflow,
            environment_digest: d(2),
            tools_digest: d(3),
            cache: "cold".into(),
            permissions: "read-only".into(),
        },
        base: "a".repeat(40),
        initial_snapshot: d(4),
        config_digest: d(5),
        task_digest: d(6),
        required_checks: vec!["check".into()],
        expected_issues: Some(vec!["bug".into()]),
        eligible_repair: true,
        exclusion: None,
    };
    let mut check = CheckResult::pending("check", true, Severity::Error);
    if fail {
        check.diagnostics.push(Diagnostic {
            id: "bug".into(),
            fingerprint: "fp".into(),
            file: Some("src/lib.rs".into()),
            range: None,
            message: "fixture failure".into(),
            evidence: json!({}),
            fix: "repair".into(),
            recheck: Default::default(),
        });
    }
    check.complete();
    let report:Report=serde_json::from_value(json!({"schema_version":1,"run_id":format!("report-{index}"),"scope":"task","profile":"full",
        "environment_digest":d(2),"snapshot":{"mode":"worktree","base":a.base,"head":"WORKTREE","content_digest":d(100+index)},
        "policy":{"source":"fixture","config_digest":d(5),"rules_digest":d(7),"task_contract_digest":d(6),"trust":"selected","changes":[]},
        "plan":{"task_id":"task","required_checks":["check"],"pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[check.clone()],&a.required_checks,&[]),"checks":[check],"summary":Summary::default()})).unwrap();
    a.cohort.tools_digest = tool_inventory_digest(&report);
    let artifact = Artifact {
        path: format!("report-{index}.json"),
        digest: d(200 + index),
        bytes: 100,
    };
    let findings = if fail {
        vec![Finding {
            diagnostic_id: "canonical-bug".into(),
            issue_id: Some("bug".into()),
            report_digest: artifact.digest.clone(),
            check_id: "check".into(),
            fingerprint: "fp".into(),
            attribution: Attribution::QualitygateRule,
            review: Some(Review {
                actor: super::super::evolution::Actor {
                    id: "reviewer".into(),
                    kind: super::super::evolution::ActorKind::Human,
                },
                label: Label::Confirmed,
            }),
        }]
    } else {
        vec![]
    };
    reports.insert(artifact.digest.clone(), Ok(report));
    let observation = Observation {
        assignment_id: a.id.clone(),
        observed_at: 20,
        attempts: vec![Attempt {
            number: 1,
            status: AttemptStatus::Completed,
            elapsed_ms: 1000,
            check_elapsed_ms: Some(100),
            profile: "full".into(),
            snapshot_digest: d(100 + index),
            report: Some(artifact),
            usage: None,
            cost: Some(Cost {
                currency: "USD".into(),
                priced_at: 1,
                source: "fixture only".into(),
                model_micros: Some(10),
                infrastructure_micros: Some(5),
            }),
        }],
        findings,
        review_active_ms: Some(200),
        review_comments: Some(0),
        rework_rounds: Some(0),
    };
    m.assignments.push(a);
    m.observations.push(observation);
}
fn summary(m: &Manifest, r: &Reports) -> Value {
    summarize(m, r, 604803).unwrap()
}
fn group(m: &Manifest, r: &Reports) -> Value {
    summary(m, r)["groups"][0].clone()
}

#[test]
fn matched_conditions_compare_full_cost_and_human_review_without_authorizing_acceptance() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::ExistingTools, false);
    append(&mut m, &mut r, 1, Workflow::Qualitygate, false);
    m.observations[1].review_active_ms = Some(150);
    let s = summary(&m, &r);
    assert_eq!(s["complete"], true);
    assert_eq!(s["protocol_ready"], true);
    assert_eq!(s["comparisons"][0]["review_reduction"], 0.25);
    assert_eq!(s["comparisons"][0]["full_p95_ratio"], 1.0);
    assert_eq!(s["comparisons"][0]["cost_ratio"], 1.0);
    assert_eq!(s["trial_acceptance"], "requires_external_review");
    let g = &s["groups"][1];
    assert_eq!(g["repair"]["value"], 1.0);
    assert!(g["repeat_stability"]["value"].is_null());
    assert_eq!(g["cost"]["total_micros"], 15);
    assert_eq!(g["cost"]["per_accepted_task_micros"], 15.0);
    assert!(g["false_positive"]["value"].is_null());
    m.assignments[1].cohort.cache = "warm".into();
    assert_eq!(
        summary(&m, &r)["comparisons"][0]["matched_inventory"],
        false
    );
    m.assignments[1].cohort.cache = "cold".into();
    m.assignments[1].input_id = "extra".into();
    assert_eq!(
        summary(&m, &r)["comparisons"][0]["matched_inventory"],
        false
    );
}

#[test]
fn assigned_missing_failed_timeout_abandoned_and_excluded_runs_are_not_dropped() {
    let (mut m, mut r) = (manifest(), Reports::new());
    for i in 0..5 {
        append(&mut m, &mut r, i, Workflow::Qualitygate, false);
    }
    m.observations.pop();
    for (i, status) in [
        AttemptStatus::TimedOut,
        AttemptStatus::Abandoned,
        AttemptStatus::Failed,
    ]
    .into_iter()
    .enumerate()
    {
        m.observations[i].attempts[0].status = status;
        m.observations[i].attempts[0].report = None;
    }
    m.assignments[3].exclusion = Some("operator cancellation".into());
    let s = summary(&m, &r);
    let g = &s["groups"][0];
    assert_eq!(s["complete"], false);
    assert_eq!(g["assigned"], 5);
    assert_eq!(g["observed"], 4);
    assert_eq!(g["repair"]["denominator"], 5);
    assert_eq!(g["repair"]["numerator"], 0);
    assert!(g["repair"]["value"].is_null());
    assert_eq!(g["attempt_statuses"]["timed_out"], 1);
    assert_eq!(g["cost"]["known_model_micros"], 40);
    assert!(g["cost"]["total_micros"].is_null());
    m.assignments.pop();
    assert_eq!(group(&m, &r)["repair"]["value"], 0.0);
    assert_eq!(group(&m, &r)["cost"]["total_micros"], 60);
    m.observations[0].attempts.clear();
    assert_eq!(summary(&m, &r)["complete"], false);
}

#[test]
fn canonical_findings_deduplicate_and_unreviewed_diagnostics_remain_visible() {
    let (mut m, mut r) = (manifest(), Reports::new());
    for i in 0..2 {
        append(&mut m, &mut r, i, Workflow::Qualitygate, true);
    }
    let g = group(&m, &r);
    assert_eq!(g["diagnostics"]["distinct"], 1);
    assert_eq!(g["detection"]["value"], 1.0);
    assert_eq!(g["repair"]["value"], 0.0);
    assert_eq!(g["false_positive"]["value"], 0.0);
    m.observations[1].findings[0].review.as_mut().unwrap().label = Label::FalsePositive;
    assert!(
        summarize(&m, &r, 604803)
            .unwrap_err()
            .to_string()
            .contains("Conflicting")
    );
    m.observations.pop();
    m.assignments.pop();
    m.observations[0].findings[0].review.as_mut().unwrap().label = Label::FalsePositive;
    assert_eq!(group(&m, &r)["false_positive"]["value"], 1.0);
    assert_eq!(
        group(&m, &r)["threshold_observations"]["false_positive"],
        false
    );
    m.observations[0].findings.clear();
    let g = group(&m, &r);
    assert_eq!(g["diagnostics"]["unreviewed"], 1);
    assert_eq!(g["diagnostics"]["by_attribution"]["unclassified"], 1);
    assert!(g["threshold_observations"]["false_positive"].is_null());
}

#[test]
fn only_final_full_in_budget_changed_snapshot_counts_and_repeated_runs_must_all_pass() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::Qualitygate, false);
    append(&mut m, &mut r, 1, Workflow::Qualitygate, false);
    assert_eq!(group(&m, &r)["repeat_stability"]["value"], 1.0);
    m.observations[0].attempts[0].elapsed_ms = 1_800_001;
    assert_eq!(group(&m, &r)["repeat_stability"]["value"], 0.0);
    assert_eq!(group(&m, &r)["repair"]["value"], 0.5);
    m.observations[0].attempts[0].elapsed_ms = 1000;
    let mut attempt = m.observations[1].attempts.remove(0);
    attempt.number = 2;
    m.observations[0].attempts.push(attempt);
    m.observations.pop();
    m.assignments.pop();
    let last = &mut m.observations[0].attempts[1];
    last.profile = "quick".into();
    r.get_mut(&last.report.as_ref().unwrap().digest)
        .unwrap()
        .as_mut()
        .unwrap()
        .profile = "quick".into();
    let g = group(&m, &r);
    assert_eq!(g["first_attempt_success"]["value"], 1.0);
    assert_eq!(g["repair"]["value"], 0.0);
    assert_eq!(g["quick_ms"]["samples"], 1);
    last_full(&mut m, &mut r);
    m.protocol.max_attempts = 1;
    assert_eq!(group(&m, &r)["repair"]["value"], 0.0);
    m.protocol.max_attempts = 3;
    m.assignments[0].initial_snapshot = m.observations[0].attempts[1].snapshot_digest.clone();
    assert_eq!(group(&m, &r)["repair"]["value"], 0.0);
}
fn last_full(m: &mut Manifest, r: &mut Reports) {
    let last = m.observations[0].attempts.last_mut().unwrap();
    last.profile = "full".into();
    r.get_mut(&last.report.as_ref().unwrap().digest)
        .unwrap()
        .as_mut()
        .unwrap()
        .profile = "full".into();
}

#[test]
fn report_bindings_gate_consistency_missing_reports_and_unbound_findings_are_rejected() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::Qualitygate, false);
    let key = m.observations[0].attempts[0]
        .report
        .as_ref()
        .unwrap()
        .digest
        .clone();
    let original = r[&key].as_ref().unwrap().clone();
    for mutate in [
        |r: &mut Report| r.snapshot.base = "b".repeat(40),
        |r: &mut Report| r.plan.required_checks.push("extra".into()),
        |r: &mut Report| r.gate.complete = false,
        |r: &mut Report| r.checks[0].block(crate::domain::ExecutionStatus::ToolError, "failed"),
    ] {
        let mut bad = original.clone();
        mutate(&mut bad);
        r.insert(key.clone(), Ok(bad));
        assert_eq!(summary(&m, &r)["complete"], false);
    }
    r.insert(key.clone(), Err("digest changed".into()));
    assert_eq!(summary(&m, &r)["complete"], false);
    r.clear();
    assert_eq!(summary(&m, &r)["complete"], false);
    m.observations[0].attempts[0].report = None;
    assert_eq!(summary(&m, &r)["complete"], false);
    m.observations[0].attempts[0].status = AttemptStatus::Incomplete;
    assert_eq!(summary(&m, &r)["complete"], false);
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::Qualitygate, true);
    m.observations[0].findings[0].fingerprint = "invented".into();
    assert!(summarize(&m, &r, 604803).is_err());
    m.observations[0].findings[0].fingerprint = "fp".into();
    let duplicate = m.observations[0].findings[0].clone();
    m.observations[0].findings.push(duplicate);
    assert!(summarize(&m, &r, 604803).is_err());
}

#[test]
fn unknown_cost_models_period_truth_and_zero_denominators_are_never_zero_benefit() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::Qualitygate, false);
    m.assignments[0].expected_issues = None;
    m.assignments[0].cohort.actual_model = None;
    m.assignments[0].origin = Origin::ExtractedModule;
    m.observations[0].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .infrastructure_micros = None;
    m.observations[0].attempts[0].check_elapsed_ms = None;
    m.observations[0].review_active_ms = None;
    m.protocol.owner = None;
    m.protocol.task_count = 8;
    m.protocol.start_at = None;
    let s = summary(&m, &r);
    let g = &s["groups"][0];
    assert_eq!(s["protocol_ready"], false);
    assert!(s["limitations"].as_array().unwrap().len() >= 5);
    for metric in ["detection", "false_positive", "repeat_stability"] {
        assert!(g[metric]["value"].is_null());
    }
    assert!(g["cost"]["total_micros"].is_null());
    assert!(g["full_ms"]["p95"].is_null());
    m.protocol.start_at = Some(2);
    m.observations[0].attempts[0].cost = None;
    assert_eq!(group(&m, &r)["cost"]["unknown_attempts"], 1);
    assert_eq!(summary(&manifest(), &Reports::new())["complete"], false);
}

#[test]
fn cost_currency_price_and_inventory_mismatches_cannot_produce_comparative_savings() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::ExistingTools, false);
    append(&mut m, &mut r, 1, Workflow::Qualitygate, false);
    m.observations[1].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .currency = "EUR".into();
    assert!(summary(&m, &r)["comparisons"][0]["cost_ratio"].is_null());
    m.observations[1].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .currency = "USD".into();
    m.observations[1].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .priced_at = 2;
    assert!(summary(&m, &r)["comparisons"][0]["cost_ratio"].is_null());
    m.assignments[1].cohort.workflow = Workflow::ExistingTools;
    assert!(group(&m, &r)["cost"]["total_micros"].is_null());
    assert_eq!(group(&m, &r)["review_active_ms"]["median"], 200.0);
}

#[test]
fn strict_contract_rejects_malformed_duplicate_and_over_budget_observation_inventory() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::Qualitygate, true);
    let original = serde_json::to_value(&m).unwrap();
    let replacements = [
        ("/schema_version", json!(9)),
        ("/protocol/max_attempts", json!(0)),
        ("/protocol/days", json!(0)),
        ("/id", json!("")),
        ("/protocol/thresholds/detection_min", json!(1.1)),
        ("/protocol/thresholds/cost_ratio_max", json!(0)),
        ("/protocol/sealed_at", json!(3)),
        ("/protocol/end_at", json!(1)),
        ("/assignments/0/base", json!("HEAD")),
        ("/assignments/0/task_digest", json!("invalid")),
        ("/assignments/0/required_checks", json!([])),
        ("/assignments/0/expected_issues", json!(["bug", "bug"])),
        ("/observations/0/assignment_id", json!("unknown")),
        ("/observations/0/observed_at", json!(0)),
        ("/observations/0/rework_rounds", json!(u64::MAX)),
        ("/observations/0/attempts/0/number", json!(2)),
        ("/observations/0/attempts/0/profile", json!("unknown")),
        ("/observations/0/attempts/0/check_elapsed_ms", json!(2000)),
        (
            "/observations/0/attempts/0/report/bytes",
            json!(17 * 1024 * 1024),
        ),
        ("/observations/0/attempts/0/cost/priced_at", json!(0)),
        (
            "/observations/0/attempts/0/cost/model_micros",
            json!(u64::MAX),
        ),
        (
            "/observations/0/findings/0/review/actor/kind",
            json!("agent"),
        ),
        ("/observations/0/findings/0/issue_id", Value::Null),
    ];
    for (pointer, value) in replacements {
        let mut data = original.clone();
        *data.pointer_mut(pointer).unwrap() = value;
        let bad = serde_json::from_value(data).unwrap();
        assert!(validate(&bad).is_err(), "{pointer}");
    }
    let mut bad = m.clone();
    bad.assignments.push(bad.assignments[0].clone());
    assert!(validate(&bad).is_err());
    let mut bad = m.clone();
    bad.observations.push(bad.observations[0].clone());
    assert!(validate(&bad).is_err());
    append(&mut m, &mut r, 1, Workflow::Qualitygate, false);
    m.observations[1].attempts[0].report = m.observations[0].attempts[0].report.clone();
    assert!(validate(&m).is_err());
    m.observations[1].attempts[0].report = None;
    m.assignments[1].expected_issues = None;
    assert!(validate(&m).is_err());
    let mut data = original;
    data["unexpected"] = json!(true);
    assert!(serde_json::from_value::<Manifest>(data).is_err());
}
