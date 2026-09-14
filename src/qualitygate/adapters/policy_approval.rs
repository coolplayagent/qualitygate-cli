//! Ed25519 DSSE policy authorizations use a distinct payload type and human principals.

use crate::{
    config::{
        attestation::{TrustStore, TrustedKey},
        policy_acceptance::EvolutionTrust,
    },
    domain::{
        evolution::{ActorKind, validate_text},
        policy_approval::{PolicyApproval, PolicyApprovalSubject},
    },
};
use anyhow::{Result, bail};

pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.policy-approval.v1+json";

pub(super) fn keys(trust: &EvolutionTrust) -> TrustStore {
    TrustStore {
        schema_version: 1,
        repository: trust.repository.clone(),
        keys: trust
            .approval_keys
            .iter()
            .map(|key| TrustedKey {
                id: key.id.clone(),
                public_key: key.public_key.clone(),
                checks: vec!["policy-promotion".into()],
                provenance_rules: Vec::new(),
                tasks: Vec::new(),
                allow_repository_checks: true,
            })
            .collect(),
        revoked_records: Vec::new(),
        max_age_seconds: trust.max_age_seconds,
    }
}

pub fn validate_keys(trust: &EvolutionTrust) -> Result<()> {
    super::attestation::validate_keys(&keys(trust))
}

pub struct Verified {
    pub approval: PolicyApproval,
    pub signer_key_id: String,
    pub public_key_digest: String,
}

pub fn verify(
    bytes: &[u8],
    trust: &EvolutionTrust,
    subject: &PolicyApprovalSubject,
    now: u64,
) -> Result<Verified> {
    if trust
        .revoked_approvals
        .contains(&crate::snapshot::digest(bytes))
    {
        bail!("Policy approval has been revoked");
    }
    let authenticated =
        super::attestation::authenticate(bytes, &keys(trust), PAYLOAD_TYPE, 1024 * 1024, |_| true)?;
    let approval: PolicyApproval = serde_json::from_slice(&authenticated.payload)?;
    let key = trust
        .approval_keys
        .iter()
        .find(|key| key.id == authenticated.signer_key_id)
        .expect("authenticated configured key");
    validate_text(&approval.reason, 4096).map_err(anyhow::Error::msg)?;
    if approval.schema_version != 1
        || approval.subject != *subject
        || approval.approver != key.actor
    {
        bail!(
            "Policy approval does not bind the exact validated candidate and authorized identity"
        );
    }
    if key.actor.kind != ActorKind::Human || key.actor.id == subject.generation_actor.id {
        bail!(
            "Candidate generation and trusted approval identities must be independent; self-approval is forbidden"
        );
    }
    if approval.issued_at == 0
        || approval.issued_at > now
        || approval.expires_at < now
        || approval.expires_at <= approval.issued_at
        || now - approval.issued_at > trust.max_age_seconds
        || approval.expires_at - approval.issued_at > trust.max_age_seconds
    {
        bail!("Policy approval is expired, future-dated or exceeds the trusted maximum age");
    }
    Ok(Verified {
        approval,
        signer_key_id: authenticated.signer_key_id,
        public_key_digest: authenticated.public_key_digest,
    })
}
