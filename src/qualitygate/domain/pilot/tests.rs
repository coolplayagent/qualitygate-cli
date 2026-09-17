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
        initial_report: None,
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
        start_sequence: None,
        model_evidence: None,
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

fn sealable_manifest() -> Manifest {
    let (mut manifest, mut reports) = (manifest(), Reports::new());
    for (index, workflow) in [
        Workflow::ExistingTools,
        Workflow::Qualitygate,
        Workflow::ExistingTools,
        Workflow::Qualitygate,
    ]
    .into_iter()
    .enumerate()
    {
        append(&mut manifest, &mut reports, index, workflow, false);
        manifest.assignments[index].cohort.requested_model = if index < 2 {
            "medium-model".into()
        } else {
            "lower-model".into()
        };
        manifest.assignments[index].cohort.actual_model = None;
        manifest.assignments[index].expected_issues = Some(Vec::new());
    }
    manifest.observations.clear();
    manifest
}

fn structured_budget_plan() -> Manifest {
    let mut manifest = sealable_manifest();
    manifest.schema_version = 2;
    manifest.protocol.monetary_cap = None;
    manifest.protocol.budget = Some(PilotBudget {
        currency: "USD".into(),
        priced_at: 1,
        source: "predeclared fixture tariff".into(),
        max_total_micros: 10_000,
        human_hourly_micros: 3_600_000,
    });
    manifest
}

fn stratified_plan() -> Manifest {
    let mut manifest = structured_budget_plan();
    manifest.schema_version = 3;
    manifest.protocol.task_count = 2;
    manifest.protocol.task_mix = Some(
        [(TaskKind::BugFix, 1), (TaskKind::Refactor, 1)]
            .into_iter()
            .collect(),
    );
    let refactors: Vec<_> = manifest
        .assignments
        .iter()
        .cloned()
        .map(|mut assignment| {
            assignment.id.push_str("-refactor");
            assignment.input_id = "input-refactor".into();
            assignment.task_id = "refactor-task".into();
            assignment.task_kind = TaskKind::Refactor;
            assignment.task_digest = d(700);
            assignment.initial_snapshot = d(701);
            assignment
        })
        .collect();
    manifest.assignments.extend(refactors);
    manifest
}

fn sourced_plan() -> Manifest {
    let mut manifest = stratified_plan();
    manifest.schema_version = 4;
    manifest.sources = vec![
        TaskSource {
            input_id: "input".into(),
            kind: SourceKind::Issue,
            source_id: "fixture/issues/1".into(),
            path: "bug-source.json".into(),
            digest: d(800),
            bytes: 32,
            selected_at: 1,
        },
        TaskSource {
            input_id: "input-refactor".into(),
            kind: SourceKind::Commit,
            source_id: "fixture/commits/2".into(),
            path: "refactor-source.json".into(),
            digest: d(801),
            bytes: 32,
            selected_at: 1,
        },
    ];
    manifest
}

#[path = "v5_tests.rs"]
mod v5;
#[path = "v6_tests.rs"]
mod v6;
#[path = "v7_tests.rs"]
mod v7;

#[test]
fn v4_seal_binds_one_unique_preselected_source_per_independent_task() {
    let plan = sourced_plan();
    let sealed = seal(plan.clone(), 1).unwrap();
    let summary = summarize(&sealed, &Reports::new(), 1).unwrap();
    assert_eq!(summary["source_audit"]["distinct_inputs"], 2);
    assert_eq!(summary["source_audit"]["sources"][0]["kind"], "issue");

    let mut missing = plan.clone();
    missing.sources.pop();
    assert!(validate(&missing).is_err());
    let mut duplicate = plan.clone();
    duplicate.sources[1].source_id = duplicate.sources[0].source_id.clone();
    assert!(validate(&duplicate).is_err());
    let mut repeated_artifact = plan.clone();
    repeated_artifact.sources[1].digest = repeated_artifact.sources[0].digest.clone();
    assert!(validate(&repeated_artifact).is_err());
    let mut repeated_path = plan.clone();
    repeated_path.sources[1].path = repeated_path.sources[0].path.clone();
    assert!(validate(&repeated_path).is_err());
    let mut wrong_input = plan.clone();
    wrong_input.sources[1].input_id = "unassigned".into();
    assert!(validate(&wrong_input).is_err());
    let mut late = plan.clone();
    late.sources[1].selected_at = 2;
    assert!(validate(&late).is_err());
    let mut oversized = plan.clone();
    oversized.sources[1].bytes = 64 * 1024 + 1;
    assert!(validate(&oversized).is_err());
    let mut legacy = plan;
    legacy.schema_version = 3;
    assert!(validate(&legacy).is_err());

    let mut drifted = sealed;
    drifted.sources[1].source_id = "fixture/commits/changed".into();
    assert!(validate(&drifted).unwrap_err().to_string().contains("seal"));
}

#[test]
fn v3_seal_enforces_declared_strata_and_independent_task_identities() {
    let plan = stratified_plan();
    let sealed = seal(plan.clone(), 1).unwrap();
    let summary = summarize(&sealed, &Reports::new(), 1).unwrap();
    assert_eq!(summary["sampling_audit"]["distinct_inputs"], 2);
    assert_eq!(summary["sampling_audit"]["declared"]["bug_fix"], 1);
    assert_eq!(summary["sampling_audit"]["observed"]["refactor"], 1);
    assert_eq!(
        summary["sampling_audit"]["tasks"].as_array().unwrap().len(),
        2
    );

    let mut missing_mix = plan.clone();
    missing_mix.protocol.task_mix = None;
    assert!(validate(&missing_mix).is_err());
    let mut zero_mix = plan.clone();
    zero_mix
        .protocol
        .task_mix
        .as_mut()
        .unwrap()
        .insert(TaskKind::Refactor, 0);
    assert!(validate(&zero_mix).is_err());
    let mut wrong_sum = plan.clone();
    wrong_sum
        .protocol
        .task_mix
        .as_mut()
        .unwrap()
        .insert(TaskKind::Refactor, 2);
    assert!(validate(&wrong_sum).is_err());
    let mut legacy_mix = structured_budget_plan();
    legacy_mix.protocol.task_mix = plan.protocol.task_mix.clone();
    assert!(validate(&legacy_mix).is_err());
    let mut wrong_mix = plan.clone();
    for assignment in &mut wrong_mix.assignments[4..] {
        assignment.task_kind = TaskKind::BugFix;
    }
    assert!(
        validate(&wrong_mix)
            .unwrap_err()
            .to_string()
            .contains("task mix differs")
    );

    let mut duplicate_id = plan.clone();
    for assignment in &mut duplicate_id.assignments[4..] {
        assignment.task_id = "task".into();
    }
    assert!(
        validate(&duplicate_id)
            .unwrap_err()
            .to_string()
            .contains("reuse a task ID")
    );

    let mut duplicate_contract = plan;
    for assignment in &mut duplicate_contract.assignments[4..] {
        assignment.task_digest = d(6);
    }
    assert!(
        validate(&duplicate_contract)
            .unwrap_err()
            .to_string()
            .contains("reuse a task contract digest")
    );

    let mut drifted = sealed.clone();
    for assignment in &mut drifted.assignments[4..] {
        assignment.task_id = "changed-refactor-task".into();
    }
    assert!(validate(&drifted).unwrap_err().to_string().contains("seal"));

    let mut drifted_mix = sealed;
    drifted_mix
        .protocol
        .task_mix
        .as_mut()
        .unwrap()
        .insert(TaskKind::BugFix, 2);
    assert!(validate(&drifted_mix).is_err());
}

#[test]
fn v2_seal_requires_a_bounded_structured_budget_and_binds_it() {
    let mut plan = structured_budget_plan();
    assert!(seal(plan.clone(), 1).is_ok());
    plan.protocol.budget = None;
    assert!(validate(&plan).is_err());
    plan = structured_budget_plan();
    plan.protocol.budget.as_mut().unwrap().currency = "US$".into();
    assert!(validate(&plan).is_err());
    plan = structured_budget_plan();
    plan.protocol.budget.as_mut().unwrap().max_total_micros = 0;
    assert!(validate(&plan).is_err());
    plan = seal(structured_budget_plan(), 1).unwrap();
    plan.protocol.budget.as_mut().unwrap().max_total_micros += 1;
    assert!(
        validate(&plan)
            .unwrap_err()
            .to_string()
            .contains("differs from its pre-observation seal")
    );
}

#[test]
fn structured_budget_counts_failed_attempts_and_rejects_unknown_or_overspend() {
    let mut manifest = manifest();
    manifest.schema_version = 2;
    manifest.protocol.monetary_cap = None;
    manifest.protocol.budget = structured_budget_plan().protocol.budget;
    manifest.protocol.budget.as_mut().unwrap().max_total_micros = 250;
    let mut reports = Reports::new();
    append(&mut manifest, &mut reports, 0, Workflow::Qualitygate, false);
    manifest.observations[0].attempts[0].status = AttemptStatus::Failed;
    let assessed = budget::assess(&manifest).unwrap().unwrap();
    assert_eq!(assessed.known_total_micros, 215);
    assert_eq!(assessed.known_human_micros, 200);
    assert_eq!(assessed.status, ThresholdStatus::Met);

    manifest
        .protocol
        .budget
        .as_mut()
        .unwrap()
        .human_hourly_micros = 1;
    manifest.observations[0].review_active_ms = Some(1);
    assert_eq!(
        budget::assess(&manifest)
            .unwrap()
            .unwrap()
            .known_human_micros,
        1
    );
    manifest
        .protocol
        .budget
        .as_mut()
        .unwrap()
        .human_hourly_micros = 3_600_000;
    manifest.observations[0].review_active_ms = Some(200);

    manifest.protocol.budget.as_mut().unwrap().max_total_micros = 200;
    assert_eq!(
        budget::assess(&manifest).unwrap().unwrap().status,
        ThresholdStatus::NotMet
    );
    manifest.protocol.budget.as_mut().unwrap().max_total_micros = 250;
    manifest.observations[0].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .model_micros = None;
    let assessed = budget::assess(&manifest).unwrap().unwrap();
    assert_eq!(assessed.known_total_micros, 205);
    assert_eq!(assessed.unknown_inputs, 1);
    assert_eq!(assessed.status, ThresholdStatus::Unknown);

    manifest.observations[0].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .infrastructure_micros = Some(300);
    assert_eq!(
        budget::assess(&manifest).unwrap().unwrap().status,
        ThresholdStatus::NotMet
    );
    manifest.observations[0].attempts[0]
        .cost
        .as_mut()
        .unwrap()
        .currency = "EUR".into();
    assert_eq!(
        budget::assess(&manifest).unwrap().unwrap().status,
        ThresholdStatus::Unknown
    );
}

#[test]
fn pre_observation_seal_binds_the_plan_but_allows_observed_model_and_results() {
    let mut sealed = seal(sealable_manifest(), 1).unwrap();
    assert!(
        serde_json::to_value(&sealed).unwrap()["protocol"]
            .get("budget")
            .is_none()
    );
    let digest = sealed.plan_seal.as_ref().unwrap().digest.clone();
    assert_eq!(sealed.plan_seal.as_ref().unwrap().algorithm, "sha256");
    assert_eq!(seal(sealed.clone(), 1).unwrap().plan_seal, sealed.plan_seal);

    sealed.assignments[0].cohort.actual_model = Some("provider-version".into());
    assert!(validate(&sealed).is_ok());
    sealed.assignments[0].cohort.permissions = "changed".into();
    assert!(
        validate(&sealed)
            .unwrap_err()
            .to_string()
            .contains("differs from its pre-observation seal")
    );
    assert_eq!(sealed.plan_seal.unwrap().digest, digest);
}

#[test]
fn summary_accepts_only_verified_authorization_for_its_exact_plan() {
    let sealed = seal(sealable_manifest(), 1).unwrap();
    let subject = authorization_subject(&sealed).unwrap();
    let mut authorization = VerifiedPlanAuthorization {
        record: PlanAuthorizationRecord {
            schema_version: 1,
            record_id: "start-1".into(),
            subject,
            authorizer: super::super::evolution::Actor {
                id: "owner".into(),
                kind: super::super::evolution::ActorKind::Human,
            },
            reason: "fixture".into(),
            issued_at: 1,
            expires_at: 700_000,
        },
        signer_key_id: "owner".into(),
        public_key_digest: d(999),
    };
    let summary =
        summarize_with_authorization(&sealed, &Reports::new(), 1, Some(&authorization)).unwrap();
    assert_eq!(summary["plan_authorization"]["status"], "authenticated");
    authorization.record.subject.pilot_id = "other".into();
    assert!(
        summarize_with_authorization(&sealed, &Reports::new(), 1, Some(&authorization))
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
}

#[test]
fn seal_rejects_unready_governance_ground_truth_matrix_and_late_observations() {
    let mut plan = sealable_manifest();
    plan.protocol.reviewer = None;
    assert!(seal(plan, 1).unwrap_err().to_string().contains("reviewer"));

    let mut plan = sealable_manifest();
    plan.protocol.reviewer = plan.protocol.owner.clone();
    assert!(
        seal(plan, 1)
            .unwrap_err()
            .to_string()
            .contains("must be distinct")
    );

    let mut plan = sealable_manifest();
    for assignment in &mut plan.assignments {
        assignment.origin = Origin::Historical;
    }
    assert!(
        seal(plan, 1)
            .unwrap_err()
            .to_string()
            .contains("real inputs")
    );

    let mut plan = sealable_manifest();
    plan.assignments.pop();
    assert!(
        seal(plan, 1)
            .unwrap_err()
            .to_string()
            .contains("matrix is incomplete")
    );

    let mut plan = sealable_manifest();
    for assignment in &mut plan.assignments {
        assignment.expected_issues = None;
    }
    assert!(
        seal(plan, 1)
            .unwrap_err()
            .to_string()
            .contains("ground truth")
    );

    let mut plan = sealable_manifest();
    plan.observations.push(Observation {
        assignment_id: "run-0".into(),
        observed_at: 2,
        start_sequence: None,
        model_evidence: None,
        attempts: Vec::new(),
        findings: Vec::new(),
        review_active_ms: None,
        review_comments: None,
        rework_rounds: None,
    });
    assert!(
        seal(plan, 1)
            .unwrap_err()
            .to_string()
            .contains("before observations")
    );

    assert!(
        seal(sealable_manifest(), 3)
            .unwrap_err()
            .to_string()
            .contains("observation window")
    );
}

#[test]
fn matched_conditions_compare_full_cost_and_human_review_without_authorizing_acceptance() {
    let (mut m, mut r) = (manifest(), Reports::new());
    append(&mut m, &mut r, 0, Workflow::ExistingTools, false);
    append(&mut m, &mut r, 1, Workflow::Qualitygate, false);
    m.observations[1].review_active_ms = Some(150);
    let s = summary(&m, &r);
    assert_eq!(s["complete"], true);
    assert_eq!(s["protocol_ready"], false);
    assert!(s["plan_seal"].is_null());
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
