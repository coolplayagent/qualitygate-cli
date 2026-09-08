use super::report_gate::map_file;
use crate::{
    adapters::{reports::Data, rules::diagnostic},
    config::{IncrementMode, ReportSpec},
    domain::CheckResult,
    snapshot::Snapshot,
};
use anyhow::{Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn apply(
    result: &mut CheckResult,
    spec: &ReportSpec,
    data: &Data,
    snapshot: &Snapshot,
    workspace: &Path,
) -> Result<()> {
    if spec.coverage_paths.is_empty() {
        bail!("Coverage requires explicit coverage_paths");
    }
    if ![IncrementMode::Full, IncrementMode::ChangedLines].contains(&spec.mode) {
        bail!("Coverage supports full or changed_lines mode");
    }
    if spec.require_branch_coverage && !data.branch_coverage {
        bail!("Required branch coverage evidence is missing");
    }
    let mut builder = globset::GlobSetBuilder::new();
    for path in &spec.coverage_paths {
        builder.add(globset::Glob::new(path)?);
    }
    let matcher = builder.build()?;
    let expected: BTreeSet<_> = snapshot
        .files
        .keys()
        .filter(|path| {
            matcher.is_match(path)
                && snapshot.includes(path)
                && (spec.mode == IncrementMode::Full
                    || snapshot
                        .changes
                        .get(*path)
                        .is_some_and(|change| !change.added_lines.is_empty()))
        })
        .cloned()
        .collect();
    let mut inventory = BTreeSet::new();
    for file in &data.coverage_files {
        inventory.insert(map_file(file, snapshot, workspace, false)?);
    }
    let mut records = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for record in &data.coverage {
        let file = map_file(&record.file, snapshot, workspace, false)?;
        let lines = *counts.entry(file.clone()).or_insert_with(|| {
            snapshot.files[&file]
                .bytes
                .split_inclusive(|byte| *byte == b'\n')
                .count()
        });
        if record.line == 0 || record.line > lines || record.branches_hit > record.branches_found {
            bail!(
                "Coverage record is outside checked source or has invalid counters: {file}:{}",
                record.line
            );
        }
        if records
            .insert((file.clone(), record.line), record)
            .is_some()
        {
            bail!(
                "Duplicate coverage record after source mapping: {file}:{}",
                record.line
            );
        }
        inventory.insert(file);
    }
    let missing: Vec<_> = expected.difference(&inventory).collect();
    if !missing.is_empty() {
        bail!(
            "Coverage report omitted sources in its configured scope: {}",
            missing.into_iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    let mut total = 0usize;
    let mut hit = 0usize;
    let mut branches = 0usize;
    let mut branches_hit = 0usize;
    for ((file, line), record) in records {
        if !expected.contains(&file) {
            continue;
        }
        if spec.mode == IncrementMode::ChangedLines
            && !snapshot.changes[&file].added_lines.contains(&line)
        {
            continue;
        }
        total += 1;
        hit += usize::from(record.hits > 0);
        branches = branches
            .checked_add(record.branches_found)
            .ok_or_else(|| anyhow::anyhow!("Branch count overflow"))?;
        branches_hit = branches_hit
            .checked_add(record.branches_hit)
            .ok_or_else(|| anyhow::anyhow!("Branch count overflow"))?;
    }
    let percentage =
        |hit: usize, total: usize| (total > 0).then(|| hit as f64 * 100.0 / total as f64);
    let line_percent = percentage(hit, total);
    let branch_percent = percentage(branches_hit, branches);
    let threshold = spec.minimum_coverage.unwrap_or(100.0);
    result.matched_entities += total;
    let evidence = serde_json::json!({"coverage_paths":spec.coverage_paths,"source_files":expected,"lines":total,"lines_hit":hit,"line_percent":line_percent,"branches":branches,"branches_hit":branches_hit,"branch_percent":branch_percent,"require_branch_coverage":spec.require_branch_coverage,"threshold":threshold,"no_executable_lines_selected":total == 0});
    result
        .metadata
        .insert(format!("{}:coverage", spec.path), evidence.clone());
    if line_percent.is_some_and(|value| value < threshold)
        || spec.require_branch_coverage && branch_percent.is_some_and(|value| value < threshold)
    {
        result.diagnostics.push(diagnostic(
            &result.id,
            None,
            None,
            format!(
                "Coverage below {threshold}%: lines {line_percent:?}, branches {branch_percent:?}"
            ),
            evidence,
            "Add tests for the uncovered behavior and branches",
            "coverage-threshold",
        ));
    }
    Ok(())
}
