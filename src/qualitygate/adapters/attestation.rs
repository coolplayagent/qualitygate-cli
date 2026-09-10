//! Verify a bounded Ed25519 DSSE envelope before interpreting its manual decision.

use crate::{
    config::attestation::TrustStore,
    domain::{ManualRecord, ManualSubject},
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.manual-acceptance.v1+json";
pub const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(rename = "payloadType")]
    payload_type: String,
    payload: String,
    signatures: Vec<EnvelopeSignature>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeSignature {
    #[serde(default)]
    keyid: String,
    sig: String,
}

#[derive(Debug, Serialize)]
pub struct VerifiedManual {
    pub record: ManualRecord,
    pub signer_key_id: String,
    pub public_key_digest: String,
}

fn decode(value: &str) -> Result<Vec<u8>> {
    for engine in [
        general_purpose::STANDARD,
        general_purpose::URL_SAFE,
        general_purpose::STANDARD_NO_PAD,
        general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(value) {
            return Ok(bytes);
        }
    }
    bail!("Invalid base64 encoding");
}

/// DSSE pre-authentication encoding binds both type and the exact payload bytes.
pub fn pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut encoded = format!(
        "DSSEv1 {} {} {} ",
        payload_type.len(),
        payload_type,
        payload.len()
    )
    .into_bytes();
    encoded.extend_from_slice(payload);
    encoded
}

pub fn validate_keys(store: &TrustStore) -> Result<()> {
    let mut keys = BTreeSet::new();
    for key in &store.keys {
        let bytes: [u8; 32] = decode(&key.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Ed25519 public keys must be 32 bytes"))?;
        let parsed = VerifyingKey::from_bytes(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid Ed25519 public key"))?;
        if parsed.is_weak() || !keys.insert(bytes) {
            bail!("Weak or duplicate Ed25519 public key");
        }
    }
    Ok(())
}

pub fn verify(
    bytes: &[u8],
    store: &TrustStore,
    expected: &ManualSubject,
    now: u64,
) -> Result<VerifiedManual> {
    let started = Instant::now();
    if bytes.len() > MAX_ENVELOPE_BYTES {
        bail!("Acceptance envelope exceeds 1 MiB");
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).context("Invalid acceptance envelope")?;
    if envelope.payload_type != PAYLOAD_TYPE || !(1..=16).contains(&envelope.signatures.len()) {
        bail!("Unsupported acceptance payload type or signature count (requires 1..16)");
    }
    let payload = decode(&envelope.payload)?;
    let message = pae(&envelope.payload_type, &payload);
    validate_keys(store)?;
    let mut signer = None;
    for key in &store.keys {
        if !key.checks.contains(&expected.check_id)
            || match &expected.task {
                Some(task) => !key.tasks.contains(&task.task_id),
                None => !key.allow_repository_checks,
            }
        {
            continue;
        }
        let public_key: [u8; 32] = decode(&key.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
        let verifier = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| anyhow::anyhow!("Invalid Ed25519 key"))?;
        for signature in &envelope.signatures {
            if started.elapsed() > Duration::from_secs(30) {
                bail!("Signature verification exceeded its 30-second budget");
            }
            if signature.keyid.len() > 256 || signature.sig.len() > 128 {
                bail!("Signature exceeds its size budget");
            }
            if !signature.keyid.is_empty() && signature.keyid != key.id {
                continue;
            }
            let signature = decode(&signature.sig)?;
            let signature = Signature::from_slice(&signature)
                .map_err(|_| anyhow::anyhow!("Invalid Ed25519 signature length"))?;
            if verifier.verify_strict(&message, &signature).is_ok() {
                signer = Some((
                    key.id.clone(),
                    format!("sha256:{:x}", Sha256::digest(public_key)),
                ));
                break;
            }
        }
        if signer.is_some() {
            break;
        }
    }
    let (signer_key_id, public_key_digest) =
        signer.context("No authorized signer verified the acceptance record")?;
    // Interpret the same authenticated bytes; never re-read the envelope payload.
    let record: ManualRecord =
        serde_json::from_slice(&payload).context("Invalid signed manual record")?;
    if record.schema_version != 1
        || record.subject != *expected
        || record.subject.repository != store.repository
    {
        bail!("Acceptance record does not match the repository, snapshot, policy, check or task");
    }
    for (value, limit) in [
        (&record.record_id, 256),
        (&record.reviewer, 1024),
        (&record.reason, 16_384),
    ] {
        if value.trim().is_empty() || value.len() > limit {
            bail!("Signed record ID, reviewer and reason must be nonempty and bounded");
        }
    }
    if store.revoked_records.contains(&record.record_id) {
        bail!("Acceptance record has been revoked");
    }
    if record.issued_at > now
        || record.expires_at <= now
        || record.expires_at <= record.issued_at
        || record.expires_at - record.issued_at > store.max_age_seconds
        || now - record.issued_at > store.max_age_seconds
    {
        bail!("Acceptance record is expired, future-dated or outside the allowed validity period");
    }
    Ok(VerifiedManual {
        record,
        signer_key_id,
        public_key_digest,
    })
}

#[cfg(test)]
#[path = "attestation_tests.rs"]
mod tests;
