//! Structural selection of immutable active packages. Application authenticates their signatures.

use super::{
    Config,
    policy_candidates::{self, FrozenCatalog},
    policy_store::Store,
};
use crate::domain::{
    evolution::{PolicyRevision, PolicyVersion},
    policy_approval::ApprovedPolicy,
    policy_evaluation::{Conclusion, EvaluationRun},
    policy_rollback::ApprovedRollback,
};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub struct Active {
    pub reference: String,
    pub approval_ref: String,
    pub since: u64,
    pub authority: Authorization,
    pub version: PolicyVersion,
    pub config: Config,
    pub frozen: FrozenCatalog,
}

pub enum Authorization {
    Promotion(ApprovedPolicy),
    Rollback(ApprovedRollback),
}

impl Active {
    pub fn evaluator_digest(&self) -> &str {
        match &self.authority {
            Authorization::Promotion(approved) => &approved.approval.subject.evaluator_digest,
            Authorization::Rollback(approved) => &approved.approval.subject.evaluator_digest,
        }
    }
}

pub fn load(root: &Path) -> Result<Option<Active>> {
    let Some(store) = Store::optional(root)? else {
        return Ok(None);
    };
    let Some(reference) = &store.index.active_policy else {
        return Ok(None);
    };
    let approval_ref = store
        .index
        .active_approval
        .as_ref()
        .context("Active policy has no authorization")?;
    if store.index.active_authorization_kind.as_deref() == Some("policy_rollback") {
        let approved: ApprovedRollback = store.record(approval_ref, "policy_rollback")?;
        if approved.approval.subject.to_policy != *reference || store.index.active_since == 0 {
            bail!("Active rollback does not bind the selected policy");
        }
        let (version, config, frozen) = policy_candidates::load_version(&store, reference)?;
        return Ok(Some(Active {
            reference: reference.clone(),
            approval_ref: approval_ref.clone(),
            since: store.index.active_since,
            authority: Authorization::Rollback(approved),
            version,
            config,
            frozen,
        }));
    }
    if store.index.active_authorization_kind.as_deref() != Some("policy_approval") {
        bail!("Unknown active policy authorization kind");
    }
    let approved: ApprovedPolicy = store.record(approval_ref, "policy_approval")?;
    let subject = &approved.approval.subject;
    let revision: PolicyRevision = store.record(&subject.candidate_revision, "candidate")?;
    let run: EvaluationRun = store.record(&subject.evaluation_ref, "evaluation")?;
    if subject.candidate_policy != *reference
        || revision.policy_digest != *reference
        || revision.parent_policy_digest != subject.parent_policy
        || revision.evaluation_ref.as_ref() != Some(&subject.evaluation_ref)
        || revision.created_by != subject.generation_actor
        || run.conclusion != Conclusion::Pass
        || run.candidate_id != subject.candidate_id
        || run.candidate_policy != *reference
        || run.baseline_policy != subject.parent_policy
        || run.suite_digest != subject.suite_digest
        || run.trust_digest != subject.trust_digest
        || run.evaluator_digest != subject.evaluator_digest
        || store.index.active_since == 0
    {
        bail!("Active policy does not bind its immutable validation and approval");
    }
    let (version, config, frozen) = policy_candidates::load_version(&store, reference)?;
    Ok(Some(Active {
        reference: reference.clone(),
        approval_ref: approval_ref.clone(),
        since: store.index.active_since,
        authority: Authorization::Promotion(approved),
        version,
        config,
        frozen,
    }))
}

/// Navigation overlays never supply enforcement settings or definitions.
pub fn navigation(root: &Path, path: &str, config: &mut Config) -> Result<()> {
    let draft_path = crate::paths::confined(root, path.as_ref())?;
    match std::fs::symlink_metadata(draft_path) {
        Ok(_) => {
            let bytes = super::rule_authoring::read_file(root, path)?;
            let draft: Config = super::parse_yaml(&bytes)?;
            super::categories::validate(&draft)?;
            config.categories = draft.categories;
            config.rule_categories = draft.rule_categories;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
