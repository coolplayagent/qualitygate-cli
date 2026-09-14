//! External authorization is the final boundary for changing the active policy by rollback.
use crate::{
    adapters::{policy_approval, policy_rollback},
    config::{
        self,
        policy_acceptance::{EvolutionTrust, ProtectedFile},
        policy_store::{Store, digest, now},
    },
    domain::{evolution::Actor, policy_rollback::ApprovedRollback},
};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub async fn subject(
    root: PathBuf,
    target: String,
    trust_path: PathBuf,
    actor: Actor,
) -> Result<serde_json::Value> {
    let evaluator = super::evaluator_digest().await?;
    tokio::task::spawn_blocking(move || {
        let file = ProtectedFile::read(&root, &trust_path)?;
        let trust: EvolutionTrust = serde_norway::from_slice(&file.bytes)?;
        config::policy_acceptance::validate_trust(&trust)?;
        policy_approval::validate_keys(&trust)?;
        let repository = dunce::canonicalize(&root)?.to_string_lossy().to_string();
        if trust.repository != repository || !trust.evaluators.contains(&evaluator) {
            bail!("Rollback trust must authorize this repository and evaluator");
        }
        let subject = config::policy_rollback::subject(&Store::open(&root)?, repository, target, actor, digest(&file.bytes), evaluator)?;
        file.unchanged(&root)?;
        Ok(serde_json::json!({"schema_version":1,"payload_type":policy_rollback::PAYLOAD_TYPE,"subject":subject}))
    }).await?
}

pub async fn rollback(
    root: PathBuf,
    target: String,
    trust_path: PathBuf,
    approval_path: PathBuf,
) -> Result<serde_json::Value> {
    let evaluator = super::evaluator_digest().await?;
    tokio::task::spawn_blocking(move || {
        let file = ProtectedFile::read(&root, &trust_path)?;
        let envelope = ProtectedFile::read(&root, &approval_path)?;
        let trust: EvolutionTrust = serde_norway::from_slice(&file.bytes)?;
        // Parse only after DSSE authentication; bind the authenticated request to current state below.
        let approval = policy_rollback::read_signed(&envelope.bytes, &trust)?;
        let expected = config::policy_rollback::subject(
            &Store::open(&root)?,
            dunce::canonicalize(&root)?.to_string_lossy().to_string(),
            target,
            approval.subject.proposed_by,
            digest(&file.bytes),
            evaluator,
        )?;
        let verified = policy_rollback::verify(&envelope.bytes, &trust, &expected, now()?)?;
        let approved = ApprovedRollback {
            approval: verified.approval,
            signer_key_id: verified.signer_key_id,
            public_key_digest: verified.public_key_digest,
            envelope_ref: digest(&envelope.bytes),
            trust_source: file
                .path
                .to_str()
                .context("Trust path is not UTF-8")?
                .into(),
        };
        file.unchanged(&root)?;
        envelope.unchanged(&root)?;
        config::policy_rollback::publish(&root, &approved, &envelope.bytes)
    })
    .await?
}
