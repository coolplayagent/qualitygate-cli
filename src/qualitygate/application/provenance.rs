//! Bind authenticated run evidence to a planned rule before evaluating its scope.

use super::{
    evidence,
    external::{self, Inputs},
};
use crate::{
    adapters::provenance::{self, ProvenanceFacts},
    config::{RuleSetting, catalog::Entry},
    domain::*,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub(super) struct Prepared {
    pub facts: ProvenanceFacts,
    pub artifacts: Vec<Artifact>,
    pub metadata: serde_json::Value,
}

fn needed(setting: &RuleSetting, entry: &Entry) -> bool {
    setting.provenance.is_some()
        || setting
            .parameters
            .get("provenance_scope")
            .and_then(serde_json::Value::as_str)
            == Some("ai_only")
        || entry.custom.as_ref().is_some_and(|rule| {
            rule.applies_to.provenance_scope.as_deref() == Some("ai_only")
                || rule
                    .requires_capabilities
                    .iter()
                    .any(|capability| capability == "external_provenance")
        })
}

pub(super) async fn prepare(
    id: &str,
    setting: &RuleSetting,
    entry: &Entry,
    snapshot: &Arc<Snapshot>,
    policy: &PolicyEvidence,
    inputs: Option<&Arc<Inputs>>,
    directory: &Path,
) -> std::result::Result<Option<Prepared>, Box<CheckResult>> {
    if !needed(setting, entry) {
        return Ok(None);
    }
    let mut result = CheckResult::pending(id, setting.required, setting.severity);
    let expected = provenance::subject(
        snapshot,
        policy,
        inputs.map_or("", |inputs| &inputs.store.repository),
        id,
    );
    result.metadata.insert(
        "external_provenance".into(),
        serde_json::json!({"expected_subject":expected,"verified":false}),
    );
    let verified = async {
        let inputs = inputs.context("Agent provenance requires caller-supplied --trust-store and --evidence-dir")?;
        let spec = setting.provenance.as_ref().context("This rule requires provenance.evidence_file in its policy setting")?;
        let bytes = external::record(inputs, &spec.evidence_file)?.to_vec();
        result.execution.artifacts.push(evidence::persist(directory,id,"provenance-envelope.json",&bytes).await?);
        result.execution.artifacts.push(evidence::persist(directory,id,"provenance-trust.json",&inputs.store_bytes).await?);
        let (owned_inputs, snapshot, now) = (inputs.clone(), snapshot.clone(), external::now()?);
        let facts = tokio::task::spawn_blocking(move || provenance::verify(&bytes, &owned_inputs.store, &expected, &snapshot, now)).await??;
        result.metadata.insert("external_provenance".into(), serde_json::json!({"expected_subject":facts.evidence()["subject"],
            "verified":true,"method":"ed25519_dsse_replay","evidence":facts.evidence(),"trust_source":inputs.store_path,
            "trust_digest":snapshot::digest(&inputs.store_bytes)}));
        Ok::<_, anyhow::Error>(facts)
    }.await;
    match verified {
        Ok(facts) => Ok(Some(Prepared {
            facts,
            artifacts: result.execution.artifacts,
            metadata: result
                .metadata
                .remove("external_provenance")
                .expect("provenance metadata"),
        })),
        Err(error) => {
            result.block(
                ExecutionStatus::Blocked,
                format!("Agent provenance could not be verified: {error:#}"),
            );
            Err(Box::new(result))
        }
    }
}

pub(super) fn revalidate(
    inputs: &Inputs,
    proofs: &BTreeMap<String, ProvenanceFacts>,
    results: &mut [CheckResult],
) -> Result<()> {
    let now = external::now()?;
    for result in results
        .iter_mut()
        .filter(|result| result.execution.status == ExecutionStatus::Completed)
    {
        if let Some(proof) = proofs.get(&result.id) {
            let validation = proof.revalidate_time(&inputs.store, now);
            result
                .metadata
                .get_mut("external_provenance")
                .expect("verified provenance")["valid_at_completion"] =
                serde_json::json!(validation.is_ok());
            if let Err(error) = validation {
                result.block(
                    ExecutionStatus::Blocked,
                    format!("Agent provenance became invalid before completion: {error:#}"),
                );
            }
        }
    }
    Ok(())
}
