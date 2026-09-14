//! Persist attempts before execution and publish results only for the frozen revision.

use super::{
    policy_candidates,
    policy_store::{Store, now},
};
use crate::domain::{
    evolution::{Actor, PolicyRevision, PolicyTransition, RevisionStatus},
    policy_evaluation::{Conclusion, EvaluationRun},
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub candidate_id: String,
    pub input_revision: String,
    pub candidate_policy: String,
    pub baseline_policy: String,
    pub suite_digest: String,
    pub trust_digest: String,
    pub evaluator_digest: String,
    pub actor: Actor,
    pub started_at: u64,
}

pub(super) fn binding(attempt: &Attempt, run: &EvaluationRun) -> Result<()> {
    if attempt.candidate_id != run.candidate_id
        || attempt.input_revision != run.candidate_revision
        || attempt.candidate_policy != run.candidate_policy
        || attempt.baseline_policy != run.baseline_policy
        || attempt.suite_digest != run.suite_digest
        || attempt.trust_digest != run.trust_digest
        || attempt.evaluator_digest != run.evaluator_digest
        || attempt.actor != run.evaluation_actor
        || attempt.started_at != run.started_at
        || run.ended_at < run.started_at
    {
        bail!("Evaluation differs from its immutable validation attempt");
    }
    Ok(())
}

pub fn begin(
    root: &Path,
    attempt: &Attempt,
    suite_bytes: &[u8],
    trust_bytes: &[u8],
) -> Result<String> {
    attempt.actor.validate().map_err(anyhow::Error::msg)?;
    Store::transaction(root, |store| {
        if store.put_blob(suite_bytes)? != attempt.suite_digest
            || store.put_blob(trust_bytes)? != attempt.trust_digest
        {
            bail!("Attempt acceptance input digests differ from frozen bytes");
        }
        let (current, mut candidate) = policy_candidates::candidate(store, &attempt.candidate_id)?;
        if current != attempt.input_revision
            || !candidate.editable()
            || candidate.policy_digest != attempt.candidate_policy
            || candidate.parent_policy_digest != attempt.baseline_policy
        {
            bail!("Candidate changed before validation could freeze its revision");
        }
        let reference = store.put_record("validation_attempt", attempt)?;
        candidate.previous_revision = Some(current);
        candidate.status = RevisionStatus::Validating;
        candidate.evaluation_ref = Some(reference.clone());
        candidate.approval_ref = None;
        let revision = store.put_record("candidate", &candidate)?;
        store
            .index
            .candidates
            .insert(attempt.candidate_id.clone(), revision.clone());
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "validation_started".into(),
            subject: reference,
            actor: attempt.actor.clone(),
            reason: candidate.reason,
            timestamp: now()?,
            from_policy: store.index.active_policy.clone(),
            to_policy: store.index.active_policy.clone(),
        })?;
        Ok(revision)
    })
}

pub fn finish(root: &Path, frozen_revision: &str, evaluation: &mut EvaluationRun) -> Result<Value> {
    Store::transaction(root, |store| {
        let (current, mut candidate) =
            policy_candidates::candidate(store, &evaluation.candidate_id)?;
        if current != frozen_revision || candidate.status != RevisionStatus::Validating {
            bail!("Frozen candidate changed or validation was cancelled");
        }
        let attempt: Attempt = store.record(
            candidate
                .evaluation_ref
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Frozen candidate has no attempt"))?,
            "validation_attempt",
        )?;
        binding(&attempt, evaluation)?;
        if candidate.policy_digest != evaluation.candidate_policy
            || candidate.parent_policy_digest != evaluation.baseline_policy
        {
            bail!("Evaluation does not bind the frozen policy pair");
        }
        policy_candidates::load_version(store, &candidate.policy_digest)?;
        policy_candidates::load_version(store, &candidate.parent_policy_digest)?;
        for reference in &candidate.evidence_refs {
            policy_candidates::evidence(store, reference)?;
        }
        let reference = store.put_record("evaluation", evaluation)?;
        store.index.evaluations.push(reference.clone());
        candidate.previous_revision = Some(current);
        candidate.status = if evaluation.conclusion == Conclusion::Block {
            RevisionStatus::Rejected
        } else {
            RevisionStatus::Candidate
        };
        candidate.evaluation_ref = Some(reference.clone());
        candidate.approval_ref = None;
        let revision = store.put_record("candidate", &candidate)?;
        store
            .index
            .candidates
            .insert(evaluation.candidate_id.clone(), revision.clone());
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "validation_completed".into(),
            subject: reference.clone(),
            actor: evaluation.evaluation_actor.clone(),
            reason: format!(
                "{:?}: {}",
                evaluation.conclusion,
                evaluation.reasons.join("; ")
            ),
            timestamp: now()?,
            from_policy: store.index.active_policy.clone(),
            to_policy: store.index.active_policy.clone(),
        })?;
        Ok(
            json!({"schema_version":1,"id":evaluation.candidate_id,"revision_ref":revision,"evaluation_ref":reference,"evaluation":evaluation}),
        )
    })
}

pub fn retain_reports(
    root: &Path,
    case: &mut crate::domain::policy_evaluation::EvaluatedCase,
    baseline: &crate::domain::Report,
    candidate: &crate::domain::Report,
) -> Result<()> {
    Store::transaction(root, |store| {
        case.baseline.report_ref = Some(store.put_record("check_report", baseline)?);
        case.candidate.report_ref = Some(store.put_record("check_report", candidate)?);
        super::policy_artifacts::retain(root, store, baseline)?;
        super::policy_artifacts::retain(root, store, candidate)?;
        Ok(())
    })
}

pub fn evaluation(root: &Path, reference: &str) -> Result<Value> {
    let store = Store::open(root)?;
    if !store.index.evaluations.iter().any(|id| id == reference) {
        bail!("Evaluation is not published in this archive");
    }
    let run: EvaluationRun = store.record(reference, "evaluation")?;
    Ok(json!({"schema_version":1,"evaluation_ref":reference,"evaluation":run}))
}

/// Explicit cancellation leaves the attempt and any externally written logs queryable.
pub fn abandon(root: &Path, id: &str, actor: Actor, reason: &str) -> Result<Value> {
    actor.validate().map_err(anyhow::Error::msg)?;
    crate::domain::evolution::validate_text(reason, 4096).map_err(anyhow::Error::msg)?;
    Store::transaction(root, |store| {
        let (current, mut candidate): (_, PolicyRevision) =
            policy_candidates::candidate(store, id)?;
        if !candidate.editable() && candidate.status != RevisionStatus::Validating {
            bail!("Only an editable candidate or running validation can be abandoned");
        }
        candidate.previous_revision = Some(current);
        candidate.status = RevisionStatus::Abandoned;
        let revision = store.put_record("candidate", &candidate)?;
        store.index.candidates.insert(id.into(), revision.clone());
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "candidate_abandoned".into(),
            subject: revision.clone(),
            actor,
            reason: reason.into(),
            timestamp: now()?,
            from_policy: store.index.active_policy.clone(),
            to_policy: store.index.active_policy.clone(),
        })?;
        Ok(json!({"schema_version":1,"id":id,"revision_ref":revision,"revision":candidate}))
    })
}
