//! Bounded checks for Shell properties that a single-line regex cannot express.

use super::{parallel, rules::diagnostic};
use crate::{
    config::RuleSetting,
    domain::{CheckResult, Range},
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSetBuilder};
use regex::Regex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) fn evaluate(
    implementation: &str,
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    let paths: Vec<String> = setting
        .parameters
        .get("paths")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?
        .unwrap_or_default();
    let mut globs = GlobSetBuilder::new();
    for path in paths {
        globs.add(Glob::new(&path)?);
    }
    let globs = globs.build()?;
    let mut selected = Vec::new();
    for (path, change) in &snapshot.changes {
        if change.kind == "deleted"
            || !snapshot.includes(path)
            || (!globs.is_empty() && !globs.is_match(path))
        {
            continue;
        }
        let file = snapshot
            .files
            .get(path)
            .with_context(|| format!("Changed Shell file is absent from snapshot: {path}"))?;
        if crate::domain::language::for_source(path, &file.bytes)
            .is_some_and(|language| language.name == "shell")
        {
            selected.push((path, change, file));
        }
    }
    let dead_code = Regex::new(
        r"^\s*#\s*(?:(?:if|for|while|case|function|return|exit|local|export|readonly)\b|[A-Za-z_][A-Za-z0-9_]*\s*=)",
    )?;
    let total = AtomicUsize::new(0);
    let batches = parallel::map(&selected, parallel::deadline(), |(path, change, file)| {
        let source = std::str::from_utf8(&file.bytes)
            .with_context(|| format!("Changed Shell file is not UTF-8: {path}"))?;
        let mut diagnostics = Vec::new();
        if implementation == "shell-shebang" {
            if !source.starts_with("#!")
                && (change.kind == "added" || change.added_lines.contains(&1))
            {
                diagnostics.push(diagnostic(
                    &result.id,
                    Some(path),
                    Some(Range {
                        start_line: 1,
                        end_line: 1,
                    }),
                    "Added Shell script has no first-line interpreter directive".into(),
                    serde_json::json!({"assertion":"require_shell_shebang"}),
                    "Add an appropriate #! interpreter directive on the first line",
                    &format!("{path}:shebang"),
                ));
            }
        } else {
            let lines: Vec<_> = source.lines().collect();
            let mut start = None;
            for index in 0..=lines.len() {
                if index < lines.len() && dead_code.is_match(lines[index]) {
                    start.get_or_insert(index);
                } else if let Some(first) = start.take()
                    && index - first >= 3
                    && (first + 1..=index).any(|line| change.added_lines.contains(&line))
                {
                    diagnostics.push(diagnostic(
                        &result.id,
                        Some(path),
                        Some(Range {
                            start_line: first + 1,
                            end_line: index,
                        }),
                        "Shell file contains three or more consecutive code-like comments".into(),
                        serde_json::json!({"assertion":"forbid_commented_code_block","lines":index-first}),
                        "Remove dead code or rewrite the block as explanatory prose",
                        &format!("{path}:{}:commented-code", first + 1),
                    ));
                }
            }
        }
        if total.fetch_add(diagnostics.len(), Ordering::Relaxed) + diagnostics.len() > 10_000 {
            bail!("Shell convention diagnostics exceed 10000");
        }
        Ok(diagnostics)
    })?;
    result.matched_entities = selected.len();
    result.diagnostics.extend(batches.into_iter().flatten());
    result
        .metadata
        .insert("change".into(), serde_json::json!("added_lines"));
    Ok(())
}
