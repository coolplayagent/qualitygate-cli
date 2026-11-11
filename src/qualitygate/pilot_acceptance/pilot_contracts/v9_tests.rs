use super::*;

fn observed() -> (Manifest, Reports) {
    let (mut manifest, reports) = super::v8::plan();
    manifest.schema_version = 9;
    let mut manifest = seal(manifest, 1).unwrap();
    super::v8::observed(&mut manifest);
    let assignment = &manifest.assignments[0];
    for (index, attempt) in manifest.observations[0].attempts.iter_mut().enumerate() {
        let started_at_ms = 10_000 + index as u64 * 1_000;
        attempt.execution_evidence = Some(ExecutionEvidence {
            artifact: Artifact {
                path: format!("execution-{}.json", attempt.number),
                digest: d(4000 + index),
                bytes: 100,
            },
            capture: ExecutionCapture {
                assignment_id: assignment.id.clone(),
                attempt_number: attempt.number,
                started_at_ms,
                ended_at_ms: started_at_ms + attempt.elapsed_ms,
                harness_digest: assignment.cohort.harness_digest.clone(),
                status: attempt.status.clone(),
                snapshot_digest: attempt.snapshot_digest.clone(),
                report_digest: None,
            },
        });
    }
    (manifest, reports)
}

#[test]
fn v9_audits_failed_and_timed_out_attempts_and_missing_receipts() {
    let (mut manifest, reports) = observed();
    validate(&manifest).unwrap();
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(summary["execution_audit"]["records"][0]["status"], "failed");
    assert_eq!(
        summary["execution_audit"]["records"][1]["status"],
        "timed_out"
    );
    assert_eq!(summary["execution_audit"]["missing_attempts"], 0);
    manifest.observations[0].attempts[1].execution_evidence = None;
    validate(&manifest).unwrap();
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(summary["execution_audit"]["missing_attempts"], 1);
    assert_eq!(summary["complete"], false);
    assert!(
        summary["limitations"]
            .to_string()
            .contains("timing evidence")
    );
}

#[test]
fn v9_rejects_mismatched_receipts_and_legacy_field() {
    let (manifest, _) = observed();
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .ended_at_ms += 1;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .status = AttemptStatus::Completed;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .harness_digest = d(9001);
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .report_digest = Some(d(8));
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[1]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .started_at_ms = 10_500;
    assert!(validate(&invalid).is_err());
    let mut invalid = manifest.clone();
    invalid.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .started_at_ms = 1;
    assert!(validate(&invalid).is_err());
    let mut old = manifest;
    old.schema_version = 8;
    assert!(
        validate(&old)
            .unwrap_err()
            .to_string()
            .contains("schema v9")
    );
}

#[test]
fn v9_receipt_duration_feeds_the_sealed_time_budget() {
    let (mut manifest, reports) = observed();
    let attempt = &mut manifest.observations[0].attempts[1];
    attempt.elapsed_ms = 1_800_001;
    attempt
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .ended_at_ms = 11_000 + attempt.elapsed_ms;
    manifest.observations[0].observed_at = 1812;
    validate(&manifest).unwrap();
    let summary = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(
        summary["execution_audit"]["records"][1]["elapsed_ms"],
        1_800_001
    );
    assert_eq!(summary["attempt_audit"]["status"], "deviated");
    assert!(
        summary["limitations"]
            .to_string()
            .contains("Attempt budget")
    );
}
