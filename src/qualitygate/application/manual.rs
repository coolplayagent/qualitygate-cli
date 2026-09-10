//! External trust inputs are loaded before commands and revalidated before publication.

use crate::{
    adapters::{attestation, rules::diagnostic},
    config::{self, CheckKind, CommandCheck, Plan, attestation::TrustStore},
    domain::*,
    paths,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) struct Inputs {
    store: TrustStore,
    store_path: PathBuf,
    store_bytes: Vec<u8>,
    records: BTreeMap<String, Result<(PathBuf, Vec<u8>), String>>,
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        bail!(
            "External evidence must be a regular non-symlink file: {}",
            path.display()
        );
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("External evidence is not a regular file");
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!("External evidence exceeds its byte budget");
    }
    Ok(bytes)
}

pub(super) fn load(
    root: &Path,
    store_path: &Path,
    evidence_directory: &Path,
    checks: &[CommandCheck],
) -> Result<Inputs> {
    let started = Instant::now();
    let root = dunce::canonicalize(root)?;
    let canonical_store = dunce::canonicalize(store_path)?;
    let directory = dunce::canonicalize(evidence_directory)?;
    if canonical_store.starts_with(&root) || directory.starts_with(&root) || !directory.is_dir() {
        bail!("Trust store and evidence directory must be outside the checked repository");
    }
    let store_bytes = read(store_path, 256 * 1024)?;
    let store = config::attestation::parse(&store_bytes)?;
    attestation::validate_keys(&store)?;
    let mut records = BTreeMap::new();
    let mut total = store_bytes.len();
    for check in checks
        .iter()
        .filter(|check| check.kind == CheckKind::Manual)
    {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("External evidence loading exceeded its 30-second budget");
        }
        if let Some(name) = &check.evidence_file {
            if records.contains_key(name) {
                continue;
            }
            if records.len() >= 128 {
                bail!("External evidence exceeds 128 records");
            }
            let record = (|| -> Result<_> {
                let path = paths::confined(&directory, Path::new(name))?;
                let bytes = read(&path, attestation::MAX_ENVELOPE_BYTES)?;
                Ok((path, bytes))
            })()
            .map_err(|error| format!("{error:#}"));
            if let Ok((_, bytes)) = &record {
                total += bytes.len();
            }
            if total > 8 * 1024 * 1024 {
                bail!("External evidence exceeds 8 MiB total");
            }
            records.insert(name.clone(), record);
        }
    }
    Ok(Inputs {
        store,
        store_path: store_path.to_owned(),
        store_bytes,
        records,
    })
}

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

fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn record<'a>(inputs: &'a Inputs, check: &CommandCheck) -> Result<&'a [u8]> {
    let name = check.evidence_file.as_ref().context(
        "Manual acceptance needs evidence_file relative to the external evidence directory",
    )?;
    match inputs.records.get(name) {
        Some(Ok((_, bytes))) => Ok(bytes),
        Some(Err(error)) => bail!("Cannot read external acceptance record {name}: {error}"),
        None => bail!("External acceptance record is unavailable: {name}"),
    }
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

fn unchanged(inputs: &Inputs) -> Result<()> {
    if read(&inputs.store_path, 256 * 1024)? != inputs.store_bytes {
        bail!("External trust store changed during checks");
    }
    for (path, bytes) in inputs
        .records
        .values()
        .filter_map(|record| record.as_ref().ok())
    {
        if read(path, attestation::MAX_ENVELOPE_BYTES)? != *bytes {
            bail!(
                "External acceptance record changed during checks: {}",
                path.display()
            );
        }
    }
    Ok(())
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
