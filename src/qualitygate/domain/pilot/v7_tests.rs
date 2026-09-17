use super::*;

fn captured_plan() -> (Manifest, Reports) {
    let (mut manifest, reports) = super::v6::plan();
    manifest.schema_version = 7;
    (manifest, reports)
}

fn observation(manifest: &Manifest) -> Observation {
    let assignment = &manifest.assignments[0];
    Observation {
        assignment_id: assignment.id.clone(),
        observed_at: 20,
        start_sequence: Some(1),
        model_evidence: Some(ModelEvidence {
            artifact: Artifact {
                path: "model-capture.json".into(),
                digest: d(9999),
                bytes: 100,
            },
            capture: ModelCapture {
                assignment_id: assignment.id.clone(),
                captured_at: 10,
                agent_version: assignment.cohort.agent_version.clone(),
                harness_digest: assignment.cohort.harness_digest.clone(),
                requested_model: assignment.cohort.requested_model.clone(),
                reasoning_effort: assignment.cohort.reasoning_effort.clone(),
                actual_model: None,
                status: ModelIdentityStatus::Unknown,
                unknown_reason: Some("Provider did not disclose routing".into()),
            },
        }),
        attempts: Vec::new(),
        findings: Vec::new(),
        review_active_ms: None,
        review_comments: None,
        rework_rounds: None,
    }
}

#[test]
fn v7_preserves_plan_seal_and_audits_explicit_unknown_model() {
    let (plan, reports) = captured_plan();
    let mut sealed = seal(plan, 1).unwrap();
    sealed.observations.push(observation(&sealed));
    validate(&sealed).unwrap();
    let summary = summarize(&sealed, &reports, 604803).unwrap();
    assert_eq!(summary["model_audit"]["records"][0]["status"], "unknown");
    assert!(
        !summary["limitations"]
            .to_string()
            .contains("Actual model identity is unknown")
    );
    sealed.observations[0].model_evidence = None;
    validate(&sealed).unwrap();
    let summary = summarize(&sealed, &reports, 604803).unwrap();
    assert_eq!(summary["complete"], false);
    assert!(
        summary["limitations"]
            .to_string()
            .contains("lack archived model capture")
    );
}

#[test]
fn v7_rejects_mismatched_model_capture_and_legacy_field() {
    let (mut manifest, _) = captured_plan();
    manifest.observations.push(observation(&manifest));
    let mut invalid = manifest.clone();
    invalid.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .requested_model = "other".into();
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .unknown_reason = None;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .captured_at = 21;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .status = ModelIdentityStatus::Reported;
    assert!(validate(&invalid).is_err());
    manifest.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .status = ModelIdentityStatus::Reported;
    manifest.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .actual_model = Some("provider-model".into());
    manifest.observations[0]
        .model_evidence
        .as_mut()
        .unwrap()
        .capture
        .unknown_reason = None;
    manifest.assignments[0].cohort.actual_model = Some("provider-model".into());
    validate(&manifest).unwrap();
    manifest.schema_version = 6;
    assert!(validate(&manifest).is_err());
}
