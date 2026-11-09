//! Bounded checks for Shell properties that a single-line regex cannot express.

use super::{parallel, rules::diagnostic};
use crate::{config::RuleSetting, domain::Range, snapshot::Snapshot};
use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use regex::Regex;

pub(super) fn evaluate(
    implementation: &str,
    result: &mut super::diagnostic_budget::BuiltinResult,
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
    let batches = parallel::map(&selected, result.deadline(), |(path, change, file)| {
        let source = std::str::from_utf8(&file.bytes)
            .with_context(|| format!("Changed Shell file is not UTF-8: {path}"))?;
        let mut diagnostics = Vec::new();
        if implementation == "shell-shebang" {
            if !source.starts_with("#!")
                && (change.kind == "added" || change.added_lines.contains(&1))
            {
                result.push_buffered(
                    &mut diagnostics,
                    diagnostic(
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
                    ),
                )?;
            }
        } else {
            let lines: Vec<_> = source.lines().collect();
            let mut start = None;
            for index in 0..=lines.len() {
                if index % 256 == 0 {
                    result.check_deadline()?;
                }
                if index < lines.len() && dead_code.is_match(lines[index]) {
                    start.get_or_insert(index);
                } else if let Some(first) = start.take()
                    && index - first >= 3
                    && (first + 1..=index).any(|line| change.added_lines.contains(&line))
                {
                    result.push_buffered(&mut diagnostics, diagnostic(
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
                    ))?;
                }
            }
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
