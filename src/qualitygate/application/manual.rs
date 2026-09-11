//! External trust inputs are loaded before commands and revalidated before publication.

use super::external::{self, Inputs, now, unchanged};
use crate::{
    adapters::{attestation, rules::diagnostic},
    config::{CheckKind, CommandCheck, Plan},
    domain::*,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

fn subject(
    check: &CommandCheck,
    plan: &Plan,
    snapshot: &Snapshot,
    policy: &PolicyEvidence,
    repository: &str,
) -> ManualSubject {
    ManualSubject {
        repository: repository.to_owned(),
        snapshot: SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone()),
        policy: PolicyBinding::from(policy),
        check_id: check.id.clone(),
        task: plan.task_id.as_ref().and_then(|task_id| {
            plan.acceptance
                .iter()
                .find(|(_, id)| **id == check.id)
                .map(|(acceptance_id, _)| TaskBinding {
                    task_id: task_id.clone(),
                    acceptance_id: acceptance_id.clone(),
                })
        }),
    }
}

fn record<'a>(inputs: &'a Inputs, check: &CommandCheck) -> Result<&'a [u8]> {
    let name = check.evidence_file.as_deref().context(
        "Manual acceptance needs evidence_file relative to the external evidence directory",
    )?;
    external::record(inputs, name)
}

pub(super) async fn execute(
    check: &CommandCheck,
    plan: &Plan,
    snapshot: &Snapshot,
    policy: &PolicyEvidence,
    inputs: Option<&Arc<Inputs>>,
    artifacts: &Path,
) -> CheckResult {
    let mut result = CheckResult::pending(&check.id, check.required, check.severity);
    result.applicability = Applicability::Applicable;
    let expected = subject(
        check,
        plan,
        snapshot,
        policy,
        inputs.map_or("", |inputs| &inputs.store.repository),
    );
    result.metadata.insert(
        "manual_acceptance".into(),
        serde_json::json!({"expected_subject":expected,"verified":false}),
    );
    let Some(inputs) = inputs else {
        result.block(ExecutionStatus::Blocked, "Manual acceptance requires a verified external record and caller-supplied --trust-store and --evidence-dir");
        return result;
    };
    let checked = (|| -> Result<_> { Ok((record(inputs, check)?.to_vec(), now()?)) })();
    let outcome = async {
        let (bytes, checked_at) = checked?;
        result.execution.artifacts.push(
            super::evidence::persist(artifacts, &check.id, "manual-envelope.json", &bytes).await?,
        );
        result.execution.artifacts.push(
            super::evidence::persist(
                artifacts,
                &check.id,
                "manual-trust.json",
                &inputs.store_bytes,
            )
            .await?,
        );
        let owned_inputs = inputs.clone();
        let verification = tokio::task::spawn_blocking(move || {
            attestation::verify(&bytes, &owned_inputs.store, &expected, checked_at)
        });
        let verified = tokio::time::timeout(
            Duration::from_secs(check.timeout_seconds.min(30)),
            verification,
        )
        .await
        .context("Manual acceptance verification timed out")???;
        result.metadata.insert("manual_acceptance".into(), serde_json::json!({
            "expected_subject":verified.record.subject,"verified":true,
            "method":"ed25519_dsse","checked_at":checked_at,"evidence":verified,
            "trust_source":inputs.store_path,"trust_digest":snapshot::digest(&inputs.store_bytes)
        }));
        if verified.record.decision == ManualDecision::Rejected {
            result.diagnostics.push(diagnostic(
                &check.id,
                None,
                None,
                format!("Manual acceptance rejected: {}", verified.record.reason),
                serde_json::to_value(&verified)?,
                "Address the review findings and obtain a new signed acceptance record",
                &verified.record.record_id,
            ));
        }
        result.matched_entities = 1;
        result.complete();
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if let Err(error) = outcome {
        result.block(
            ExecutionStatus::Blocked,
            format!("Manual acceptance could not be verified: {error:#}"),
        );
    }
    result
}

pub(super) async fn revalidate(
    inputs: Arc<Inputs>,
    plan: &Plan,
    snapshot: &Snapshot,
    policy: &PolicyEvidence,
    results: &mut [CheckResult],
) -> Result<()> {
    let checks: Vec<_> = plan
        .commands
        .iter()
        .filter(|check| check.kind == CheckKind::Manual)
        .filter(|check| {
            results.iter().any(|result| {
                result.id == check.id && result.execution.status == ExecutionStatus::Completed
            })
        })
        .map(|check| {
            (
                check.clone(),
                subject(check, plan, snapshot, policy, &inputs.store.repository),
            )
        })
        .collect();
    let validation = tokio::task::spawn_blocking(move || -> Result<_> {
        unchanged(&inputs)?;
        let now = now()?;
        Ok(checks.into_iter().filter_map(|(check, expected)| {
            let validated = record(&inputs, &check).and_then(|bytes| attestation::verify(bytes, &inputs.store, &expected, now));
            validated.err().map(|error| (check.id, format!("Manual acceptance expired or became invalid before completion: {error:#}")))
        }).collect::<BTreeMap<_, _>>())
    }).await??;
    for result in results {
        if let Some(error) = validation.get(&result.id) {
            result.block(ExecutionStatus::Blocked, error);
            result
                .metadata
                .get_mut("manual_acceptance")
                .expect("manual evidence")["valid_at_completion"] = serde_json::json!(false);
        } else if result.execution.status == ExecutionStatus::Completed
            && let Some(evidence) = result.metadata.get_mut("manual_acceptance")
        {
            evidence["valid_at_completion"] = serde_json::json!(true);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "manual_tests.rs"]
mod tests;
