use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::Signer;
use serde_json::json;

#[tokio::test]
async fn signed_agent_origin_is_required_only_for_declared_scope_and_revalidated_at_completion() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut input = crate::application::test_support::snapshot(root.path());
    let bytes = b"def test_behavior():\n    assert True\n";
    input.files.insert(
        "tests/test_behavior.py".into(),
        snapshot::File {
            bytes: bytes.to_vec(),
            executable: false,
        },
    );
    input.identity.content_digest = snapshot::content_digest(&input.files);
    let input = Arc::new(input);
    let policy = crate::application::test_support::policy();
    let config = crate::config::parse(b"schema_version: 1\n").unwrap();
    let catalog = crate::config::catalog::Catalog::load(&config, []).unwrap();
    let entry = &catalog.entries["line-ending"];
    let mut setting = RuleSetting::default();
    assert!(
        prepare(
            "trace",
            &setting,
            entry,
            &input,
            &policy,
            None,
            artifacts.path()
        )
        .await
        .unwrap()
        .is_none()
    );
    setting
        .parameters
        .insert("provenance_scope".into(), json!("ai_only"));
    let missing = prepare(
        "trace",
        &setting,
        entry,
        &input,
        &policy,
        None,
        artifacts.path(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(missing.execution.status, ExecutionStatus::Blocked);
    let now = external::now().unwrap();
    let record = json!({"schema_version":1,"record_id":"origin-1","coverage":"complete_comparison",
        "subject":provenance::subject(&input,&policy,"unit/repo","trace"),"issued_at":now,"expires_at":now+600,
        "runs":[{"run_id":"agent-1","actor":{"kind":"agent","id":"unit-agent","version":"1"},"started_at":now-2,"ended_at":now-1}],
        "steps":[{"run_id":"agent-1","input_content_digest":snapshot::content_digest(&input.base_files),"output_content_digest":input.identity.content_digest,
            "changes":[{"path":"tests/test_behavior.py","before":null,"after":{"content_base64":STANDARD.encode(bytes),"executable":false}}]}]});
    let key = ed25519_dalek::SigningKey::from_bytes(&[29; 32]);
    let payload = serde_json::to_vec(&record).unwrap();
    let signature = key.sign(&crate::adapters::attestation::pae(
        provenance::PAYLOAD_TYPE,
        &payload,
    ));
    std::fs::write(outside.path().join("origin.json"),serde_json::to_vec(&json!({"payloadType":provenance::PAYLOAD_TYPE,
        "payload":STANDARD.encode(payload),"signatures":[{"keyid":"harness","sig":STANDARD.encode(signature.to_bytes())}]})).unwrap()).unwrap();
    let trust = outside.path().join("trust.json");
    std::fs::write(&trust,serde_json::to_vec(&json!({"schema_version":1,"repository":"unit/repo","max_age_seconds":3600,
        "keys":[{"id":"harness","public_key":STANDARD.encode(key.verifying_key().as_bytes()),"provenance_rules":["trace"]}]})).unwrap()).unwrap();
    let inputs = Arc::new(
        external::load(
            root.path(),
            &trust,
            outside.path(),
            &[("origin.json".into(), provenance::MAX_ENVELOPE_BYTES)].into(),
        )
        .unwrap(),
    );
    assert!(
        prepare(
            "trace",
            &setting,
            entry,
            &input,
            &policy,
            Some(&inputs),
            artifacts.path()
        )
        .await
        .is_err()
    );
    setting.provenance =
        Some(serde_json::from_value(json!({"evidence_file":"origin.json"})).unwrap());
    let prepared = prepare(
        "trace",
        &setting,
        entry,
        &input,
        &policy,
        Some(&inputs),
        artifacts.path(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(prepared.artifacts.len(), 2);
    assert_eq!(prepared.metadata["verified"], true);
    let parsed = crate::adapters::syntax::parse("tests/test_behavior.py", bytes)
        .unwrap()
        .unwrap();
    assert!(
        prepared
            .facts
            .participated("tests/test_behavior.py", &parsed.tests[0])
    );
    let mut result = CheckResult::pending("trace", true, Severity::Error);
    result.complete();
    result
        .metadata
        .insert("external_provenance".into(), prepared.metadata);
    let proofs = [("trace".into(), prepared.facts)].into();
    revalidate(&inputs, &proofs, std::slice::from_mut(&mut result)).unwrap();
    assert_eq!(
        result.metadata["external_provenance"]["valid_at_completion"],
        true
    );
    let mut revoked = external::load(
        root.path(),
        &trust,
        outside.path(),
        &[("origin.json".into(), provenance::MAX_ENVELOPE_BYTES)].into(),
    )
    .unwrap();
    revoked.store.revoked_records.push("origin-1".into());
    revalidate(&revoked, &proofs, std::slice::from_mut(&mut result)).unwrap();
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert_eq!(
        result.metadata["external_provenance"]["valid_at_completion"],
        false
    );
    let mut changed = policy;
    changed.rules_digest = snapshot::digest(b"different rules");
    assert!(
        prepare(
            "trace",
            &setting,
            entry,
            &input,
            &changed,
            Some(&inputs),
            artifacts.path()
        )
        .await
        .is_err()
    );
}
