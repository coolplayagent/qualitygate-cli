//! Applies explicit incremental policies to parsed tool results.

use crate::{
    adapters::{
        reports::{Data, Issue, IssueLocation},
        rules::diagnostic,
    },
    config::{IncrementMode, ReportSpec},
    domain::{CheckResult, Range, ratchet},
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn apply(
    result: &mut CheckResult,
    spec: &ReportSpec,
    mut data: Data,
    baseline: Option<Data>,
    snapshot: &Snapshot,
    workspace: &Path,
) -> Result<()> {
    if !data.sarif_runs.is_empty() {
        result.metadata.insert(
            format!("{}:sarif_runs", spec.path),
            serde_json::to_value(&data.sarif_runs)?,
        );
    }
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
    let mut effective_mode = spec.mode;
    if !data.coverage.is_empty() || !data.coverage_files.is_empty() {
        effective_mode = super::coverage_gate::apply(result, spec, &data, snapshot, workspace)?;
        result.metadata.insert(
            format!("{}:configured_mode", spec.path),
            serde_json::to_value(spec.mode)?,
        );
    } else if spec.minimum_coverage.is_some() || !spec.coverage_paths.is_empty() {
        bail!("Configured coverage gate requires a source inventory and coverage records");
    }
    let mut previous = BTreeMap::new();
    let mut line_counts = BTreeMap::new();
    let mut baseline_counts = BTreeMap::new();
    let mut current_counts = BTreeMap::new();
    let mut increased = BTreeSet::new();
    let is_ratchet = spec.mode == IncrementMode::Ratchet;
    if snapshot.delivery() {
        for issue in &mut data.issues {
            normalize_issue(issue, snapshot, workspace, false, &mut line_counts)?;
        }
        let before = data.issues.len();
        data.issues
            .retain(|issue| delivery_issue(issue, snapshot, false));
        result.metadata.insert(
            format!("{}:delivery_filtered", spec.path),
            (before - data.issues.len()).into(),
        );
    }
    if spec.mode.needs_baseline() {
        let baseline = baseline.context("Missing baseline analysis")?;
        if is_ratchet {
            require_diagnostics(&baseline)?;
            require_diagnostics(&data)?;
        }
        for mut issue in baseline.issues {
            normalize_issue(&mut issue, snapshot, workspace, true, &mut line_counts)?;
            if snapshot.delivery() {
                let selected = if is_ratchet {
                    delivery_issue(&issue, snapshot, true)
                } else if issue.locations.is_empty() {
                    snapshot.selects_diagnostic(issue.file.as_deref(), None)
                } else {
                    issue
                        .locations
                        .iter()
                        .any(|location| snapshot.selects_diagnostic(location.file.as_deref(), None))
                };
                if !selected {
                    continue;
                }
            }
            if is_ratchet {
                *baseline_counts.entry(count_key(&issue)).or_insert(0) += 1;
            } else {
                *previous.entry(fingerprint(&issue)).or_insert(0usize) += 1;
            }
        }
    }
    if is_ratchet {
        for issue in &data.issues {
            *current_counts.entry(count_key(issue)).or_insert(0) += 1;
        }
        let measurements = ratchet::compare(&baseline_counts, &current_counts);
        increased.extend(
            measurements
                .iter()
                .filter(|value| value.increased())
                .map(|value| value.key.clone()),
        );
        result.metadata.insert(
            format!("{}:ratchet", spec.path),
            serde_json::to_value(measurements)?,
        );
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
        normalize_issue(&mut issue, snapshot, workspace, false, &mut line_counts)?;
        let identity = fingerprint(&issue);
        let mut displayed = IssueLocation {
            file: issue.file.clone(),
            line: issue.line,
            end_line: None,
            symbol: issue.symbol.clone(),
        };
        if let Some(first) = issue.locations.first() {
            displayed = first.clone();
        }
        if snapshot.delivery() {
            select_location(&issue, &mut displayed, |location| {
                Ok(snapshot.selects_diagnostic(
                    location.file.as_deref(),
                    location
                        .line
                        .map(|line| Range {
                            start_line: line,
                            end_line: location.end_line.unwrap_or(line),
                        })
                        .as_ref(),
                ))
            })?;
        }
        let include = match spec.mode {
            IncrementMode::Full => true,
            IncrementMode::ChangedLines if snapshot.delivery() => true,
            IncrementMode::ChangedLines => select_location(&issue, &mut displayed, |location| {
                let file = location
                    .file
                    .as_ref()
                    .context("Cannot map a location-free diagnostic to changed lines")?;
                let line = location
                    .line
                    .context("Cannot map a diagnostic without a line to changed lines")?;
                Ok(snapshot.changes.get(file).is_some_and(|change| {
                    change
                        .added_lines
                        .range(line..=location.end_line.unwrap_or(line))
                        .next()
                        .is_some()
                }))
            })?,
            IncrementMode::AffectedScope => select_location(&issue, &mut displayed, |location| {
                let file = location
                    .file
                    .as_ref()
                    .context("Cannot map diagnostic to affected scope")?;
                Ok(affected.as_ref().is_some_and(|files| files.contains(file)))
            })?,
            IncrementMode::NewDiagnostics => match previous.get_mut(&identity) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    false
                }
                _ => true,
            },
            IncrementMode::Ratchet => increased.contains(&count_key(&issue)),
        };
        if !include {
            filtered += 1;
            continue;
        }
        // Multi-location identity already includes the complete set of paths. Its
        // selected display location must not change the repair-feedback identity.
        let identity_file = if issue.tool.is_some() || !issue.locations.is_empty() {
            None
        } else {
            displayed.file.as_deref()
        };
        let mut normalized = diagnostic(
            &result.id,
            identity_file,
            displayed.line.map(|line| Range {
                start_line: line,
                end_line: displayed.end_line.unwrap_or(line),
            }),
            issue.message.clone(),
            serde_json::to_value(&issue)?,
            "Repair the reported issue and rerun the same analyzer",
            &identity,
        );
        normalized.file = displayed.file;
        result.diagnostics.push(normalized);
    }
    result.metadata.insert(
        format!("{}:mode", spec.path),
        serde_json::to_value(effective_mode)?,
    );
    result
        .metadata
        .insert(format!("{}:filtered", spec.path), filtered.into());
    Ok(())
}

fn delivery_issue(issue: &Issue, snapshot: &Snapshot, baseline: bool) -> bool {
    let selected = |file: Option<&str>, line: Option<usize>, end: Option<usize>| {
        if baseline {
            let Some(file) = file else {
                return true;
            };
            if !snapshot.includes(file) {
                return false;
            }
            let Some(change) = snapshot.changes.get(file) else {
                return false;
            };
            line.is_none_or(|line| {
                change
                    .removed_lines
                    .range(line..=end.unwrap_or(line))
                    .next()
                    .is_some()
            })
        } else {
            snapshot.selects_diagnostic(
                file,
                line.map(|line| Range {
                    start_line: line,
                    end_line: end.unwrap_or(line),
                })
                .as_ref(),
            )
        }
    };
    if issue.locations.is_empty() {
        selected(issue.file.as_deref(), issue.line, None)
    } else {
        issue
            .locations
            .iter()
            .any(|location| selected(location.file.as_deref(), location.line, location.end_line))
    }
}

fn count_key(issue: &Issue) -> ratchet::Key {
    ratchet::Key {
        tool: issue.tool.clone(),
        rule: issue.rule.clone(),
    }
}

fn require_diagnostics(data: &Data) -> Result<()> {
    if data.tests.is_some()
        || !data.coverage.is_empty()
        || !data.coverage_files.is_empty()
        || !data.coverage_roots.is_empty()
        || data.coverage_producer.is_some()
        || data.branch_coverage
    {
        bail!("ratchet requires diagnostic counts, not tests or coverage");
    }
    Ok(())
}

#[cfg(test)]
#[path = "report_gate_tests.rs"]
mod tests;

fn fingerprint(issue: &Issue) -> String {
    if issue.tool.is_some() || !issue.locations.is_empty() {
        let mut locations: std::collections::BTreeSet<_> = issue
            .locations
            .iter()
            .map(|location| (&location.file, &location.symbol))
            .collect();
        if locations.is_empty() {
            locations.insert((&issue.file, &issue.symbol));
        }
        return snapshot::digest(
            serde_json::to_string(&(&issue.tool, &issue.rule, locations, &issue.message))
                .expect("serializable identity")
                .as_bytes(),
        );
    }
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
    line_counts: &mut BTreeMap<(bool, String), usize>,
) -> Result<()> {
    for location in &mut issue.locations {
        normalize_location(location, snapshot, workspace, base, line_counts)?;
    }
    let mut location = IssueLocation {
        file: issue.file.clone(),
        line: issue.line,
        end_line: None,
        symbol: issue.symbol.clone(),
    };
    normalize_location(&mut location, snapshot, workspace, base, line_counts)?;
    issue.file = location.file;
    Ok(())
}

fn normalize_location(
    location: &mut IssueLocation,
    snapshot: &Snapshot,
    workspace: &Path,
    base: bool,
    line_counts: &mut BTreeMap<(bool, String), usize>,
) -> Result<()> {
    if let Some(file) = &location.file {
        let mapped = map_file(file, snapshot, workspace, base)?;
        if let Some(line) = location.line {
            let key = (base, mapped.clone());
            let count = if let Some(count) = line_counts.get(&key) {
                *count
            } else {
                let files = if base {
                    &snapshot.base_files
                } else {
                    &snapshot.files
                };
                let count = std::str::from_utf8(&files[&mapped].bytes)?
                    .lines()
                    .count()
                    .max(1);
                line_counts.insert(key, count);
                count
            };
            if line == 0
                || line > count
                || location
                    .end_line
                    .is_some_and(|end| end < line || end > count)
            {
                bail!("Report location is outside the selected source: {file}");
            }
        }
        location.file = Some(if base {
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

fn select_location(
    issue: &Issue,
    displayed: &mut IssueLocation,
    matches: impl Fn(&IssueLocation) -> Result<bool>,
) -> Result<bool> {
    let fallback = [displayed.clone()];
    let locations = if issue.locations.is_empty() {
        &fallback[..]
    } else {
        &issue.locations
    };
    let mut unresolved = None;
    for location in locations {
        match matches(location) {
            Ok(true) => {
                *displayed = location.clone();
                return Ok(true);
            }
            Ok(false) => {}
            Err(error) => unresolved = Some(error),
        }
    }
    if let Some(error) = unresolved {
        return Err(error);
    }
    Ok(false)
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
    let relative = crate::paths::from_native(path)?;
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
