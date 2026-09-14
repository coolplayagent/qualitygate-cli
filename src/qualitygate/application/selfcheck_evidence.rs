//! In-memory signed review fixtures use a public test key and no external trust.

use crate::{
    adapters::attestation,
    config,
    domain::{ManualRecord, ManualSubject},
};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};

pub(super) fn manual(
    record: &ManualRecord,
    expected: &ManualSubject,
    now: u64,
    tamper: bool,
) -> Result<Value> {
    // Public deterministic fixture key. These records never leave this
    // evaluator and must never be accepted as actual human review evidence.
    let key = SigningKey::from_bytes(&[7; 32]);
    let store = config::attestation::parse(&serde_json::to_vec(&json!({
        "schema_version":1, "repository":"fixture/repository", "max_age_seconds":3600,
        "keys":[{"id":"fixture", "public_key":STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":["manual"], "tasks":["fixture-task"], "allow_repository_checks":true}]
    }))?)?;
    let payload = serde_json::to_vec(record)?;
    let mut signature = key
        .sign(&attestation::pae(attestation::PAYLOAD_TYPE, &payload))
        .to_bytes();
    if tamper {
        signature[0] ^= 1;
    }
    let envelope = serde_json::to_vec(&json!({
        "payloadType":attestation::PAYLOAD_TYPE, "payload":STANDARD.encode(&payload),
        "signatures":[{"keyid":"fixture", "sig":STANDARD.encode(signature)}]
    }))?;
    Ok(
        match attestation::verify(&envelope, &store, expected, now) {
            Ok(verified) => json!({"status":"completed", "decision":verified.record.decision}),
            Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
        },
    )
}
