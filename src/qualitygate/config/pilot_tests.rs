use super::*;
use crate::domain::pilot::*;
use serde_json::json;

fn manifest() -> Manifest {
    serde_json::from_slice(br#"{"schema_version":1,"id":"qualitygate-eight-task-seven-day-pilot","protocol":{"project":"qualitygate-cli","sampling":"Before execution select four bug fixes and four refactors; assign each to both model groups and both workflows","owner":null,"reviewer":null,"archive":null,"sealed_at":null,"start_at":null,"end_at":null,"task_count":8,"days":7,"max_attempts":3,"max_seconds":1800,"review_fraction_min":1.0,"monetary_cap":null,"thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,"completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,"cost_ratio_max":1.0}},"assignments":[],"observations":[]}"#)
    .unwrap()
}

fn artifact(root: &Path, name: &str, bytes: &[u8]) -> crate::domain::Artifact {
    std::fs::write(root.join(name), bytes).unwrap();
    crate::domain::Artifact {
        path: name.into(),
        bytes: bytes.len() as u64,
        digest: crate::config::policy_store::digest(bytes),
    }
}

#[test]
fn draft_loading_is_read_only_and_invalid_inputs_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest.json");
    let bytes = serde_json::to_vec(&manifest()).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let (loaded, reports) = load(&path).unwrap();
    assert_eq!(loaded.schema_version, 1);
    assert!(reports.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(read(root.path(), 100).is_err());
    assert!(read(&path, 1).is_err());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(load(&path).is_err());
    std::fs::write(&path, vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert!(load_manifest(&path).is_err());
    #[cfg(unix)]
    {
        let link = root.path().join("link.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(load_manifest(&link).is_err());
    }
}

#[test]
fn model_artifacts_must_match_retained_bytes_and_claimed_capture_for_each_schema() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest.json");
    let capture: ModelCapture = serde_json::from_value(json!({"assignment_id":"run-1","attempt_number":1,
        "captured_at":10,"agent_version":"agent","harness_digest":crate::config::policy_store::digest(b"harness"),
        "requested_model":"model","reasoning_effort":"medium","actual_model":"model","status":"reported","unknown_reason":null})).unwrap();
    let evidence = ModelEvidence {
        artifact: artifact(
            root.path(),
            "model.json",
            &serde_json::to_vec(&capture).unwrap(),
        ),
        capture,
    };
    let observation: Observation = serde_json::from_value(json!({"assignment_id":"run-1","observed_at":11,
        "model_evidence":evidence,"attempts":[{"number":1,"status":"completed","elapsed_ms":100,
        "profile":"full","snapshot_digest":crate::config::policy_store::digest(b"snapshot"),"model_evidence":evidence}],"findings":[]})).unwrap();
    for version in [7, 8, 9, 10] {
        let mut m = manifest();
        m.schema_version = version;
        m.observations.push(observation.clone());
        verify_model_evidence(&path, &m).unwrap();
        let selected = if version == 7 {
            m.observations[0].model_evidence.as_mut().unwrap()
        } else {
            m.observations[0].attempts[0]
                .model_evidence
                .as_mut()
                .unwrap()
        };
        selected.capture.actual_model = Some("different-model".into());
        assert!(
            verify_model_evidence(&path, &m)
                .unwrap_err()
                .to_string()
                .contains("differs from the manifest claim")
        );
    }
    let mut m = manifest();
    m.schema_version = 8;
    m.observations.push(observation);
    let saved = m.observations[0].attempts[0]
        .model_evidence
        .clone()
        .unwrap();
    for change in 0..4 {
        let selected = m.observations[0].attempts[0]
            .model_evidence
            .as_mut()
            .unwrap();
        *selected = saved.clone();
        match change {
            0 => selected.artifact.digest = crate::config::policy_store::digest(b"wrong"),
            1 => selected.artifact.bytes += 1,
            2 => selected.artifact.path = "../outside.json".into(),
            3 => selected.artifact.bytes = 8 * 1024 * 1024,
            _ => unreachable!(),
        }
        assert!(verify_model_evidence(&path, &m).is_err(), "change {change}");
    }
}

#[test]
fn execution_artifact_cannot_claim_different_outcome_or_outlive_its_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest.json");
    let capture: ExecutionCapture = serde_json::from_value(json!({"assignment_id":"run-1","attempt_number":1,
        "started_at_ms":100,"ended_at_ms":200,"harness_digest":crate::config::policy_store::digest(b"harness"),
        "status":"completed","snapshot_digest":crate::config::policy_store::digest(b"snapshot"),"report_digest":null})).unwrap();
    let evidence = ExecutionEvidence {
        artifact: artifact(
            root.path(),
            "execution.json",
            &serde_json::to_vec(&capture).unwrap(),
        ),
        capture,
    };
    let mut m = manifest();
    m.schema_version = 9;
    m.observations.push(serde_json::from_value(json!({"assignment_id":"run-1","observed_at":11,
        "attempts":[{"number":1,"status":"completed","elapsed_ms":100,"profile":"full",
        "snapshot_digest":crate::config::policy_store::digest(b"snapshot"),"execution_evidence":evidence}],"findings":[]})).unwrap());
    verify_execution_evidence(&path, &m).unwrap();
    m.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .capture
        .ended_at_ms += 1;
    assert!(
        verify_execution_evidence(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("differs from the manifest claim")
    );
    m.observations[0].attempts[0].execution_evidence = Some(evidence.clone());
    std::fs::write(root.path().join("execution.json"), b"{}").unwrap();
    assert!(verify_execution_evidence(&path, &m).is_err());
    m.observations[0].attempts[0]
        .execution_evidence
        .as_mut()
        .unwrap()
        .artifact
        .bytes = 8 * 1024 * 1024;
    assert!(
        verify_execution_evidence(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("exceeds 8 MiB")
    );
}

#[test]
fn source_and_initial_report_inventories_detect_substitution_missing_files_and_budget_overflow() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest.json");
    let source = artifact(root.path(), "issue.txt", b"Observed bug and reproduction");
    let mut m = manifest();
    m.sources.push(serde_json::from_value(json!({"input_id":"issue-1","kind":"issue","source_id":"tracker/1","path":source.path,"digest":source.digest,"bytes":source.bytes,"selected_at":1})).unwrap());
    verify_sources(&path, &m).unwrap();
    m.sources[0].digest = crate::config::policy_store::digest(b"different source");
    assert!(
        verify_sources(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("digest differs")
    );
    m.sources[0].path = "missing.txt".into();
    assert!(verify_sources(&path, &m).is_err());
    m.sources.clear();
    let d = crate::config::policy_store::digest(b"identity");
    let initial = artifact(root.path(), "initial.json", b"{}");
    m.assignments.push(serde_json::from_value(json!({"id":"run","input_id":"input","task_id":"task","task_kind":"bug_fix","origin":"real",
        "cohort":{"agent_version":"agent","harness_digest":d,"requested_model":"model","actual_model":null,"reasoning_effort":"medium","workflow":"qualitygate","environment_digest":d,"tools_digest":d,"cache":"cold","permissions":"read"},
        "base":"a".repeat(40),"initial_snapshot":d,"initial_report":initial,"config_digest":d,"task_digest":d,"required_checks":["behavior"],"expected_issues":[],"eligible_repair":true,"exclusion":null})).unwrap());
    m.schema_version = 6;
    verify_initial_reports(&path, &m).unwrap();
    m.assignments[0].initial_report.as_mut().unwrap().digest = d;
    assert!(
        verify_initial_reports(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("digest differs")
    );
    m.assignments[0].initial_report.as_mut().unwrap().bytes = 64 * 1024 * 1024;
    assert!(
        verify_initial_reports(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("64 MiB")
    );
    m.assignments[0].initial_report = None;
    assert!(
        verify_initial_reports(&path, &m)
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
}
