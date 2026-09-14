//! Verify external approval signatures before recording or activating an immutable policy.

use crate::{
    adapters::policy_approval,
    config::{
        self,
        policy_acceptance::{EvolutionTrust, ProtectedFile},
        policy_store::{Store, digest, now},
    },
    domain::{ManualDecision, policy_approval::ApprovedPolicy},
};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub async fn subject(root: PathBuf, id: String, trust_path: PathBuf) -> Result<serde_json::Value> {
    let evaluator = super::evaluator_digest().await?;
    tokio::task::spawn_blocking(move || {
        let trust_file = ProtectedFile::read(&root, &trust_path)?;
        let trust: EvolutionTrust = serde_norway::from_slice(&trust_file.bytes)?;
        let store = Store::open(&root)?;
        let subject = config::policy_promotion::subject(&store, &id, &dunce::canonicalize(&root)?.to_string_lossy(), false)?;
        config::policy_promotion::authorize_trust(&trust, &trust_file.bytes, &subject, &evaluator)?;
        policy_approval::validate_keys(&trust)?;
        Ok(serde_json::json!({"schema_version":1,"payload_type":policy_approval::PAYLOAD_TYPE,"subject":subject}))
    }).await?
}

pub async fn approve(
    root: PathBuf,
    id: String,
    approval_path: PathBuf,
    trust_path: PathBuf,
) -> Result<(serde_json::Value, u8)> {
    let evaluator = super::evaluator_digest().await?;
    tokio::task::spawn_blocking(move || {
        let trust_file = ProtectedFile::read(&root, &trust_path)?;
        let envelope = ProtectedFile::read(&root, &approval_path)?;
        let trust: EvolutionTrust = serde_norway::from_slice(&trust_file.bytes)?;
        let expected = config::policy_promotion::subject(
            &Store::open(&root)?,
            &id,
            &dunce::canonicalize(&root)?.to_string_lossy(),
            false,
        )?;
        config::policy_promotion::authorize_trust(
            &trust,
            &trust_file.bytes,
            &expected,
            &evaluator,
        )?;
        let verified = policy_approval::verify(&envelope.bytes, &trust, &expected, now()?)?;
        let code = if verified.approval.decision == ManualDecision::Approved {
            0
        } else {
            1
        };
        let approved = ApprovedPolicy {
            approval: verified.approval,
            signer_key_id: verified.signer_key_id,
            public_key_digest: verified.public_key_digest,
            envelope_ref: digest(&envelope.bytes),
            trust_source: trust_file
                .path
                .to_str()
                .context("Trust path is not UTF-8")?
                .into(),
        };
        trust_file.unchanged(&root)?;
        envelope.unchanged(&root)?;
        Ok((
            config::policy_promotion::approve(&root, &approved, &envelope.bytes)?,
            code,
        ))
    })
    .await?
}

pub async fn promote(root: PathBuf, id: String) -> Result<serde_json::Value> {
    let evaluator = super::evaluator_digest().await?;
    tokio::task::spawn_blocking(move || {
        let store = Store::open(&root)?;
        let (_, candidate) = config::policy_candidates::candidate(&store, &id)?;
        let reference = candidate
            .approval_ref
            .as_deref()
            .context("Candidate has no trusted approval")?;
        let approved: ApprovedPolicy = store.record(reference, "policy_approval")?;
        let trust_file = ProtectedFile::read(&root, approved.trust_source.as_ref())?;
        let trust: EvolutionTrust = serde_norway::from_slice(&trust_file.bytes)?;
        let expected = config::policy_promotion::subject(
            &store,
            &id,
            &dunce::canonicalize(&root)?.to_string_lossy(),
            true,
        )?;
        config::policy_promotion::authorize_trust(
            &trust,
            &trust_file.bytes,
            &expected,
            &evaluator,
        )?;
        let verified = policy_approval::verify(
            &store.blob(&approved.envelope_ref)?,
            &trust,
            &expected,
            now()?,
        )?;
        if verified.signer_key_id != approved.signer_key_id
            || verified.public_key_digest != approved.public_key_digest
        {
            bail!("Recorded approval signer differs from the authenticated identity");
        }
        trust_file.unchanged(&root)?;
        config::policy_promotion::promote(&root, &approved)
    })
    .await?
}
