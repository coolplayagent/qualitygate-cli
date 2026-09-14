//! Bounded longitudinal reads grouped only by matched inputs, budgets and evaluator epochs.

use super::{
    policy_candidates,
    policy_store::{Store, digest},
};
use crate::domain::{
    evolution::PolicyRevision, policy_effectiveness::measure, policy_evaluation::EvaluationRun,
};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn report(root: &Path, policy: Option<&str>, offset: usize, limit: usize) -> Result<Value> {
    if !(1..=256).contains(&limit) {
        bail!("Effectiveness page size must be 1..256");
    }
    let store = Store::open(root)?;
    if let Some(reference) = policy {
        policy_candidates::load_version(&store, reference)?;
    }
    let mut groups = BTreeMap::<String, Value>::new();
    let selected = store
        .index
        .evaluations
        .iter()
        .rev()
        .skip(offset)
        .take(limit);
    let mut scanned = 0;
    for reference in selected {
        scanned += 1;
        let run: EvaluationRun = store.record(reference, "evaluation")?;
        if policy
            .is_some_and(|policy| policy != run.candidate_policy && policy != run.baseline_policy)
        {
            continue;
        }
        let inputs = json!({"baseline_policy":run.baseline_policy,"suite_digest":run.suite_digest,
            "evaluator_epoch":run.evaluator_epoch,"evaluator_digest":run.evaluator_digest,
            "environment_digest":run.environment_digest,"budget":run.budget,"jobs":run.jobs});
        let key = digest(&serde_json::to_vec(&inputs)?);
        let group = groups
            .entry(key.clone())
            .or_insert_with(|| json!({"matched_group":key,"inputs":inputs,"observations":[]}));
        group["observations"]
            .as_array_mut()
            .expect("observations array")
            .push(serde_json::to_value(measure(reference.clone(), &run))?);
    }
    Ok(
        json!({"schema_version":1,"active_policy":store.index.active_policy,"groups":groups.into_values().collect::<Vec<_>>(),
        "offset":offset,"scanned":scanned,"next_offset":(offset.saturating_add(scanned) < store.index.evaluations.len()).then_some(offset.saturating_add(scanned)),
        "verification":{"conclusion":"Paired observations are comparable only within each matched group",
        "verified_shapes":["Frozen suite, baseline, evaluator epoch, environment and identical resource budgets"],
        "known_limits":["Pagination covers only the reported evaluations; groups are not whole-archive aggregates", "Runtime includes evaluation preparation and checks; it is not a universal speedup guarantee", "Findings are not labeled false positives without independent evidence"],
        "unverified_assumptions":["Subsequent agent use, downstream benefit, context tokens and review effort are unknown"]}}),
    )
}

pub fn rule_history(root: &Path, rule: &str, offset: usize, limit: usize) -> Result<Value> {
    if !(1..=256).contains(&limit) {
        bail!("Rule history page size must be 1..256");
    }
    let store = Store::open(root)?;
    let mut evidence = Vec::new();
    for reference in store.index.evidence.iter().rev().skip(offset).take(limit) {
        let record = policy_candidates::evidence(&store, reference)?;
        if record.rule_ids.iter().any(|id| id == rule) {
            evidence.push(
                json!({"reference":reference,"record":record,"trust":"unverified_source_evidence"}),
            );
        }
    }
    let mut candidates = Vec::new();
    for (id, reference) in store.index.candidates.iter().skip(offset).take(limit) {
        let revision: PolicyRevision = store.record(reference, "candidate")?;
        let (_, config, _) = policy_candidates::load_version(&store, &revision.policy_digest)?;
        if config.rules.contains_key(rule) {
            candidates.push(json!({"id":id,"revision_ref":reference,"revision":revision,"lifecycle":config.rule_lifecycle.get(rule)}));
        }
    }
    let has_more =
        offset.saturating_add(limit) < store.index.evidence.len().max(store.index.candidates.len());
    Ok(
        json!({"schema_version":1,"rule_id":rule,"evidence":evidence,"candidates":candidates,
        "active_policy":store.index.active_policy,"next_offset":has_more.then_some(offset.saturating_add(limit)),
        "known_limits":["Evidence and candidate inventories are paged independently at the same offset", "Candidate lifecycle state is explicit; archived proposals are not approval"]}),
    )
}

pub fn record(root: &Path, reference: &str, kind: &str) -> Result<Value> {
    let store = Store::open(root)?;
    if kind == "blob" {
        use base64::Engine;
        let bytes = store.blob(reference)?;
        return Ok(
            json!({"schema_version":1,"reference":reference,"bytes":bytes.len(),"encoding":"base64",
            "data":base64::engine::general_purpose::STANDARD.encode(&bytes)}),
        );
    }
    if ![
        "check_report",
        "validation_attempt",
        "policy_approval",
        "candidate",
    ]
    .contains(&kind)
    {
        bail!("Unsupported experience record kind");
    }
    let value: Value = store.record(reference, kind)?;
    Ok(json!({"schema_version":1,"reference":reference,"kind":kind,"record":value}))
}
