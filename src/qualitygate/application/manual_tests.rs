use super::*;
use base64::Engine;
use serde_json::json;

fn load(root: &Path, store: &Path, directory: &Path, checks: &[CommandCheck]) -> Result<Inputs> {
    let requests = checks
        .iter()
        .filter_map(|check| check.evidence_file.as_ref())
        .map(|name| (name.clone(), attestation::MAX_ENVELOPE_BYTES))
        .collect();
    external::load(root, store, directory, &requests)
}

pub(in crate::application) fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Vec<CommandCheck>)
{
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let public = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
    std::fs::write(external.path().join("trust.json"), serde_json::to_vec(&json!({
        "schema_version":1,"repository":"example/project","max_age_seconds":3600,
        "keys":[{"id":"review","public_key":base64::engine::general_purpose::STANDARD.encode(public.as_bytes()),"checks":["manual"],"tasks":["release"],"allow_repository_checks":true}]
    })).unwrap()).unwrap();
    let checks = vec![
        serde_json::from_value(
            json!({"id":"manual","kind":"manual","evidence_file":"review.json"}),
        )
        .unwrap(),
    ];
    (root, external, checks)
}

#[test]
fn external_inputs_are_separate_bounded_and_revalidated() {
    let (root, external, checks) = fixture();
    let store = external.path().join("trust.json");
    let load_inputs = || load(root.path(), &store, external.path(), &checks);
    let missing = load_inputs().unwrap();
    assert!(record(&missing, &checks[0]).is_err());
    std::fs::write(external.path().join("review.json"), b"signed record").unwrap();
    let inputs = load_inputs().unwrap();
    unchanged(&inputs).unwrap();
    assert_eq!(record(&inputs, &checks[0]).unwrap(), b"signed record");
    std::fs::write(external.path().join("review.json"), b"changed").unwrap();
    assert!(unchanged(&inputs).is_err());
    std::fs::write(external.path().join("review.json"), b"signed record").unwrap();
    std::fs::write(&store, b"changed trust").unwrap();
    assert!(unchanged(&inputs).is_err());
    assert!(load_inputs().is_err());
    assert!(load(root.path(), &store, root.path(), &checks).is_err());
    std::fs::write(root.path().join("trust.json"), &inputs.store_bytes).unwrap();
    assert!(
        load(
            root.path(),
            &root.path().join("trust.json"),
            external.path(),
            &checks
        )
        .is_err()
    );
    std::fs::write(&store, &inputs.store_bytes).unwrap();
    std::fs::write(
        external.path().join("review.json"),
        vec![0; attestation::MAX_ENVELOPE_BYTES + 1],
    )
    .unwrap();
    assert!(record(&load_inputs().unwrap(), &checks[0]).is_err());
    let many: Vec<_> = (0..129)
        .map(|number| {
            let mut check = checks[0].clone();
            check.evidence_file = Some(format!("review-{number}.json"));
            check
        })
        .collect();
    assert!(load(root.path(), &store, external.path(), &many).is_err());
    let mut no_filename = checks[0].clone();
    no_filename.evidence_file = None;
    assert!(record(&inputs, &no_filename).is_err());
    let mut bulk = Vec::new();
    for number in 0..9 {
        let name = format!("bulk-{number}.json");
        std::fs::write(
            external.path().join(&name),
            vec![b' '; attestation::MAX_ENVELOPE_BYTES],
        )
        .unwrap();
        let mut check = checks[0].clone();
        check.id = format!("bulk-{number}");
        check.evidence_file = Some(name);
        bulk.push(check);
    }
    assert!(
        load(root.path(), &store, external.path(), &bulk)
            .err()
            .unwrap()
            .to_string()
            .contains("8 MiB")
    );
}

#[cfg(unix)]
#[test]
fn external_inputs_reject_symlinks_and_special_files() {
    let (root, external, checks) = fixture();
    let store = external.path().join("trust.json");
    std::os::unix::fs::symlink(&store, external.path().join("review.json")).unwrap();
    let inputs = load(root.path(), &store, external.path(), &checks).unwrap();
    assert!(record(&inputs, &checks[0]).is_err());
    std::os::unix::fs::symlink(&store, external.path().join("linked-trust.json")).unwrap();
    assert!(
        load(
            root.path(),
            &external.path().join("linked-trust.json"),
            external.path(),
            &checks
        )
        .is_err()
    );
    std::fs::remove_file(external.path().join("review.json")).unwrap();
    std::fs::create_dir(external.path().join("review.json")).unwrap();
    assert!(
        record(
            &load(root.path(), &store, external.path(), &checks).unwrap(),
            &checks[0]
        )
        .is_err()
    );
}

pub(in crate::application) fn signed_record(
    subject: ManualSubject,
    decision: ManualDecision,
) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let timestamp = now().unwrap();
    let record = ManualRecord {
        schema_version: 1,
        record_id: "review-1".into(),
        subject,
        reviewer: "independent-reviewer".into(),
        decision,
        reason: "Reviewed acceptance evidence".into(),
        issued_at: timestamp - 1,
        expires_at: timestamp + 600,
    };
    let payload = serde_json::to_vec(&record).unwrap();
    let signature = ed25519_dalek::SigningKey::from_bytes(&[7; 32])
        .sign(&attestation::pae(attestation::PAYLOAD_TYPE, &payload));
    let base64 = base64::engine::general_purpose::STANDARD;
    serde_json::to_vec(&json!({"payloadType":attestation::PAYLOAD_TYPE,
        "payload":base64.encode(payload),"signatures":[{"keyid":"review","sig":base64.encode(signature.to_bytes())}]})).unwrap()
}

#[tokio::test]
async fn signed_review_distinguishes_approval_rejection_and_invalid_evidence() {
    let (root, external, checks) = fixture();
    let snapshot = super::super::test_support::snapshot(root.path());
    let policy = super::super::test_support::policy();
    let config = crate::config::parse(b"schema_version: 1\n").unwrap();
    let mut plan = Plan::build(&config, None, "full").unwrap();
    plan.commands = checks.clone();
    plan.task_id = Some("release".into());
    plan.acceptance
        .insert("review-release".into(), "manual".into());
    let expected = subject(&checks[0], &plan, &snapshot, &policy, "example/project");
    assert_eq!(
        expected.task.as_ref().unwrap().acceptance_id,
        "review-release"
    );
    let artifacts = tempfile::tempdir().unwrap();
    let missing = execute(
        &checks[0],
        &plan,
        &snapshot,
        &policy,
        None,
        artifacts.path(),
    )
    .await;
    assert_eq!(missing.execution.status, ExecutionStatus::Blocked);
    for (decision, verdict) in [
        (ManualDecision::Approved, Verdict::Pass),
        (ManualDecision::Rejected, Verdict::Fail),
    ] {
        std::fs::write(
            external.path().join("review.json"),
            signed_record(expected.clone(), decision),
        )
        .unwrap();
        let inputs = Arc::new(
            load(
                root.path(),
                &external.path().join("trust.json"),
                external.path(),
                &checks,
            )
            .unwrap(),
        );
        let result = execute(
            &checks[0],
            &plan,
            &snapshot,
            &policy,
            Some(&inputs),
            artifacts.path(),
        )
        .await;
        assert_eq!(
            result.execution.status,
            ExecutionStatus::Completed,
            "{:?}",
            result.execution
        );
        assert_eq!(result.verdict, Some(verdict));
        assert_eq!(result.execution.artifacts.len(), 2);
        assert_eq!(result.metadata["manual_acceptance"]["verified"], true);
        let mut results = vec![result];
        revalidate(inputs.clone(), &plan, &snapshot, &policy, &mut results)
            .await
            .unwrap();
        assert_eq!(
            results[0].metadata["manual_acceptance"]["valid_at_completion"],
            true
        );
        let mut changed = policy.clone();
        changed.rules_digest = snapshot::digest(b"new rules");
        revalidate(inputs.clone(), &plan, &snapshot, &changed, &mut results)
            .await
            .unwrap();
        assert_eq!(results[0].execution.status, ExecutionStatus::Blocked);
        assert_eq!(
            results[0].metadata["manual_acceptance"]["valid_at_completion"],
            false
        );
        std::fs::write(external.path().join("review.json"), b"tampered").unwrap();
        assert!(
            revalidate(inputs, &plan, &snapshot, &policy, &mut results)
                .await
                .is_err()
        );
    }
    let inputs = Arc::new(
        load(
            root.path(),
            &external.path().join("trust.json"),
            external.path(),
            &checks,
        )
        .unwrap(),
    );
    let invalid = execute(
        &checks[0],
        &plan,
        &snapshot,
        &policy,
        Some(&inputs),
        artifacts.path(),
    )
    .await;
    assert_eq!(invalid.execution.status, ExecutionStatus::Blocked);
    assert_eq!(invalid.metadata["manual_acceptance"]["verified"], false);
    assert_eq!(invalid.execution.artifacts.len(), 2);
}
