use super::*;
use crate::domain::pilot::ThresholdStatus;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;

fn subject(outcome: ThresholdStatus) -> PilotAcceptanceSubject {
    serde_json::from_value(json!({
        "repository":"owner/repository","pilot_id":"pilot-1",
        "plan_seal":{"algorithm":"sha256","digest":format!("sha256:{}","a".repeat(64))},
        "owner":"pilot-owner","reviewer":"pilot-reviewer","observation_end":800,
        "manifest_digest":format!("sha256:{}","b".repeat(64)),
        "start_authorization_digest":format!("sha256:{}","c".repeat(64)),
        "start_signer_key_id":"pilot-owner","start_public_key_digest":format!("sha256:{}","d".repeat(64)),
        "assessment":{"schema_version":1,"evaluator":"qualitygate-pilot-thresholds-v1",
            "metrics_digest":format!("sha256:{}","e".repeat(64)),"outcome":outcome,"checks":[]}
    }))
    .unwrap()
}

fn store(key: &SigningKey) -> TrustStore {
    serde_json::from_value(json!({
        "schema_version":1,"repository":"owner/repository","max_age_seconds":1000,
        "keys":[{"id":"pilot-reviewer","public_key":STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":[CHECK_SCOPE],"allow_repository_checks":true}],"revoked_records":[]
    }))
    .unwrap()
}

fn record(subject: &PilotAcceptanceSubject, decision: &str) -> PilotAcceptanceRecord {
    serde_json::from_value(json!({
        "schema_version":1,"record_id":"pilot-review-1","subject":subject,
        "reviewer":{"id":"pilot-reviewer","kind":"human"},"decision":decision,
        "reason":"Independent review of the retained evidence","issued_at":850,"expires_at":950
    }))
    .unwrap()
}

fn envelope(key: &SigningKey, record: &PilotAcceptanceRecord) -> Vec<u8> {
    let payload = serde_json::to_vec(record).unwrap();
    let signature = key.sign(&attestation::pae(PAYLOAD_TYPE, &payload));
    serde_json::to_vec(&json!({
        "payloadType":PAYLOAD_TYPE,"payload":STANDARD.encode(payload),
        "signatures":[{"keyid":"pilot-reviewer","sig":STANDARD.encode(signature.to_bytes())}]
    }))
    .unwrap()
}

#[test]
fn independent_reviewer_can_accept_only_met_exact_evidence() {
    let key = SigningKey::from_bytes(&[71; 32]);
    let expected = subject(ThresholdStatus::Met);
    let accepted = record(&expected, "accepted");
    let verified = verify(&envelope(&key, &accepted), &store(&key), &expected, 900).unwrap();
    assert_eq!(verified.signer_key_id, "pilot-reviewer");

    let unknown = subject(ThresholdStatus::Unknown);
    let invalid_acceptance = record(&unknown, "accepted");
    assert!(
        verify(
            &envelope(&key, &invalid_acceptance),
            &store(&key),
            &unknown,
            900,
        )
        .unwrap_err()
        .to_string()
        .contains("failed or unknown")
    );
    let rejected = record(&unknown, "rejected");
    assert!(verify(&envelope(&key, &rejected), &store(&key), &unknown, 900).is_ok());
}

#[test]
fn acceptance_rejects_early_foreign_and_revoked_review() {
    let key = SigningKey::from_bytes(&[72; 32]);
    let expected = subject(ThresholdStatus::Met);
    let mut early = record(&expected, "rejected");
    early.issued_at = 799;
    assert!(
        verify(&envelope(&key, &early), &store(&key), &expected, 900)
            .unwrap_err()
            .to_string()
            .contains("before the observation")
    );

    let record = record(&expected, "rejected");
    let mut revoked = store(&key);
    revoked.revoked_records.push(record.record_id.clone());
    assert!(
        verify(&envelope(&key, &record), &revoked, &expected, 900)
            .unwrap_err()
            .to_string()
            .contains("revoked")
    );

    let mut foreign = record;
    foreign.subject.manifest_digest = format!("sha256:{}", "f".repeat(64));
    assert!(
        verify(&envelope(&key, &foreign), &store(&key), &expected, 900)
            .unwrap_err()
            .to_string()
            .contains("evaluated evidence subject")
    );
}
