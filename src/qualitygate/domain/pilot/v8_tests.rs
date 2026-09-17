use super::*;

pub(super) fn plan() -> (Manifest, Reports) {
    let (mut manifest, reports) = super::v6::plan();
    manifest.schema_version = 8;
    (manifest, reports)
}

fn attempt(number: u16, status: AttemptStatus) -> Attempt {
    Attempt {
        number,
        status,
        elapsed_ms: 1000,
        check_elapsed_ms: None,
        profile: "full".into(),
        snapshot_digest: d(2000 + usize::from(number)),
        report: None,
        cost: None,
        usage: None,
        model_evidence: None,
        execution_evidence: None,
    }
}

fn evidence(assignment: &Assignment, number: u16, model: Option<&str>) -> ModelEvidence {
    ModelEvidence {
        artifact: Artifact {
            path: format!("model-{number}.json"),
            digest: d(3000 + usize::from(number)),
            bytes: 100,
        },
        capture: ModelCapture {
            assignment_id: assignment.id.clone(),
            attempt_number: Some(number),
            captured_at: 10 + u64::from(number),
            agent_version: assignment.cohort.agent_version.clone(),
            harness_digest: assignment.cohort.harness_digest.clone(),
            requested_model: assignment.cohort.requested_model.clone(),
            reasoning_effort: assignment.cohort.reasoning_effort.clone(),
            actual_model: model.map(str::to_owned),
            status: if model.is_some() {
                ModelIdentityStatus::Reported
            } else {
                ModelIdentityStatus::Unknown
            },
            unknown_reason: model
                .is_none()
                .then(|| "Provider did not report routing".into()),
        },
    }
}

pub(super) fn observed(manifest: &mut Manifest) {
    let assignment = &manifest.assignments[0];
    let mut first = attempt(1, AttemptStatus::Failed);
    first.model_evidence = Some(evidence(assignment, 1, Some("model-A")));
    let mut second = attempt(2, AttemptStatus::TimedOut);
    second.model_evidence = Some(evidence(assignment, 2, None));
    manifest.observations.push(Observation {
        assignment_id: assignment.id.clone(),
        observed_at: 20,
        start_sequence: Some(1),
        model_evidence: None,
        attempts: vec![first, second],
        findings: Vec::new(),
        review_active_ms: None,
        review_comments: None,
        rework_rounds: None,
    });
}

#[test]
fn v8_audits_every_attempt_including_failed_and_unknown() {
    let (plan, reports) = plan();
    let mut sealed = seal(plan, 1).unwrap();
    observed(&mut sealed);
    validate(&sealed).unwrap();
    let summary = summarize(&sealed, &reports, 604803).unwrap();
    assert_eq!(
        summary["model_audit"]["records"][0]["attempt_status"],
        "failed"
    );
    assert_eq!(
        summary["model_audit"]["records"][1]["attempt_status"],
        "timed_out"
    );
    assert_eq!(summary["model_audit"]["actual_identity_unknown"], 1);
    assert_eq!(summary["model_audit"]["missing_attempts"], 0);
    assert_eq!(summary["model_audit"]["drifted_assignments"], json!([]));

    sealed.observations[0].attempts[1].model_evidence = None;
    validate(&sealed).unwrap();
    let summary = summarize(&sealed, &reports, 604803).unwrap();
    assert_eq!(summary["model_audit"]["missing_attempts"], 1);
    assert_eq!(
        summary["model_audit"]["records"][1]["attempt_status"],
        "timed_out"
    );
    assert_eq!(
        summary["model_audit"]["records"][1]["requested_model"],
        sealed.assignments[0].cohort.requested_model
    );
    assert_eq!(summary["complete"], false);
    assert!(
        summary["limitations"]
            .to_string()
            .contains("Observed attempts lack")
    );
}

#[test]
fn v8_blocks_reported_route_drift_and_validates_actual_identity() {
    let (mut manifest, reports) = plan();
    observed(&mut manifest);
    manifest.observations[0].attempts[1]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture = evidence(&manifest.assignments[0], 2, Some("model-B")).capture;
    validate(&manifest).unwrap();
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(
        summary["model_audit"]["drifted_assignments"],
        json!(["run-0"])
    );
    assert!(
        summary["limitations"]
            .to_string()
            .contains("changed across attempts")
    );

    manifest.observations[0].attempts[1]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .actual_model = Some("model-A".into());
    manifest.assignments[0].cohort.actual_model = Some("model-A".into());
    validate(&manifest).unwrap();
    manifest.assignments[0].cohort.actual_model = None;
    assert!(
        validate(&manifest)
            .unwrap_err()
            .to_string()
            .contains("actual model")
    );
}

#[test]
fn v8_rejects_wrong_attempt_claim_and_legacy_evidence() {
    let (mut manifest, _) = plan();
    observed(&mut manifest);
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .attempt_number = Some(2);
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .requested_model = "different".into();
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .captured_at = 21;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[1]
        .model_evidence
        .as_mut()
        .unwrap()
        .artifact
        .digest = invalid.observations[0].attempts[0]
        .model_evidence
        .as_ref()
        .unwrap()
        .artifact
        .digest
        .clone();
    assert!(validate(&invalid).is_err());

    let mut old = manifest.clone();
    old.schema_version = 7;
    assert!(
        validate(&old)
            .unwrap_err()
            .to_string()
            .contains("schema v8")
    );
    let mut invalid = manifest;
    invalid.observations[0].model_evidence =
        invalid.observations[0].attempts[0].model_evidence.clone();
    assert!(validate(&invalid).is_err());
}

#[test]
fn v8_requires_post_seal_and_observed_actual_model_claims() {
    let (mut manifest, _) = plan();
    manifest.assignments[0].cohort.actual_model = Some("premature".into());
    assert!(
        seal(manifest.clone(), 1)
            .unwrap_err()
            .to_string()
            .contains("Unobserved")
    );
    assert!(
        validate(&manifest)
            .unwrap_err()
            .to_string()
            .contains("Unobserved")
    );
}
