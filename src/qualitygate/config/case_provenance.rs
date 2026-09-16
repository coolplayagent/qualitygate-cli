//! Validate retained source lineage within the existing immutable evidence archive.
use super::{policy_acceptance::ValidationSuite, policy_candidates, policy_store::Store};
use crate::domain::{
    case_provenance::Classification,
    evolution::{EvidenceRecord, PolicyRevision},
    policy_evaluation::CaseKind,
};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_record(store: &Store, record: &EvidenceRecord) -> Result<()> {
    let Some(case) = &record.case else {
        return Ok(());
    };
    let mut pending = case.derived_from.clone();
    let mut seen = BTreeSet::new();
    let mut bytes = 0;
    while let Some(reference) = pending.pop() {
        if !seen.insert(reference.clone()) {
            continue;
        }
        if seen.len() > 128 {
            bail!("Case lineage exceeds 128 retained records");
        }
        let parent: EvidenceRecord = store.record(&reference, "evidence")?;
        bytes += serde_json::to_vec(&parent)?.len();
        bytes += store
            .index
            .object_inventory
            .get(&parent.source_digest)
            .copied()
            .unwrap_or(super::policy_store::MAX_OBJECT_BYTES as u64) as usize;
        if bytes > 8 * 1024 * 1024 {
            bail!("Case lineage exceeds 8 MiB");
        }
        let parent = policy_candidates::evidence(store, &reference)?;
        let Some(history) = &parent.case else {
            bail!("Case lineage must retain provenance, including generated/tuning use");
        };
        if case.classification == Classification::Independent && history.contaminated() {
            bail!("Independent case descends from generated or tuning evidence");
        }
        pending.extend(history.derived_from.clone());
    }
    if case.classification == Classification::Independent {
        // Include previous declarations even if a later claim omits its ancestry.
        if store.index.evidence.len() > 4096 {
            bail!("Case history exceeds 4096 records");
        }
        let mut bytes = 0;
        for reference in &store.index.evidence {
            let previous: EvidenceRecord = store.record(reference, "evidence")?;
            previous.validate().map_err(anyhow::Error::msg)?;
            bytes += serde_json::to_vec(&previous)?.len();
            if bytes > 16 * 1024 * 1024 {
                bail!("Case history exceeds 16 MiB");
            }
            if let Some(prior) = &previous.case
                && (prior.case_id == case.case_id || previous.source_digest == record.source_digest)
                && prior.contaminated()
            {
                bail!("Previously generated or tuned case cannot be relabeled independent");
            }
        }
    }
    Ok(())
}

pub fn validate_suite(
    store: &Store,
    suite: &ValidationSuite,
    revision: &PolicyRevision,
) -> Result<()> {
    if suite.schema_version != 2 {
        return Ok(());
    }
    let mut sources = BTreeSet::new();
    let mut cases = BTreeSet::new();
    let mut authors = BTreeSet::from([revision.created_by.id.as_str()]);
    for patch in &revision.patch {
        if let Some(actor) = patch["actor"]["id"].as_str() {
            authors.insert(actor);
        }
    }
    let mut motivating_sources = BTreeSet::new();
    for reference in &revision.evidence_refs {
        motivating_sources.insert(policy_candidates::evidence(store, reference)?.source_digest);
    }
    let mut inventory = BTreeMap::new();
    for input in &suite.cases {
        let reference = input
            .evidence_ref
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Suite v2 needs case evidence"))?;
        let evidence = policy_candidates::evidence(store, reference)?;
        validate_record(store, &evidence)?;
        let case = evidence
            .case
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Suite v2 needs case provenance"))?;
        if !sources.insert(evidence.source_digest.clone()) || !cases.insert(case.case_id.clone()) {
            bail!("Independent suite cases must not reuse the same source or case identity");
        }
        if input.kind != CaseKind::Replay
            && (case.classification != Classification::Independent
                || case
                    .reviewer
                    .as_ref()
                    .is_none_or(|reviewer| authors.contains(reviewer.id.as_str())))
        {
            bail!(
                "Held-out/anchor cases require independent provenance and a reviewer outside candidate authors"
            );
        }
        inventory.insert(reference, evidence);
    }
    // All ancestry is retained; independent cases cannot also be a tuning input in this suite.
    for evidence in inventory.values() {
        if let Some(case) = &evidence.case
            && case.classification == Classification::Independent
            && motivating_sources.contains(&evidence.source_digest)
        {
            bail!("Motivating/tuning evidence cannot double as an independent validation case");
        }
    }
    Ok(())
}
