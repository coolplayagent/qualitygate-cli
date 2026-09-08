//! Team rules over parsed entities, with baseline identity and language routing.

use super::{
    rules::diagnostic,
    syntax::{self, Entity, Structure},
};
use crate::{config::RuleSetting, domain::*, snapshot::Snapshot};
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

struct Change {
    path: String,
    view: Structure,
    added: Vec<Entity>,
}

pub(super) fn evaluate(
    id: &str,
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    let changes = collect(setting, snapshot)?;
    match id {
        "test-naming" => naming(result, setting, &changes),
        "parameterized-tests" => parameterized(result, setting, &changes),
        "comment-language" => comments(result, setting, &changes, snapshot),
        "ai-code-traceability" => markers(result, setting, &changes, snapshot),
        _ => bail!("Unknown structure rule: {id}"),
    }
}

fn collect(setting: &RuleSetting, snapshot: &Snapshot) -> Result<Vec<Change>> {
    let mut filters = globset::GlobSetBuilder::new();
    if let Some(paths) = setting.parameters.get("paths") {
        for path in paths.as_array().context("paths must be an array")? {
            filters.add(globset::Glob::new(
                path.as_str().context("paths entries must be strings")?,
            )?);
        }
    }
    let filters = filters.build()?;
    let mut head = BTreeMap::new();
    let mut base = BTreeMap::new();
    for (path, change) in &snapshot.changes {
        if !snapshot.includes(path) || !filters.is_empty() && !filters.is_match(path) {
            continue;
        }
        if let Some(file) = snapshot.files.get(path)
            && let Some(view) = syntax::parse(path, &file.bytes)?
        {
            head.insert(path.clone(), view);
        }
        if let Some(old_path) = &change.old_path
            && let Some(file) = snapshot.base_files.get(old_path)
            && let Some(view) = syntax::parse(old_path, &file.bytes)?
        {
            base.insert(old_path.clone(), view);
        }
    }
    let head_ids: BTreeSet<_> = head
        .iter()
        .flat_map(|(path, view)| {
            view.tests
                .iter()
                .map(|test| (path.clone(), view.language.clone(), test.symbol.clone()))
        })
        .collect();
    let base_ids: BTreeSet<_> = base
        .iter()
        .flat_map(|(path, view)| {
            view.tests
                .iter()
                .map(|test| (path.clone(), view.language.clone(), test.symbol.clone()))
        })
        .collect();
    let mut removed_bodies = BTreeMap::new();
    for (path, view) in &base {
        for test in &view.tests {
            if !head_ids.contains(&(path.clone(), view.language.clone(), test.symbol.clone())) {
                *removed_bodies
                    .entry((view.language.clone(), test.body_digest.clone()))
                    .or_insert(0usize) += 1;
            }
        }
    }
    let mut result = Vec::new();
    for (path, view) in head {
        let mut added = Vec::new();
        for test in &view.tests {
            if base_ids.contains(&(path.clone(), view.language.clone(), test.symbol.clone())) {
                continue;
            }
            if let Some(count) =
                removed_bodies.get_mut(&(view.language.clone(), test.body_digest.clone()))
                && *count > 0
            {
                *count -= 1;
                continue;
            }
            added.push(test.clone());
        }
        result.push(Change { path, view, added });
    }
    Ok(result)
}

#[cfg(test)]
#[path = "structure_rules_tests.rs"]
mod tests;

fn naming(result: &mut CheckResult, setting: &RuleSetting, changes: &[Change]) -> Result<()> {
    for change in changes {
        let pattern = setting
            .parameters
            .get("patterns")
            .and_then(|patterns| patterns.get(&change.view.language))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                setting
                    .parameters
                    .get("pattern")
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or(match change.view.language.as_str() {
                "java" => r"^should_[A-Za-z0-9_]+_when_[A-Za-z0-9_]+$",
                "python" => r"^test_[a-z0-9_]+$",
                "go" => r"^Test[A-Z_][A-Za-z0-9_]*$",
                "rust" => r"^[a-z][a-z0-9_]*$",
                _ => r"^\S.+$",
            });
        let regex = Regex::new(pattern)?;
        for test in &change.added {
            result.matched_entities += 1;
            if !regex.is_match(&test.name) {
                result.diagnostics.push(diagnostic(&result.id, Some(&change.path), Some(test.range.clone()), format!("Test '{}' does not match {pattern}", test.name), serde_json::json!({"symbol":test.symbol,"language":change.view.language,"pattern":pattern}), "Rename the test while preserving the framework's discovery convention", &test.symbol));
            }
        }
    }
    Ok(())
}

fn parameterized(
    result: &mut CheckResult,
    setting: &RuleSetting,
    changes: &[Change],
) -> Result<()> {
    let minimum = setting
        .parameters
        .get("minimum_similar")
        .map(|value| value.as_u64().context("minimum_similar must be an integer"))
        .transpose()?
        .unwrap_or(3);
    if minimum < 2 {
        bail!("minimum_similar must be at least 2");
    }
    for change in changes {
        let mut groups: BTreeMap<_, Vec<&Entity>> = BTreeMap::new();
        for test in &change.added {
            if test.annotations.keys().any(|name| {
                [
                    "ParameterizedTest",
                    "parametrize",
                    "rstest",
                    "parameterized",
                    "test_case",
                ]
                .contains(&name.as_str())
            }) {
                continue;
            }
            groups
                .entry((
                    test.symbol.split('#').next().unwrap_or(""),
                    &test.shape_digest,
                ))
                .or_default()
                .push(test);
        }
        result.matched_entities += change.added.len();
        for tests in groups
            .values()
            .filter(|tests| tests.len() as u64 >= minimum)
        {
            result.diagnostics.push(diagnostic(&result.id, Some(&change.path), Some(tests[0].range.clone()), format!("{} new tests have the same syntax structure; consider parameterization", tests.len()), serde_json::json!({"tests":tests.iter().map(|test| &test.symbol).collect::<Vec<_>>(),"heuristic":true}), "Review whether these cases can share a parameterized test", &tests[0].shape_digest));
        }
    }
    Ok(())
}

fn comments(
    result: &mut CheckResult,
    setting: &RuleSetting,
    changes: &[Change],
    snapshot: &Snapshot,
) -> Result<()> {
    let language = setting
        .parameters
        .get("language")
        .and_then(serde_json::Value::as_str)
        .context("comment-language requires parameters.language: chinese, english or bilingual")?;
    if !["chinese", "english", "bilingual"].contains(&language) {
        bail!("Unknown comment language: {language}");
    }
    let exemptions = setting
        .parameters
        .get("exempt_patterns")
        .map(|value| value.as_array().context("exempt_patterns must be an array"))
        .transpose()?
        .into_iter()
        .flatten()
        .map(|value| Regex::new(value.as_str().unwrap_or("(?!)")))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for change in changes {
        for comment in &change.view.comments {
            if !snapshot.changes[&change.path]
                .added_lines
                .iter()
                .any(|line| (comment.range.start_line..=comment.range.end_line).contains(line))
            {
                continue;
            }
            if exemptions
                .iter()
                .any(|pattern| pattern.is_match(&comment.text))
            {
                continue;
            }
            result.matched_entities += 1;
            let cjk = comment
                .text
                .chars()
                .any(|character| ('\u{3400}'..='\u{9fff}').contains(&character));
            let words = comment
                .text
                .split_whitespace()
                .filter(|word| {
                    word.chars()
                        .all(|character| character.is_ascii_alphabetic())
                })
                .count();
            let wrong = match language {
                "english" => cjk,
                "chinese" => !cjk && words >= 3,
                _ => false,
            };
            if wrong {
                result.diagnostics.push(diagnostic(
                    &result.id,
                    Some(&change.path),
                    Some(comment.range.clone()),
                    format!("New comment does not follow the {language} convention"),
                    serde_json::json!({"comment":comment.text,"heuristic":true}),
                    "Use the team's comment language or a reviewed terminology exemption",
                    &comment.text,
                ));
            }
        }
    }
    Ok(())
}

fn markers(
    result: &mut CheckResult,
    setting: &RuleSetting,
    changes: &[Change],
    snapshot: &Snapshot,
) -> Result<()> {
    let marker = setting
        .parameters
        .get("marker")
        .context("Source declarations require an explicit marker binding")?;
    let kind = marker["type"].as_str().context("marker.type missing")?;
    let name = marker["name"].as_str().context("marker.name missing")?;
    let scope = setting
        .parameters
        .get("provenance_scope")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("all_added_tests");
    if scope != "all_added_tests" {
        bail!("AI-only provenance scope requires a verified external execution record");
    }
    if !["annotation", "comment", "git_trailer"].contains(&kind) {
        bail!("Unsupported source declaration binding: {kind}");
    }
    for change in changes {
        for test in &change.added {
            result.matched_entities += 1;
            let declaration = match kind {
                "annotation" => test.annotations.get(name).cloned(),
                "comment" => change
                    .view
                    .comments
                    .iter()
                    .rev()
                    .find(|comment| {
                        comment.range.end_line <= test.range.start_line
                            && test.range.start_line - comment.range.end_line <= 3
                            && comment.text.contains(name)
                    })
                    .map(|comment| comment.text.clone()),
                "git_trailer" => {
                    if !["diff"].contains(&snapshot.identity.mode.as_str()) {
                        bail!("Commit trailers cannot prove declarations for uncommitted changes");
                    }
                    snapshot.commits.iter().find_map(|(_, message)| {
                        message
                            .lines()
                            .rev()
                            .take_while(|line| !line.trim().is_empty())
                            .find(|line| line.starts_with(&format!("{name}:")))
                            .map(Into::into)
                    })
                }
                _ => unreachable!(),
            };
            let fields = marker
                .get("fields")
                .map(|fields| fields.as_array().context("marker.fields must be an array"))
                .transpose()?;
            let mut missing = Vec::new();
            if let Some(fields) = fields {
                for field in fields {
                    let field = field
                        .as_str()
                        .context("marker field name must be a string")?;
                    let pattern = Regex::new(&format!(
                        r#"\b{}\s*[:=]\s*(?:"[^"]+"|'[^']+'|[A-Za-z0-9_.]+)"#,
                        regex::escape(field)
                    ))?;
                    if declaration
                        .as_ref()
                        .is_none_or(|text| !pattern.is_match(text))
                    {
                        missing.push(field);
                    }
                }
            }
            if declaration.is_none() || !missing.is_empty() {
                result.diagnostics.push(diagnostic(&result.id, Some(&change.path), Some(test.range.clone()), format!("Test {} lacks the required {name} source declaration or fields", test.name), serde_json::json!({"symbol":test.symbol,"marker":name,"missing_fields":missing,"declaration_only":true}), "Record the actual source using the configured binding and required fields", &test.symbol));
            }
        }
    }
    Ok(())
}
