//! A rollback has its own DSSE type; promotion signatures cannot authorize it.

use crate::{
    config::policy_acceptance::EvolutionTrust,
    domain::{
        ManualDecision,
        evolution::{ActorKind, validate_text},
        policy_rollback::{RollbackApproval, RollbackSubject},
    },
};
use anyhow::{Result, bail};
pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.policy-rollback.v1+json";

pub struct Verified {
    pub approval: RollbackApproval,
    pub signer_key_id: String,
    pub public_key_digest: String,
}

pub fn read_signed(bytes: &[u8], trust: &EvolutionTrust) -> Result<RollbackApproval> {
    let authenticated = super::attestation::authenticate(
        bytes,
        &super::policy_approval::keys(trust),
        PAYLOAD_TYPE,
        1024 * 1024,
        |_| true,
    )?;
    Ok(serde_json::from_slice(&authenticated.payload)?)
}

pub fn verify(
    bytes: &[u8],
    trust: &EvolutionTrust,
    expected: &RollbackSubject,
    now: u64,
) -> Result<Verified> {
    crate::config::policy_acceptance::validate_trust(trust)?;
    if trust.repository != expected.repository
        || !trust.evaluators.contains(&expected.evaluator_digest)
        || trust
            .revoked_approvals
            .contains(&crate::snapshot::digest(bytes))
    {
        bail!("Rollback authorization is revoked or outside the trusted repository/evaluator");
    }
    let authenticated = super::attestation::authenticate(
        bytes,
        &super::policy_approval::keys(trust),
        PAYLOAD_TYPE,
        1024 * 1024,
        |_| true,
    )?;
    let approval: RollbackApproval = serde_json::from_slice(&authenticated.payload)?;
    let key = trust
        .approval_keys
        .iter()
        .find(|key| key.id == authenticated.signer_key_id)
        .expect("authenticated key");
    validate_text(&approval.reason, 4096).map_err(anyhow::Error::msg)?;
    if approval.schema_version != 1
        || approval.subject != *expected
        || approval.approver != key.actor
        || key.actor.kind != ActorKind::Human
        || key.actor.id == expected.proposed_by.id
        || approval.decision != ManualDecision::Approved
    {
        bail!("Rollback requires independent signed approval for the exact transition");
    }
    if approval.issued_at == 0
        || approval.issued_at > now
        || approval.expires_at < now
        || approval.expires_at <= approval.issued_at
        || now - approval.issued_at > trust.max_age_seconds
        || approval.expires_at - approval.issued_at > trust.max_age_seconds
    {
        bail!("Rollback approval is expired, future-dated or exceeds the trusted maximum age");
    }
    Ok(Verified {
        approval,
        signer_key_id: authenticated.signer_key_id,
        public_key_digest: authenticated.public_key_digest,
    })
}
