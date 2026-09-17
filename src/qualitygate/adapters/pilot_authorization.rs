//! Authenticate a bounded Ed25519 DSSE pilot-start authorization.

use super::attestation::{self, Authenticated};
use crate::{
    config::attestation::TrustStore,
    domain::{
        evolution::ActorKind,
        pilot::{PlanAuthorizationRecord, PlanAuthorizationSubject, VerifiedPlanAuthorization},
    },
};
use anyhow::{Context, Result, bail};

pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.pilot-plan-authorization.v1+json";
pub const CHECK_SCOPE: &str = "pilot-plan-authorization";
pub const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;

pub fn verify(
    bytes: &[u8],
    store: &TrustStore,
    expected: &PlanAuthorizationSubject,
    now: u64,
) -> Result<VerifiedPlanAuthorization> {
    let Authenticated {
        payload,
        signer_key_id,
        public_key_digest,
    } = attestation::authenticate(bytes, store, PAYLOAD_TYPE, MAX_ENVELOPE_BYTES, |key| {
        key.id == expected.owner
            && key.checks.iter().any(|scope| scope == CHECK_SCOPE)
            && key.allow_repository_checks
    })?;
    let record: PlanAuthorizationRecord = serde_json::from_slice(&payload)
        .context("Invalid signed pilot plan authorization record")?;
    if record.schema_version != 1
        || record.subject != *expected
        || record.subject.repository != store.repository
    {
        bail!("Pilot authorization does not match the repository or sealed plan subject");
    }
    if record.record_id.trim().is_empty()
        || record.record_id.len() > 256
        || record.reason.trim().is_empty()
        || record.reason.len() > 16_384
    {
        bail!("Pilot authorization ID and reason must be nonempty and bounded");
    }
    if record.authorizer.kind != ActorKind::Human
        || record.authorizer.id != expected.owner
        || signer_key_id != record.authorizer.id
    {
        bail!("Pilot authorization requires the configured human owner and matching signing key");
    }
    if record.issued_at < expected.sealed_at
        || record.issued_at > expected.start_at
        || record.expires_at < expected.end_at
    {
        bail!(
            "Pilot authorization must be issued after sealing, before start, and cover the observation window"
        );
    }
    attestation::validity(
        store,
        &record.record_id,
        record.issued_at,
        record.expires_at,
        now,
    )?;
    Ok(VerifiedPlanAuthorization {
        record,
        signer_key_id,
        public_key_digest,
    })
}

#[cfg(test)]
#[path = "pilot_authorization_tests.rs"]
mod tests;
