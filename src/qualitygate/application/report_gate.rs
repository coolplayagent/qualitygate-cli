//! Applies explicit incremental policies to parsed tool results.

use crate::{
    adapters::{
        reports::{Data, Issue},
        rules::diagnostic,
    },
    config::{IncrementMode, ReportSpec},
    domain::{CheckResult, Range},
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path};

pub(super) fn apply(
    result: &mut CheckResult,
    spec: &ReportSpec,
    data: Data,
    baseline: Option<Data>,
    snapshot: &Snapshot,
    workspace: &Path,
) -> Result<()> {
    if let Some(tests) = &data.tests {
        let minimum = spec.minimum_tests.unwrap_or(1);
        result
            .metadata
            .insert("tests".into(), serde_json::to_value(tests)?);
        result.matched_entities += tests.executed;
        if tests.executed < minimum {
            result.diagnostics.push(diagnostic(
                &result.id,
                None,
                None,
                format!(
                    "Only {} tests executed; at least {minimum} required",
                    tests.executed
                ),
                serde_json::to_value(tests)?,
                "Ensure the intended test suite executes and does not only skip tests",
                "test-count",
            ));
        }
        if tests.failures > 0 {
            result.diagnostics.push(diagnostic(
                &result.id,
                None,
                None,
                format!("{} executed tests failed", tests.failures),
                serde_json::to_value(tests)?,
                "Repair the failing test behavior and rerun the suite",
                "test-failures",
            ));
        }
    } else if spec.minimum_tests.is_some() {
        bail!("Configured minimum_tests requires a test-count report");
    }
    if !data.coverage.is_empty() || !data.coverage_files.is_empty() {
        super::coverage_gate::apply(result, spec, &data, snapshot, workspace)?;
    } else if spec.minimum_coverage.is_some() || !spec.coverage_paths.is_empty() {
        bail!("Configured coverage gate requires a source inventory and coverage records");
    }
    let mut previous = BTreeMap::new();
    if spec.mode == IncrementMode::NewDiagnostics {
        for mut issue in baseline.context("Missing baseline analysis")?.issues {
            normalize_issue(&mut issue, snapshot, workspace, true)?;
            *previous.entry(fingerprint(&issue)).or_insert(0usize) += 1;
        }
    }
    let affected = if spec.mode == IncrementMode::AffectedScope {
        Some(
            data.affected_files
                .as_ref()
                .context("Analyzer did not supply affected_files")?
                .iter()
                .map(|file| map_file(file, snapshot, workspace, false))
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };
    let mut filtered = 0;
    for mut issue in data.issues {
        normalize_issue(&mut issue, snapshot, workspace, false)?;
        let include = match spec.mode {
            IncrementMode::Full => true,
            IncrementMode::ChangedLines => {
                let file = issue
                    .file
                    .as_ref()
                    .context("Cannot map a location-free diagnostic to changed lines")?;
                let line = issue
                    .line
                    .context("Cannot map a diagnostic without a line to changed lines")?;
                snapshot
                    .changes
                    .get(file)
                    .is_some_and(|change| change.added_lines.contains(&line))
            }
            IncrementMode::AffectedScope => {
                let file = issue
                    .file
                    .as_ref()
                    .context("Cannot map diagnostic to affected scope")?;
                affected.as_ref().is_some_and(|files| files.contains(file))
            }
            IncrementMode::NewDiagnostics => match previous.get_mut(&fingerprint(&issue)) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    false
                }
                _ => true,
            },
        };
        if !include {
            filtered += 1;
            continue;
        }
        let identity = fingerprint(&issue);
        result.diagnostics.push(diagnostic(
            &result.id,
            issue.file.as_deref(),
            issue.line.map(|line| Range {
                start_line: line,
                end_line: line,
            }),
            issue.message.clone(),
            serde_json::to_value(&issue)?,
            "Repair the reported issue and rerun the same analyzer",
            &identity,
        ));
    }
    result.metadata.insert(
        format!("{}:mode", spec.path),
        serde_json::to_value(spec.mode)?,
    );
    result
        .metadata
        .insert(format!("{}:filtered", spec.path), filtered.into());
    Ok(())
}

#[cfg(test)]
#[path = "report_gate_tests.rs"]
mod tests;

fn fingerprint(issue: &Issue) -> String {
    snapshot::digest(
        format!(
            "{}\0{}\0{}\0{}",
            issue.rule,
            issue.file.as_deref().unwrap_or(""),
            issue.symbol.as_deref().unwrap_or(""),
            issue.message
        )
        .as_bytes(),
    )
}

fn normalize_issue(
    issue: &mut Issue,
    snapshot: &Snapshot,
    workspace: &Path,
    base: bool,
) -> Result<()> {
    if let Some(file) = &issue.file {
        let mapped = map_file(file, snapshot, workspace, base)?;
        issue.file = Some(if base {
            snapshot
                .changes
                .iter()
                .find(|(_, change)| {
                    change.kind == "renamed" && change.old_path.as_deref() == Some(&mapped)
                })
                .map(|(path, _)| path.clone())
                .unwrap_or(mapped)
        } else {
            mapped
        });
    }
    Ok(())
}

pub(super) fn map_file(
    file: &str,
    snapshot: &Snapshot,
    workspace: &Path,
    base: bool,
) -> Result<String> {
    let files = if base {
        &snapshot.base_files
    } else {
        &snapshot.files
    };
    let path = Path::new(file.strip_prefix("file://").unwrap_or(file));
    let path = path
        .strip_prefix(workspace)
        .or_else(|_| path.strip_prefix(&snapshot.root))
        .unwrap_or(path);
    let relative = crate::paths::relative(path)?;
    if files.contains_key(&relative) {
        return Ok(relative);
    }
    let suffix = format!("/{relative}");
    let matches: Vec<_> = files
        .keys()
        .filter(|name| name.ends_with(&suffix))
        .collect();
    match matches.as_slice() {
        [name] => Ok((*name).clone()),
        _ => bail!("Report path cannot be mapped uniquely to checked sources: {file}"),
    }
}
