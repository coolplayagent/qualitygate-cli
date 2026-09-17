//! Authenticate an independent reviewer's final pilot decision.

use super::attestation::{self, Authenticated};
use crate::{
    config::attestation::TrustStore,
    domain::{
        evolution::ActorKind,
        pilot::{
            PilotAcceptanceRecord, PilotAcceptanceSubject, VerifiedPilotAcceptance,
            acceptance_decision,
        },
    },
};
use anyhow::{Context, Result, bail};

pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.pilot-trial-acceptance.v1+json";
pub const CHECK_SCOPE: &str = "pilot-trial-acceptance";
pub const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;

pub fn verify(
    bytes: &[u8],
    store: &TrustStore,
    expected: &PilotAcceptanceSubject,
    now: u64,
) -> Result<VerifiedPilotAcceptance> {
    let Authenticated {
        payload,
        signer_key_id,
        public_key_digest,
    } = attestation::authenticate(bytes, store, PAYLOAD_TYPE, MAX_ENVELOPE_BYTES, |key| {
        key.id == expected.reviewer
            && key.checks.iter().any(|scope| scope == CHECK_SCOPE)
            && key.allow_repository_checks
    })?;
    let record: PilotAcceptanceRecord =
        serde_json::from_slice(&payload).context("Invalid signed pilot acceptance record")?;
    if record.schema_version != 1
        || record.subject != *expected
        || record.subject.repository != store.repository
    {
        bail!("Pilot acceptance does not match the repository or evaluated evidence subject");
    }
    if record.record_id.trim().is_empty()
        || record.record_id.len() > 256
        || record.reason.trim().is_empty()
        || record.reason.len() > 16_384
    {
        bail!("Pilot acceptance ID and reason must be nonempty and bounded");
    }
    if record.reviewer.kind != ActorKind::Human
        || record.reviewer.id != expected.reviewer
        || signer_key_id != record.reviewer.id
        || record.reviewer.id == expected.owner
    {
        bail!("Pilot acceptance requires the configured independent human reviewer key");
    }
    if record.issued_at < expected.observation_end {
        bail!("Pilot acceptance cannot be issued before the observation window ends");
    }
    acceptance_decision(expected, record.decision)?;
    attestation::validity(
        store,
        &record.record_id,
        record.issued_at,
        record.expires_at,
        now,
    )?;
    Ok(VerifiedPilotAcceptance {
        record,
        signer_key_id,
        public_key_digest,
    })
}

#[cfg(test)]
#[path = "pilot_acceptance_tests.rs"]
mod tests;
