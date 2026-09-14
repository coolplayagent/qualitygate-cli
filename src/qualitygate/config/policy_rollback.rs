//! Immutable rollback transitions over retained, previously selected policy states.

use super::{
    policy_candidates,
    policy_store::{Store, digest, now},
};
use crate::domain::{
    evolution::{Actor, PolicyTransition, RevisionStatus},
    policy_rollback::{ApprovedRollback, RollbackSubject},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

pub fn subject(
    store: &Store,
    repository: String,
    target: String,
    actor: Actor,
    trust_digest: String,
    evaluator_digest: String,
) -> Result<RollbackSubject> {
    actor.validate().map_err(anyhow::Error::msg)?;
    policy_candidates::load_version(store, &target)?;
    let from_policy = store
        .index
        .active_policy
        .clone()
        .context("No active policy to roll back")?;
    if from_policy == target {
        bail!("Rollback target is already active");
    }
    let history_cursor = store
        .index
        .last_event
        .clone()
        .context("No published policy history")?;
    let mut cursor = Some(history_cursor.clone());
    let mut target_history_ref = None;
    while let Some(reference) = cursor {
        let event: PolicyTransition = store.record(&reference, "transition")?;
        if ["policy_promoted", "policy_rolled_back"].contains(&event.action.as_str())
            && (event.from_policy.as_ref() == Some(&target)
                || event.to_policy.as_ref() == Some(&target))
        {
            target_history_ref = Some(reference);
            break;
        }
        cursor = event.previous;
    }
    Ok(RollbackSubject { repository, from_policy,
        from_authorization: store.index.active_approval.clone().context("Active policy has no authorization")?,
        from_since: store.index.active_since, to_policy: target, history_cursor,
        target_history_ref: target_history_ref.context("Rollback requires a previously selected policy; a draft archive is not a rollback target")?,
        trust_digest, evaluator_digest, proposed_by: actor })
}

pub fn supersede(store: &mut Store, status: RevisionStatus) -> Result<()> {
    if let Some(id) = store.index.active_candidate.clone() {
        let (reference, mut revision) = policy_candidates::candidate(store, &id)?;
        revision.previous_revision = Some(reference);
        revision.status = status;
        let reference = store.put_record("candidate", &revision)?;
        store.index.candidates.insert(id, reference);
    }
    Ok(())
}

pub(crate) fn publish(root: &Path, approved: &ApprovedRollback, envelope: &[u8]) -> Result<Value> {
    Store::transaction(root, |store| {
        let request = &approved.approval.subject;
        let expected = subject(
            store,
            request.repository.clone(),
            request.to_policy.clone(),
            request.proposed_by.clone(),
            request.trust_digest.clone(),
            request.evaluator_digest.clone(),
        )?;
        if expected != *request || digest(envelope) != approved.envelope_ref {
            bail!("Rollback state changed after authorization");
        }
        supersede(store, RevisionStatus::RolledBack)?;
        store.put_blob(envelope)?;
        let reference = store.put_record("policy_rollback", approved)?;
        store.index.active_policy = Some(request.to_policy.clone());
        store.index.active_approval = Some(reference.clone());
        store.index.active_authorization_kind = Some("policy_rollback".into());
        store.index.active_candidate = None;
        store.index.active_since = now()?;
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "policy_rolled_back".into(),
            subject: reference.clone(),
            actor: approved.approval.approver.clone(),
            reason: approved.approval.reason.clone(),
            timestamp: store.index.active_since,
            from_policy: Some(request.from_policy.clone()),
            to_policy: Some(request.to_policy.clone()),
        })?;
        Ok(
            json!({"schema_version":1,"active_policy":request.to_policy,"rollback_ref":reference,"approval":approved.approval}),
        )
    })
}
