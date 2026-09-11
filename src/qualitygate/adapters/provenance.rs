//! Replay signed run snapshots, then derive test participation from actual entities.

use super::{attestation, entity_changes, syntax::Entity};
use crate::{
    config::attestation::TrustStore,
    domain::*,
    snapshot::{self, File, Snapshot},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};

pub const PAYLOAD_TYPE: &str = "application/vnd.qualitygate.agent-provenance.v1+json";
pub const MAX_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct EntityKey {
    path: String,
    symbol: String,
    start_line: usize,
    end_line: usize,
    body_digest: String,
}

fn key(path: &str, entity: &Entity) -> EntityKey {
    EntityKey {
        path: path.into(),
        symbol: entity.symbol.clone(),
        start_line: entity.range.start_line,
        end_line: entity.range.end_line,
        body_digest: entity.body_digest.clone(),
    }
}

#[derive(Debug, Clone)]
pub struct ProvenanceFacts {
    origins: BTreeMap<EntityKey, BTreeSet<String>>,
    actors: BTreeMap<String, ActorKind>,
    evidence: serde_json::Value,
    expires_at: u64,
    subject: ProvenanceSubject,
    record_id: String,
    issued_at: u64,
}

impl ProvenanceFacts {
    pub fn evidence(&self) -> &serde_json::Value {
        &self.evidence
    }
    pub fn ensure_binding(&self, snapshot: &Snapshot, rule_id: &str) -> Result<()> {
        if self.subject.rule_id != rule_id
            || self.subject.snapshot
                != SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone())
            || self.subject.input_content_digest != snapshot::content_digest(&snapshot.base_files)
        {
            bail!("Verified provenance belongs to a different rule or snapshot");
        }
        Ok(())
    }

    pub fn revalidate_time(&self, store: &TrustStore, now: u64) -> Result<()> {
        attestation::validity(store, &self.record_id, self.issued_at, self.expires_at, now)
    }
    pub fn agent_runs(&self, path: &str, entity: &Entity) -> Vec<String> {
        self.origins
            .get(&key(path, entity))
            .into_iter()
            .flatten()
            .filter(|id| self.actors.get(*id) == Some(&ActorKind::Agent))
            .cloned()
            .collect()
    }

    pub fn participated(&self, path: &str, entity: &Entity) -> bool {
        !self.agent_runs(path, entity).is_empty()
    }
}

pub fn file_identity(file: &File) -> ProvenanceFileIdentity {
    ProvenanceFileIdentity {
        digest: snapshot::digest(&file.bytes),
        bytes: file.bytes.len(),
        executable: file.executable,
    }
}

pub fn subject(
    snapshot: &Snapshot,
    policy: &PolicyEvidence,
    repository: &str,
    rule_id: &str,
) -> ProvenanceSubject {
    ProvenanceSubject {
        repository: repository.into(),
        snapshot: SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone()),
        policy: PolicyBinding::from(policy),
        rule_id: rule_id.into(),
        input_content_digest: snapshot::content_digest(&snapshot.base_files),
    }
}

pub fn verify(
    bytes: &[u8],
    store: &TrustStore,
    expected: &ProvenanceSubject,
    snapshot: &Snapshot,
    now: u64,
) -> Result<ProvenanceFacts> {
    let authenticated =
        attestation::authenticate(bytes, store, PAYLOAD_TYPE, MAX_ENVELOPE_BYTES, |key| {
            key.provenance_rules.contains(&expected.rule_id)
        })?;
    let record: ProvenanceRecord = serde_json::from_slice(&authenticated.payload)
        .context("Invalid signed provenance record")?;
    if record.schema_version != 1
        || record.subject != *expected
        || record.subject.repository != store.repository
        || expected.snapshot
            != SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone())
        || expected.input_content_digest != snapshot::content_digest(&snapshot.base_files)
        || snapshot.identity.content_digest != snapshot::content_digest(&snapshot.files)
    {
        bail!(
            "Provenance record does not match the repository, input/output snapshots, policy or rule"
        );
    }
    if record.record_id.trim().is_empty()
        || record.record_id.len() > 256
        || record.runs.len() > 256
        || record.steps.len() > 256
    {
        bail!("Provenance requires a bounded record ID and at most 256 runs/steps");
    }
    attestation::validity(
        store,
        &record.record_id,
        record.issued_at,
        record.expires_at,
        now,
    )?;
    let mut actors = BTreeMap::new();
    for run in &record.runs {
        if run.run_id.trim().is_empty()
            || run.run_id.len() > 256
            || run.actor.id.trim().is_empty()
            || run.actor.id.len() > 1024
            || run
                .actor
                .version
                .as_ref()
                .is_some_and(|version| version.trim().is_empty() || version.len() > 256)
            || run.actor.kind == ActorKind::Agent && run.actor.version.is_none()
            || run.started_at > run.ended_at
            || run.ended_at > record.issued_at
            || actors.insert(run.run_id.clone(), run.actor.kind).is_some()
        {
            bail!(
                "Run IDs, actor identities/versions and execution intervals must be valid and unambiguous"
            );
        }
    }
    let origins = replay(&record, snapshot, &actors)?;
    let agent_entities = origins
        .values()
        .filter(|ids| {
            ids.iter()
                .any(|id| actors.get(id) == Some(&ActorKind::Agent))
        })
        .count();
    let participation: Vec<_> = origins
        .iter()
        .filter_map(|(entity, ids)| {
            let agent_runs: Vec<_> = ids
                .iter()
                .filter(|id| actors.get(*id) == Some(&ActorKind::Agent))
                .collect();
            (!agent_runs.is_empty())
                .then(|| serde_json::json!({"entity":entity,"agent_run_ids":agent_runs}))
        })
        .collect();
    let evidence = serde_json::json!({"record_id":record.record_id,"subject":record.subject,"coverage":record.coverage,
        "signer_key_id":authenticated.signer_key_id,"public_key_digest":authenticated.public_key_digest,
        "issued_at":record.issued_at,"expires_at":record.expires_at,"checked_at":now,
        "runs":record.runs,"steps_replayed":record.steps.len(),"agent_test_entities":agent_entities,"participation":participation});
    Ok(ProvenanceFacts {
        origins,
        actors,
        evidence,
        expires_at: record.expires_at,
        subject: record.subject,
        record_id: record.record_id,
        issued_at: record.issued_at,
    })
}

fn replay(
    record: &ProvenanceRecord,
    snapshot: &Snapshot,
    actors: &BTreeMap<String, ActorKind>,
) -> Result<BTreeMap<EntityKey, BTreeSet<String>>> {
    let started = Instant::now();
    let mut state = snapshot.base_files.clone();
    let mut origins: BTreeMap<EntityKey, BTreeSet<String>> = BTreeMap::new();
    let mut used_runs = BTreeSet::new();
    let mut total_changes = 0;
    for step in &record.steps {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("Provenance replay exceeded its 30-second budget");
        }
        total_changes += step.changes.len();
        if total_changes > 8192 {
            bail!("Provenance replay exceeds 8192 file changes");
        }
        if !actors.contains_key(&step.run_id)
            || step.input_content_digest != snapshot::content_digest(&state)
        {
            bail!("Unknown run or broken provenance input snapshot chain");
        }
        used_runs.insert(step.run_id.clone());
        let previous = state.clone();
        let mut changed = BTreeSet::new();
        for change in &step.changes {
            if change.path.is_empty()
                || crate::paths::relative(Path::new(&change.path))? != change.path
                || !changed.insert(change.path.clone())
            {
                bail!(
                    "Provenance change paths must be normalized, confined and unique within a step"
                );
            }
            if change.before != state.get(&change.path).map(file_identity) {
                bail!(
                    "Provenance file input identity does not match: {}",
                    change.path
                );
            }
            let after = change
                .after
                .as_ref()
                .map(|after| -> Result<File> {
                    let bytes = attestation::decode(&after.content_base64)?;
                    if bytes.len() > snapshot::MAX_FILE_BYTES {
                        bail!("Provenance file exceeds the snapshot byte budget");
                    }
                    Ok(File {
                        bytes,
                        executable: after.executable,
                    })
                })
                .transpose()?;
            if after.as_ref().map(file_identity) == change.before {
                bail!("Provenance file change has no effect: {}", change.path);
            }
            if let Some(file) = after {
                state.insert(change.path.clone(), file);
            } else {
                state.remove(&change.path);
            }
        }
        if state.len() > snapshot::MAX_FILES
            || state.values().map(|file| file.bytes.len()).sum::<usize>()
                > snapshot::MAX_SNAPSHOT_BYTES
        {
            bail!("Replayed snapshot exceeds file count or total byte budget");
        }
        if snapshot::content_digest(&state) != step.output_content_digest {
            bail!("Provenance output snapshot digest does not match replayed bytes");
        }
        let prior_origins = origins.clone();
        origins.retain(|entity, _| !changed.contains(&entity.path));
        for file in entity_changes::collect_between(&previous, &state)? {
            for test in file.tests {
                if test.ambiguous {
                    bail!(
                        "Ambiguous test lineage during provenance replay: {} {}",
                        file.path,
                        test.entity.symbol
                    );
                }
                let inherited = test.previous.as_ref().zip(test.previous_path.as_deref());
                let mut participation = inherited
                    .and_then(|(entity, path)| prior_origins.get(&key(path, entity)))
                    .cloned()
                    .unwrap_or_default();
                let edited = test.kind != "unchanged"
                    || inherited.is_some_and(|(entity, path)| {
                        previous[path].bytes[entity.byte_range.clone()]
                            != state[&file.path].bytes[test.entity.byte_range.clone()]
                    });
                if edited {
                    participation.insert(step.run_id.clone());
                }
                origins.insert(key(&file.path, &test.entity), participation);
            }
        }
        if origins.len() > 50_000 {
            bail!("Provenance replay exceeds 50000 test entities");
        }
    }
    if snapshot::content_digest(&state) != snapshot.identity.content_digest
        || used_runs.len() != actors.len()
    {
        bail!("Provenance does not cover the complete comparison or contains unused run IDs");
    }
    Ok(origins)
}

#[cfg(test)]
#[path = "provenance_tests.rs"]
mod tests;
