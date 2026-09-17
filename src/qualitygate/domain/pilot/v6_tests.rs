use super::*;

fn report_for(assignment: &Assignment, snapshot: &str, run_id: &str, issues: &[&str]) -> Report {
    let mut check = CheckResult::pending("check", true, Severity::Error);
    for issue in issues {
        check.diagnostics.push(Diagnostic {
            id: (*issue).into(),
            fingerprint: (*issue).into(),
            file: Some("src/lib.rs".into()),
            range: None,
            message: "fixture failure".into(),
            evidence: json!({}),
            fix: "repair".into(),
            recheck: Default::default(),
        });
    }
    check.complete();
    serde_json::from_value(json!({"schema_version":1,"run_id":run_id,"scope":"task","profile":"full",
        "environment_digest":assignment.cohort.environment_digest,
        "snapshot":{"mode":"worktree","base":assignment.base,"head":"WORKTREE","content_digest":snapshot},
        "policy":{"source":"fixture","config_digest":assignment.config_digest,"rules_digest":d(7),
            "task_contract_digest":assignment.task_digest,"trust":"selected","changes":[]},
        "plan":{"task_id":assignment.task_id,"required_checks":assignment.required_checks,
            "pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[check.clone()],&assignment.required_checks,&[]),
        "checks":[check],"summary":Summary::default()}))
    .unwrap()
}

pub(super) fn plan() -> (Manifest, Reports) {
    let mut manifest = sourced_plan();
    manifest.schema_version = 6;
    manifest.protocol.no_progress_limit = Some(2);
    manifest.run_order = manifest.assignments.iter().map(|a| a.id.clone()).collect();
    let mut reports = Reports::new();
    for (index, assignment) in manifest.assignments.iter_mut().enumerate() {
        let report = report_for(
            assignment,
            &assignment.initial_snapshot,
            &format!("initial-{index}"),
            &["a", "b"],
        );
        assert_eq!(
            tool_inventory_digest(&report),
            assignment.cohort.tools_digest
        );
        let artifact = Artifact {
            path: format!("initial-{index}.json"),
            digest: d(1400 + index),
            bytes: 100,
        };
        reports.insert(artifact.digest.clone(), Ok(report));
        assignment.initial_report = Some(artifact);
    }
    (manifest, reports)
}

fn observe(manifest: &mut Manifest, reports: &mut Reports, issues: &[&[&str]], elapsed_ms: &[u64]) {
    let assignment = &manifest.assignments[0];
    let mut attempts = Vec::new();
    for (index, debt) in issues.iter().enumerate() {
        let snapshot = d(2000 + index);
        let report = report_for(
            assignment,
            &snapshot,
            &format!("attempt-{}", index + 1),
            debt,
        );
        let artifact = Artifact {
            path: format!("attempt-{index}.json"),
            digest: d(2100 + index),
            bytes: 100,
        };
        reports.insert(artifact.digest.clone(), Ok(report));
        attempts.push(Attempt {
            number: (index + 1) as u16,
            status: AttemptStatus::Completed,
            elapsed_ms: elapsed_ms[index],
            check_elapsed_ms: Some(100),
            profile: "full".into(),
            snapshot_digest: snapshot,
            report: Some(artifact),
            cost: None,
            usage: None,
            model_evidence: None,
            execution_evidence: None,
        });
    }
    manifest.observations.push(Observation {
        assignment_id: assignment.id.clone(),
        observed_at: 20,
        start_sequence: Some(1),
        model_evidence: None,
        attempts,
        findings: Vec::new(),
        review_active_ms: None,
        review_comments: None,
        rework_rounds: None,
    });
}

#[test]
fn v6_seal_binds_verified_initial_reports_and_progress_limit() {
    let (plan, reports) = plan();
    verify_initial_baselines(&plan, &reports).unwrap();
    let sealed = seal(plan.clone(), 1).unwrap();
    let mut drift = sealed.clone();
    drift.protocol.no_progress_limit = Some(1);
    assert!(validate(&drift).unwrap_err().to_string().contains("seal"));
    drift = sealed;
    drift.assignments[0].initial_report.as_mut().unwrap().digest = d(9000);
    assert!(validate(&drift).unwrap_err().to_string().contains("seal"));

    let mut missing = plan.clone();
    missing.assignments[0].initial_report = None;
    assert!(validate(&missing).is_err());
    let mut invalid_limit = plan.clone();
    invalid_limit.protocol.no_progress_limit = Some(0);
    assert!(validate(&invalid_limit).is_err());
    invalid_limit.protocol.no_progress_limit = Some(4);
    assert!(validate(&invalid_limit).is_err());
    let mut oversized = plan.clone();
    oversized.assignments[0]
        .initial_report
        .as_mut()
        .unwrap()
        .bytes = 16 * 1024 * 1024 + 1;
    assert!(validate(&oversized).is_err());
    let mut duplicate = plan.clone();
    duplicate.assignments[1].initial_report = duplicate.assignments[0].initial_report.clone();
    assert!(validate(&duplicate).is_err());
    let mut missing_report = reports.clone();
    missing_report.remove(&plan.assignments[0].initial_report.as_ref().unwrap().digest);
    assert!(verify_initial_baselines(&plan, &missing_report).is_err());
    let mut wrong_snapshot = reports.clone();
    wrong_snapshot
        .get_mut(&plan.assignments[0].initial_report.as_ref().unwrap().digest)
        .unwrap()
        .as_mut()
        .unwrap()
        .snapshot
        .content_digest = d(9999);
    assert!(verify_initial_baselines(&plan, &wrong_snapshot).is_err());
    let mut wrong_blockers = reports.clone();
    wrong_blockers
        .get_mut(&plan.assignments[0].initial_report.as_ref().unwrap().digest)
        .unwrap()
        .as_mut()
        .unwrap()
        .gate
        .blockers
        .clear();
    assert!(verify_initial_baselines(&plan, &wrong_blockers).is_err());

    let mut legacy = plan;
    legacy.schema_version = 5;
    legacy.protocol.no_progress_limit = None;
    assert!(validate(&legacy).is_err());
    for assignment in &mut legacy.assignments {
        assignment.initial_report = None;
    }
    legacy.protocol.no_progress_limit = Some(2);
    assert!(validate(&legacy).is_err());
}

#[test]
fn v6_audit_stops_after_two_unverified_reductions_and_retains_later_attempts() {
    let (mut manifest, mut reports) = plan();
    observe(
        &mut manifest,
        &mut reports,
        &[&["a", "c"], &["a", "c"], &[]],
        &[1000, 1000, 1000],
    );
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    let audit = &summary["attempt_audit"]["assignments"][0];
    assert_eq!(audit["stop_after_attempt"], 2);
    assert_eq!(audit["attempts"][0]["verified_progress"], false);
    assert_eq!(audit["attempts"][2]["verified_progress"], true);
    assert_eq!(
        audit["deviations"][0]["reason"],
        "continued_after_no_progress"
    );
    assert_eq!(summary["attempt_audit"]["status"], "deviated");
    assert_eq!(summary["protocol_ready"], false);

    manifest.observations[0].attempts.pop();
    let stopped = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(
        stopped["attempt_audit"]["assignments"][0]["status"],
        "matched"
    );
    assert_eq!(
        stopped["attempt_audit"]["assignments"][0]["stop_after_attempt"],
        2
    );
}

#[test]
fn v6_audit_resets_only_on_strict_debt_reduction_and_checks_both_budgets() {
    let (mut manifest, mut reports) = plan();
    observe(
        &mut manifest,
        &mut reports,
        &[&["a"], &["a"], &[]],
        &[1000, 1000, 1000],
    );
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    let audit = &summary["attempt_audit"]["assignments"][0];
    assert_eq!(audit["status"], "matched");
    assert_eq!(audit["attempts"][0]["verified_progress"], true);
    assert_eq!(audit["attempts"][1]["verified_progress"], false);
    assert_eq!(audit["attempts"][2]["verified_progress"], true);

    manifest.observations[0].attempts[0].elapsed_ms = 1_800_001;
    let overtime = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(overtime["attempt_audit"]["status"], "deviated");
    assert_eq!(
        overtime["attempt_audit"]["assignments"][0]["attempts"][0]["within_budget"],
        false
    );

    manifest.observations[0].attempts[0].elapsed_ms = 1000;
    let mut fourth = manifest.observations[0].attempts[2].clone();
    fourth.number = 4;
    fourth.report = None;
    fourth.snapshot_digest = d(3000);
    manifest.observations[0].attempts.push(fourth);
    let extra = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(extra["attempt_audit"]["status"], "deviated");
    assert_eq!(
        extra["attempt_audit"]["assignments"][0]["attempts"][3]["within_budget"],
        false
    );
    assert_eq!(extra["complete"], false);
}
