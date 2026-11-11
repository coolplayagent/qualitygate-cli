use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;

fn subject() -> PlanAuthorizationSubject {
    PlanAuthorizationSubject {
        repository: "owner/repository".into(),
        pilot_id: "pilot-1".into(),
        plan_seal: crate::domain::pilot::PlanSeal {
            algorithm: "sha256".into(),
            digest: format!("sha256:{}", "a".repeat(64)),
        },
        owner: "pilot-owner".into(),
        reviewer: "independent-reviewer".into(),
        sealed_at: 100,
        start_at: 200,
        end_at: 800,
        task_count: 8,
        assignment_count: 32,
    }
}

fn store(key: &SigningKey) -> TrustStore {
    serde_json::from_value(json!({
        "schema_version":1,"repository":"owner/repository","max_age_seconds":1000,
        "keys":[{"id":"pilot-owner","public_key":STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":[CHECK_SCOPE],"allow_repository_checks":true}],
        "revoked_records":[]
    }))
    .unwrap()
}

fn record(subject: &PlanAuthorizationSubject) -> PlanAuthorizationRecord {
    serde_json::from_value(json!({
        "schema_version":1,"record_id":"pilot-start-1","subject":subject,
        "authorizer":{"id":"pilot-owner","kind":"human"},
        "reason":"Start the sealed comparison","issued_at":150,"expires_at":900
    }))
    .unwrap()
}

fn envelope(key: &SigningKey, record: &PlanAuthorizationRecord) -> Vec<u8> {
    let payload = serde_json::to_vec(record).unwrap();
    let signature = key.sign(&attestation::pae(PAYLOAD_TYPE, &payload));
    serde_json::to_vec(&json!({
        "payloadType":PAYLOAD_TYPE,"payload":STANDARD.encode(payload),
        "signatures":[{"keyid":"pilot-owner","sig":STANDARD.encode(signature.to_bytes())}]
    }))
    .unwrap()
}

#[test]
fn owner_signature_binds_repository_plan_and_window() {
    let key = SigningKey::from_bytes(&[41; 32]);
    let subject = subject();
    let record = record(&subject);
    let verified = verify(&envelope(&key, &record), &store(&key), &subject, 300).unwrap();
    assert_eq!(verified.signer_key_id, "pilot-owner");
    assert_eq!(verified.record.subject.assignment_count, 32);

    let mut foreign = record.clone();
    foreign.subject.plan_seal.digest = format!("sha256:{}", "b".repeat(64));
    assert!(
        verify(&envelope(&key, &foreign), &store(&key), &subject, 300)
            .unwrap_err()
            .to_string()
            .contains("sealed plan subject")
    );

    let mut late = record.clone();
    late.issued_at = 201;
    assert!(
        verify(&envelope(&key, &late), &store(&key), &subject, 300)
            .unwrap_err()
            .to_string()
            .contains("before start")
    );
}

#[test]
fn authorization_rejects_expiry_revocation_and_nonhuman_owner() {
    let key = SigningKey::from_bytes(&[42; 32]);
    let subject = subject();
    let mut value = record(&subject);
    value.expires_at = 700;
    assert!(
        verify(&envelope(&key, &value), &store(&key), &subject, 300)
            .unwrap_err()
            .to_string()
            .contains("cover the observation window")
    );

    let value = record(&subject);
    let mut revoked = store(&key);
    revoked.revoked_records.push(value.record_id.clone());
    assert!(
        verify(&envelope(&key, &value), &revoked, &subject, 300)
            .unwrap_err()
            .to_string()
            .contains("revoked")
    );

    let mut agent = value;
    agent.authorizer.kind = ActorKind::Agent;
    assert!(
        verify(&envelope(&key, &agent), &store(&key), &subject, 300)
            .unwrap_err()
            .to_string()
            .contains("human owner")
    );
}
