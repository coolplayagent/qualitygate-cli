//! Complete validation evidence and immutable transitions used by trusted promotion.

use super::{
    policy_acceptance::{EvolutionTrust, ValidationSuite},
    policy_candidates,
    policy_store::{Store, digest, now},
};
use crate::domain::{
    ManualDecision, Report,
    evolution::{PolicyRevision, PolicyTransition, RevisionStatus},
    policy_approval::{ApprovedPolicy, PolicyApprovalSubject},
    policy_evaluation::{Conclusion, EvaluationRun, Observation, decide},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

pub fn rules_digest(config: &super::Config, catalog: &super::catalog::Catalog) -> Result<String> {
    Ok(digest(&serde_json::to_vec(
        &json!({"settings":config.rules,"definitions":catalog,
        "source_reviews":config.source_reviews,"engine_version":env!("CARGO_PKG_VERSION")}),
    )?))
}

/// Recompute the oracle comparison from the retained full reports before accepting an approval.
pub fn complete_evaluation(
    store: &Store,
    id: &str,
    candidate: &PolicyRevision,
) -> Result<EvaluationRun> {
    let reference = candidate
        .evaluation_ref
        .as_deref()
        .context("Candidate has no validation record")?;
    if !store.index.evaluations.iter().any(|id| id == reference) {
        bail!("Candidate evaluation is not published");
    }
    let mut run: EvaluationRun = store.record(reference, "evaluation")?;
    if run.schema_version != 1
        || run.candidate_id != id
        || run.conclusion != Conclusion::Pass
        || run.candidate_policy != candidate.policy_digest
        || run.baseline_policy != candidate.parent_policy_digest
        || run.generation_actor != candidate.created_by
    {
        bail!(
            "Promotion requires a complete passing validation for the exact candidate and parent"
        );
    }
    let validated: PolicyRevision = if candidate.status == RevisionStatus::Approved {
        store.record(
            candidate
                .previous_revision
                .as_deref()
                .context("Approved candidate has no validated revision")?,
            "candidate",
        )?
    } else {
        candidate.clone()
    };
    let frozen: PolicyRevision = store.record(
        validated
            .previous_revision
            .as_deref()
            .context("Validation has no frozen revision")?,
        "candidate",
    )?;
    if frozen.status != RevisionStatus::Validating
        || frozen.policy_digest != candidate.policy_digest
        || frozen.evidence_refs != candidate.evidence_refs
        || frozen.patch != candidate.patch
    {
        bail!("Validation revision chain differs from the candidate");
    }
    let attempt: super::policy_validation::Attempt = store.record(
        frozen
            .evaluation_ref
            .as_deref()
            .context("Frozen revision has no validation attempt")?,
        "validation_attempt",
    )?;
    super::policy_validation::binding(&attempt, &run)?;
    let suite: ValidationSuite = super::parse_yaml(&store.blob(&run.suite_digest)?)?;
    super::policy_acceptance::validate_suite(&suite)?;
    if suite.baseline_policy != run.baseline_policy
        || suite.budget != run.budget
        || suite.evaluator_epoch != run.evaluator_epoch
        || run.jobs == 0
        || run.jobs > run.budget.max_parallel
    {
        bail!("Evaluation does not bind the protected acceptance epoch and resource budget");
    }
    let (baseline, baseline_config, baseline_catalog) =
        policy_candidates::load_version(store, &run.baseline_policy)?;
    let (proposed, candidate_config, candidate_catalog) =
        policy_candidates::load_version(store, &run.candidate_policy)?;
    if candidate_config.rules.len() > suite.max_rules
        || candidate_config
            .rules
            .len()
            .saturating_sub(baseline_config.rules.len())
            > suite.max_rule_growth
    {
        bail!("Candidate exceeds the protected library contribution policy");
    }
    let (baseline_config, baseline_catalog) = baseline_catalog.resolve(&baseline_config)?;
    let (candidate_config, candidate_catalog) = candidate_catalog.resolve(&candidate_config)?;
    let mut seen = BTreeSet::new();
    for case in &mut run.cases {
        if !seen.insert(case.id.clone()) {
            bail!("Duplicate evaluation case");
        }
        let expected = suite
            .cases
            .iter()
            .find(|expected| expected.id == case.id)
            .context("Evaluation case is not in the protected suite")?;
        let task_digest = digest(&serde_json::to_vec(&expected.task)?);
        if case.base != expected.base
            || case.head != expected.head
            || case.kind != expected.kind
            || case.task_digest != task_digest
        {
            bail!("Evaluation case changed its pinned snapshot or task");
        }
        let mut producers = Vec::new();
        for (observation, policy, reference, config, catalog) in [
            (
                &mut case.baseline,
                &baseline,
                &run.baseline_policy,
                &baseline_config,
                &baseline_catalog,
            ),
            (
                &mut case.candidate,
                &proposed,
                &run.candidate_policy,
                &candidate_config,
                &candidate_catalog,
            ),
        ] {
            let report_ref = observation
                .report_ref
                .clone()
                .context("Complete evaluation has no retained check report")?;
            let report: Report = store.record(&report_ref, "check_report")?;
            super::policy_artifacts::verify(store, &report)?;
            producers.push(crate::domain::policy_evaluation::producer_environment(
                &report,
            ));
            let plan = super::Plan::build(config, Some(&expected.task), "full")?;
            let gate = crate::domain::evaluate(&report.checks, &plan.required, &[]);
            if report.profile != "full"
                || report.policy.rules_digest != rules_digest(config, catalog)?
                || report.plan.execution_order != plan.order
                || report.plan.required_checks != plan.required
                || report.plan.task_id != plan.task_id
                || report
                    .checks
                    .iter()
                    .map(|check| &check.id)
                    .ne(plan.order.iter())
                || !gate.complete
                || gate.decision != report.gate.decision
                || report.snapshot.base != case.base
                || report.snapshot.head != case.head
                || Some(&report.snapshot.content_digest) != case.snapshot_digest.as_ref()
                || report.policy.source != *reference
                || report.policy.config_digest != policy.files[&policy.config_path].digest
                || report.policy.task_contract_digest.as_ref() != Some(&case.task_digest)
                || report.evaluator_digest != run.evaluator_digest
                || report.environment_digest != run.environment_digest
            {
                bail!(
                    "Retained report does not match the paired policy/snapshot/task/evaluator/environment binding"
                );
            }
            let mut recomputed =
                Observation::from_report(&report, &expected.expectations, observation.duration_ms);
            recomputed.report_ref = Some(report_ref);
            *observation = recomputed;
        }
        if !crate::domain::policy_evaluation::matched_producers(&producers[0], &producers[1]) {
            bail!("Paired producer identities or versions differ");
        }
    }
    let (conclusion, _) = decide(&run.cases, suite.cases.len(), suite.min_improvements, &[]);
    if conclusion != Conclusion::Pass {
        bail!("Retained reports do not establish complete passing protected acceptance");
    }
    Ok(run)
}

pub fn subject(
    store: &Store,
    id: &str,
    repository: &str,
    promoting: bool,
) -> Result<PolicyApprovalSubject> {
    let (reference, candidate) = policy_candidates::candidate(store, id)?;
    if promoting && candidate.status != RevisionStatus::Approved
        || !promoting && !candidate.editable()
    {
        bail!("Candidate is not in the required approval lifecycle state");
    }
    let run = complete_evaluation(store, id, &candidate)?;
    Ok(PolicyApprovalSubject {
        repository: repository.into(),
        candidate_id: id.into(),
        candidate_revision: if promoting {
            candidate
                .previous_revision
                .context("Approved candidate has no authorized prior revision")?
        } else {
            reference
        },
        candidate_policy: candidate.policy_digest,
        parent_policy: candidate.parent_policy_digest,
        evaluation_ref: candidate.evaluation_ref.expect("validated evaluation"),
        suite_digest: run.suite_digest,
        trust_digest: run.trust_digest,
        evaluator_digest: run.evaluator_digest,
        generation_actor: candidate.created_by,
    })
}

pub fn authorize_trust(
    trust: &EvolutionTrust,
    trust_bytes: &[u8],
    subject: &PolicyApprovalSubject,
    evaluator: &str,
) -> Result<()> {
    super::policy_acceptance::validate_trust(trust)?;
    if trust.repository != subject.repository
        || digest(trust_bytes) != subject.trust_digest
        || !trust.suites.contains(&subject.suite_digest)
        || !trust.baselines.contains(&subject.parent_policy)
        || !trust.evaluators.contains(&subject.evaluator_digest)
        || subject.evaluator_digest != evaluator
    {
        bail!("Approval trust or evaluator differs from the protected validation inputs");
    }
    Ok(())
}

pub(crate) fn approve(root: &Path, approved: &ApprovedPolicy, envelope: &[u8]) -> Result<Value> {
    Store::transaction(root, |store| {
        let expected = subject(
            store,
            &approved.approval.subject.candidate_id,
            &approved.approval.subject.repository,
            false,
        )?;
        if expected != approved.approval.subject || digest(envelope) != approved.envelope_ref {
            bail!("Candidate or approval bytes changed before publication");
        }
        let (previous, mut candidate) =
            policy_candidates::candidate(store, &expected.candidate_id)?;
        store.put_blob(envelope)?;
        let reference = store.put_record("policy_approval", approved)?;
        candidate.previous_revision = Some(previous);
        candidate.status = if approved.approval.decision == ManualDecision::Approved {
            RevisionStatus::Approved
        } else {
            RevisionStatus::Rejected
        };
        candidate.approval_ref = Some(reference.clone());
        let revision = store.put_record("candidate", &candidate)?;
        store
            .index
            .candidates
            .insert(expected.candidate_id.clone(), revision.clone());
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "trusted_policy_review".into(),
            subject: reference.clone(),
            actor: approved.approval.approver.clone(),
            reason: approved.approval.reason.clone(),
            timestamp: now()?,
            from_policy: store.index.active_policy.clone(),
            to_policy: store.index.active_policy.clone(),
        })?;
        Ok(
            json!({"schema_version":1,"id":expected.candidate_id,"revision_ref":revision,"approval_ref":reference,"revision":candidate}),
        )
    })
}

pub(crate) fn promote(root: &Path, approved: &ApprovedPolicy) -> Result<Value> {
    Store::transaction(root, |store| {
        let expected = subject(
            store,
            &approved.approval.subject.candidate_id,
            &approved.approval.subject.repository,
            true,
        )?;
        if expected != approved.approval.subject
            || approved.approval.decision != ManualDecision::Approved
        {
            bail!("Promotion does not have the required trusted approval");
        }
        if store
            .index
            .active_policy
            .as_ref()
            .is_some_and(|active| active != &expected.parent_policy)
        {
            bail!("Active policy changed; create and validate a candidate from the current parent");
        }
        let (previous, mut candidate) =
            policy_candidates::candidate(store, &expected.candidate_id)?;
        let from_policy = store
            .index
            .active_policy
            .clone()
            .or_else(|| Some(expected.parent_policy.clone()));
        super::policy_rollback::supersede(store, RevisionStatus::Superseded)?;
        candidate.previous_revision = Some(previous);
        candidate.status = RevisionStatus::Active;
        let revision = store.put_record("candidate", &candidate)?;
        store
            .index
            .candidates
            .insert(expected.candidate_id.clone(), revision.clone());
        store.index.active_policy = Some(expected.candidate_policy.clone());
        store.index.active_approval = candidate.approval_ref.clone();
        store.index.active_authorization_kind = Some("policy_approval".into());
        store.index.active_candidate = Some(expected.candidate_id.clone());
        store.index.active_since = now()?;
        store.event(PolicyTransition {
            sequence: 0,
            previous: None,
            action: "policy_promoted".into(),
            subject: revision.clone(),
            actor: approved.approval.approver.clone(),
            reason: approved.approval.reason.clone(),
            timestamp: store.index.active_since,
            from_policy,
            to_policy: store.index.active_policy.clone(),
        })?;
        Ok(
            json!({"schema_version":1,"id":expected.candidate_id,"revision_ref":revision,"active_policy":store.index.active_policy,"approval_ref":candidate.approval_ref}),
        )
    })
}
