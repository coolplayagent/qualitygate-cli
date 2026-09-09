//! Dependency directions from ecosystem facts, with explicit complete module inventory.

use super::rules::diagnostic;
use crate::{
    config::{
        RuleSetting,
        project_rules::{self, DependencyKind},
    },
    domain::*,
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
#[cfg(test)]
#[path = "project_rules_tests.rs"]
mod tests;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

pub(super) fn module_boundary(
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    facts: &[ProjectFacts],
) -> Result<()> {
    let policy = project_rules::module_boundary(setting)?;
    let directions = policy
        .forbidden
        .iter()
        .map(|direction| {
            Ok((
                globset::Glob::new(&direction.from)?.compile_matcher(),
                globset::Glob::new(&direction.to)?.compile_matcher(),
                direction,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut projects = Vec::new();
    let mut identities = BTreeSet::new();
    for root in &policy.modules {
        let candidates: Vec<_> = facts
            .iter()
            .filter(|project| &project.root == root)
            .collect();
        if candidates.len() != 1 {
            bail!(
                "Module {root} requires exactly one project facts producer; found {}",
                candidates.len()
            );
        }
        let project = candidates[0];
        if project.schema_version != 1
            || project.ecosystem != "maven"
            || project.snapshot_digest != snapshot.identity.content_digest
        {
            bail!("Unsupported or foreign project facts for module {root}");
        }
        if !snapshot.files.contains_key(&project.manifest) {
            bail!(
                "Module manifest is not in the checked snapshot: {}",
                project.manifest
            );
        }
        let coordinate = project
            .coordinate
            .rsplit_once(':')
            .map(|(coordinate, _)| coordinate)
            .context("Malformed module coordinate")?;
        if !identities.insert(coordinate) {
            bail!("Multiple configured modules have the same identity: {coordinate}");
        }
        projects.push((coordinate, project));
    }
    result
        .metadata
        .insert("increment_mode".into(), serde_json::json!("full"));
    result.metadata.insert(
        "dependency_kind".into(),
        serde_json::json!(policy.dependency_kind),
    );
    result.metadata.insert("module_inventory".into(), serde_json::json!(projects.iter().map(|(id, project)|
        serde_json::json!({"module":id,"root":project.root,"manifest":project.manifest,"producer":project.producer_check})).collect::<Vec<_>>()));
    let started = Instant::now();
    for (from, project) in projects {
        let dependencies = match policy.dependency_kind {
            DependencyKind::Declared => &project.declared,
            DependencyKind::Resolved => &project.resolved,
        };
        // A transitive artifact may occur on multiple graph paths. A single
        // module/classpath relationship receives one diagnostic, not path duplicates.
        let mut edges: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        for dependency in dependencies.iter().collect::<BTreeSet<_>>() {
            if started.elapsed() > Duration::from_secs(30) {
                bail!("Module boundary analysis exceeded 30 seconds");
            }
            result.matched_entities += 1;
            let to = format!("{}:{}", dependency.group, dependency.artifact);
            for (index, (source, target, direction)) in directions.iter().enumerate() {
                if source.is_match(from)
                    && target.is_match(&to)
                    && (direction.scopes.is_empty() || direction.scopes.contains(&dependency.scope))
                {
                    edges
                        .entry((
                            to.clone(),
                            dependency.artifact_type.clone(),
                            dependency.classifier.clone(),
                            dependency.scope.clone(),
                        ))
                        .or_default()
                        .push(index);
                }
            }
        }
        for ((to, artifact_type, classifier, scope), mut directions) in edges {
            directions.sort_unstable();
            directions.dedup();
            result.diagnostics.push(diagnostic(&result.id, Some(&project.manifest), None,
                format!("Forbidden module dependency: {from} -> {to} ({scope})"),
                serde_json::json!({"from":from, "to":to, "scope":scope, "type":artifact_type, "classifier":classifier,
                    "dependency_kind":policy.dependency_kind, "matching_directions":directions, "producer":project.producer_check,
                    "snapshot_digest":project.snapshot_digest}),
                "Move the dependency or shared interface to the permitted module, repair affected callers, then rebuild and recheck",
                &format!("{from}:{to}:{artifact_type}:{classifier}:{scope}")));
        }
    }
    Ok(())
}
