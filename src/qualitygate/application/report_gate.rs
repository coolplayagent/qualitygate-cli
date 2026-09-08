//! Applies explicit incremental policies to parsed tool results.

use crate::{
    adapters::{
        reports::{self, Data, Issue},
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
    } else if spec.minimum_tests.is_some() {
        bail!("Configured minimum_tests requires a test-count report");
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
    if !data.coverage.is_empty() {
        coverage(result, spec, data.coverage, snapshot, workspace)?;
    } else if spec.minimum_coverage.is_some() {
        bail!("Configured minimum_coverage requires executable coverage records");
    }
    Ok(())
}

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

fn map_file(file: &str, snapshot: &Snapshot, workspace: &Path, base: bool) -> Result<String> {
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

fn coverage(
    result: &mut CheckResult,
    spec: &ReportSpec,
    records: Vec<reports::CoverageLine>,
    snapshot: &Snapshot,
    workspace: &Path,
) -> Result<()> {
    if ![IncrementMode::Full, IncrementMode::ChangedLines].contains(&spec.mode) {
        bail!("Coverage supports full or changed_lines mode");
    }
    let mut total = 0;
    let mut hit = 0;
    let mut branches = 0;
    let mut branches_hit = 0;
    for record in records {
        let file = map_file(&record.file, snapshot, workspace, false)?;
        if !snapshot.includes(&file) {
            continue;
        }
        if spec.mode == IncrementMode::ChangedLines
            && !snapshot
                .changes
                .get(&file)
                .is_some_and(|change| change.added_lines.contains(&record.line))
        {
            continue;
        }
        total += 1;
        hit += usize::from(record.hits > 0);
        branches += record.branches_found;
        branches_hit += record.branches_hit;
    }
    let line_percent = if total == 0 {
        100.0
    } else {
        hit as f64 * 100.0 / total as f64
    };
    let branch_percent = if branches == 0 {
        100.0
    } else {
        branches_hit as f64 * 100.0 / branches as f64
    };
    let threshold = spec.minimum_coverage.unwrap_or(100.0);
    result.matched_entities += total;
    let evidence = serde_json::json!({"lines":total,"lines_hit":hit,"line_percent":line_percent,"branches":branches,"branches_hit":branches_hit,"branch_percent":branch_percent,"threshold":threshold});
    result
        .metadata
        .insert(format!("{}:coverage", spec.path), evidence.clone());
    if line_percent < threshold || branch_percent < threshold {
        result.diagnostics.push(diagnostic(&result.id, None, None, format!("Coverage below {threshold}%: lines {line_percent:.2}%, branches {branch_percent:.2}%"), evidence, "Add tests for the uncovered changed behavior and branches", "coverage-threshold"));
    }
    Ok(())
}
