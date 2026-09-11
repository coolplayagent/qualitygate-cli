//! Relate declarations to the last byte-changing commits of actual test entities.

use super::{entity_changes, markers, syntax::Entity};
use crate::{
    config::{Marker, RuleSetting, catalog::Entry},
    domain::SnapshotBinding,
    snapshot::{self, File, Snapshot, history::History},
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct Anchor {
    path: String,
    symbol: String,
    start_line: usize,
    end_line: usize,
    body_digest: String,
}

fn anchor(path: &str, entity: &Entity) -> Anchor {
    Anchor {
        path: path.into(),
        symbol: entity.symbol.clone(),
        start_line: entity.range.start_line,
        end_line: entity.range.end_line,
        body_digest: entity.body_digest.clone(),
    }
}

// None represents an uncommitted entity change, which has no commit declaration.
type Origins = BTreeSet<Option<String>>;
type Entities = BTreeMap<Anchor, Origins>;
type Candidates = BTreeMap<Anchor, Option<Origins>>;

#[derive(Debug, Clone)]
pub struct GitFacts {
    binding: SnapshotBinding,
    base_digest: String,
    current: Entities,
    baseline: Entities,
    trailers: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    evidence: serde_json::Value,
}

pub fn needed(setting: &RuleSetting, entry: &Entry) -> bool {
    setting
        .parameters
        .get("marker")
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("git_trailer")
        || entry
            .custom
            .as_ref()
            .and_then(|rule| rule.binding.as_ref())
            .is_some_and(|binding| binding.marker.kind == "git_trailer")
}

impl GitFacts {
    pub fn ensure_binding(&self, snapshot: &Snapshot) -> Result<()> {
        if self.binding != SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone())
            || self.base_digest != snapshot::content_digest(&snapshot.base_files)
            || self.binding.content_digest != snapshot::content_digest(&snapshot.files)
        {
            bail!("Git declaration facts belong to a different snapshot");
        }
        Ok(())
    }

    pub fn evidence(&self) -> &serde_json::Value {
        &self.evidence
    }

    pub(super) fn declaration(
        &self,
        marker: &Marker,
        path: &str,
        entity: &Entity,
        baseline: bool,
    ) -> Result<Option<String>> {
        let entities = if baseline {
            &self.baseline
        } else {
            &self.current
        };
        let origins = entities
            .get(&anchor(path, entity))
            .context("Git declaration entity could not be associated")?;
        let mut declarations = Vec::new();
        for origin in origins {
            let Some(oid) = origin else {
                return Ok(None);
            };
            let Some(values) = self.trailers[oid].get(&marker.name.to_ascii_lowercase()) else {
                return Ok(None);
            };
            if values.len() != 1 {
                bail!(
                    "Ambiguous duplicate {} trailers in commit {oid}",
                    marker.name
                );
            }
            let value = &values[0];
            if value.trim().is_empty() {
                return Ok(None);
            }
            declarations.push(value);
        }
        // Each contributing parent must independently satisfy the field contract.
        if declarations.iter().any(|value| {
            let fields = markers::fields(value);
            marker.fields.iter().any(|field| {
                fields
                    .get(field)
                    .is_none_or(|value| value.trim().is_empty())
            })
        }) {
            return Ok(Some(String::new()));
        }
        Ok(declarations.first().map(|value| (*value).clone()))
    }
}

fn files(commit: &snapshot::history::Commit) -> BTreeMap<String, File> {
    commit
        .files
        .iter()
        .map(|(path, file)| (path.clone(), file.as_ref().clone()))
        .collect()
}

fn candidates(
    before: &BTreeMap<String, File>,
    after: &BTreeMap<String, File>,
    previous: &Entities,
    local: bool,
) -> Result<Candidates> {
    let changes = snapshot::compare_files(before, after);
    let mut result: Candidates = previous
        .iter()
        .filter(|(key, _)| after.contains_key(&key.path) && !changes.contains_key(&key.path))
        .map(|(key, origins)| (key.clone(), Some(origins.clone())))
        .collect();
    for file in entity_changes::collect_between(before, after)? {
        for test in file.tests {
            if test.ambiguous {
                bail!(
                    "Ambiguous Git test lineage: {} {}",
                    file.path,
                    test.entity.symbol
                );
            }
            let inherited = test.previous.as_ref().zip(test.previous_path.as_deref());
            let unchanged = inherited.filter(|(old, path)| {
                !(local && test.kind == "renamed")
                    && before[*path].bytes[old.byte_range.clone()]
                        == after[&file.path].bytes[test.entity.byte_range.clone()]
            });
            let origins = unchanged
                .map(|(old, path)| {
                    previous
                        .get(&anchor(path, old))
                        .cloned()
                        .context("Previous Git entity identity is unavailable")
                })
                .transpose()?;
            result.insert(anchor(&file.path, &test.entity), origins);
        }
    }
    Ok(result)
}

pub fn analyze(snapshot: &Snapshot, history: &History) -> Result<GitFacts> {
    let started = Instant::now();
    let mut states: BTreeMap<String, Entities> = BTreeMap::new();
    let mut nodes: BTreeMap<String, &snapshot::history::Commit> = BTreeMap::new();
    let mut total_entities = 0;
    for commit in &history.commits {
        if started.elapsed() > Duration::from_secs(60) {
            bail!("Git declaration analysis exceeded its 60-second budget");
        }
        let output = files(commit);
        if snapshot::content_digest(&output) != commit.content_digest
            || nodes.contains_key(&commit.oid)
        {
            bail!("Invalid or duplicate Git history node");
        }
        let mut merged: Candidates = BTreeMap::new();
        if commit.parents.is_empty() {
            merged = candidates(&BTreeMap::new(), &output, &BTreeMap::new(), false)?;
        }
        for parent in &commit.parents {
            if started.elapsed() > Duration::from_secs(60) {
                bail!("Git merge-parent analysis exceeded its 60-second budget");
            }
            let previous = nodes
                .get(parent)
                .context("Incomplete or unordered Git ancestry (including shallow history)")?;
            let from_parent = candidates(&files(previous), &output, &states[parent], false)?;
            for (key, origins) in from_parent {
                let current = merged.entry(key).or_default();
                if let Some(origins) = origins {
                    current.get_or_insert_default().extend(origins);
                }
            }
        }
        let state: Entities = merged
            .into_iter()
            .map(|(key, origins)| {
                (
                    key,
                    origins.unwrap_or_else(|| BTreeSet::from([Some(commit.oid.clone())])),
                )
            })
            .collect();
        total_entities += state.len();
        if state.len() > 50_000 || total_entities > 200_000 {
            bail!("Git declaration analysis exceeds its entity budget");
        }
        nodes.insert(commit.oid.clone(), commit);
        states.insert(commit.oid.clone(), state);
    }
    let base = nodes
        .get(&snapshot.identity.base)
        .context("Git history lacks the comparison base")?;
    let head = nodes
        .get(&snapshot.identity.head)
        .context("Git history lacks the checked head")?;
    let base_digest = snapshot::content_digest(&snapshot.base_files);
    if base.content_digest != base_digest
        || snapshot.identity.content_digest != snapshot::content_digest(&snapshot.files)
        || matches!(snapshot.identity.mode.as_str(), "diff" | "mr")
            && head.content_digest != snapshot.identity.content_digest
    {
        bail!("Git history does not reproduce the checked comparison");
    }
    let current: Entities = candidates(
        &files(head),
        &snapshot.files,
        &states[&snapshot.identity.head],
        true,
    )?
    .into_iter()
    .map(|(key, origins)| (key, origins.unwrap_or_else(|| BTreeSet::from([None]))))
    .collect();
    let baseline = states
        .remove(&snapshot.identity.base)
        .context("Missing baseline entities")?;
    let binding = SnapshotBinding::new(&snapshot.identity, snapshot.path_filter.clone());
    let records: Vec<_> = history.commits.iter().map(|commit| serde_json::json!({
        "commit":commit.oid,"parents":commit.parents,"content_digest":commit.content_digest,
        "message":commit.message,"message_digest":snapshot::digest(commit.message.as_bytes()),
        "trailers":commit.trailers
    })).collect();
    let entities = |values: &Entities| {
        values
            .iter()
            .map(|(entity, origins)| serde_json::json!({"entity":entity,"commits":origins}))
            .collect::<Vec<_>>()
    };
    let evidence = serde_json::json!({"schema_version":1,"snapshot":binding,
        "base_content_digest":base_digest,"method":"git_parent_entity_comparison",
        "declaration_only":true,"commits":records,"entities":entities(&current),
        "baseline_entities":entities(&baseline)});
    if serde_json::to_vec(&evidence)?.len() > 8 * 1024 * 1024 {
        bail!("Git declaration evidence exceeds 8 MiB");
    }
    Ok(GitFacts {
        binding,
        base_digest,
        current,
        baseline,
        evidence,
        trailers: history
            .commits
            .iter()
            .map(|commit| (commit.oid.clone(), commit.trailers.clone()))
            .collect(),
    })
}

#[cfg(test)]
#[path = "git_trailers_tests.rs"]
mod tests;
