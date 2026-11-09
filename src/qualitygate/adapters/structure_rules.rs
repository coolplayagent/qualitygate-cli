//! Team rules over parsed entities, with baseline identity and language routing.

use super::{
    rules::diagnostic,
    syntax::{Entity, Structure},
};
use crate::{config::RuleSetting, snapshot::Snapshot};
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::BTreeMap;

struct Change {
    path: String,
    view: std::sync::Arc<Structure>,
    added: Vec<Entity>,
    removed_declarations: Vec<Entity>,
}

pub(super) fn evaluate_with_provenance(
    id: &str,
    result: &mut super::diagnostic_budget::BuiltinResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
) -> Result<()> {
    let changes = collect(setting, snapshot, facts, result.deadline())?;
    match id {
        "test-naming" | "test-naming-strict" => naming(result, setting, &changes),
        "parameterized-tests" => parameterized(result, setting, &changes),
        "comment-language" => comments(result, setting, &changes, snapshot),
        "ai-code-traceability" => markers(result, setting, &changes, snapshot, facts),
        "import-boundary" => imports(result, setting, &changes, snapshot),
        _ => bail!("Unknown structure rule: {id}"),
    }
}

fn collect(
    setting: &RuleSetting,
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
    deadline: std::time::Instant,
) -> Result<Vec<Change>> {
    let git = facts.git_trailers;
    let strings = |key: &str| -> Result<Vec<String>> {
        setting
            .parameters
            .get(key)
            .map(|value| serde_json::from_value(value.clone()).map_err(Into::into))
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let marker: Option<crate::config::Marker> = setting
        .parameters
        .get("marker")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?;
    let mut previous_views = BTreeMap::new();
    let mut result = Vec::new();
    for file in super::entity_changes::collect_before(
        snapshot,
        &strings("paths")?,
        &strings("languages")?,
        false,
        deadline,
        facts.syntax,
    )? {
        let mut removed_declarations = Vec::new();
        if let Some(marker) = &marker {
            for test in &file.tests {
                super::parallel::check_deadline(deadline)?;
                if let (Some(previous), Some(path)) = (&test.previous, &test.previous_path) {
                    if !previous_views.contains_key(path) {
                        previous_views.insert(
                            path.clone(),
                            super::syntax::parse_cached(
                                facts.syntax,
                                path,
                                &snapshot.base_files[path].bytes,
                                deadline,
                            )?
                            .context("Previous syntax is unavailable")?,
                        );
                    }
                    let previous_declaration = super::markers::bound_declaration(
                        marker,
                        previous,
                        &previous_views[path],
                        path,
                        true,
                        git,
                    )?;
                    if previous_declaration.is_some()
                        && (!snapshot.delivery()
                            || previous_declaration
                                != super::markers::bound_declaration(
                                    marker,
                                    &test.entity,
                                    &file.view,
                                    &file.path,
                                    false,
                                    git,
                                )?)
                    {
                        // Retained declarations remain obligations even when the entity
                        // has no agent participation in the current comparison.
                        removed_declarations.push(test.entity.clone());
                    }
                }
            }
        }
        result.push(Change {
            path: file.path,
            view: file.view,
            removed_declarations,
            added: file
                .tests
                .into_iter()
                .filter(|test| test.kind == "added")
                .map(|test| test.entity)
                .collect(),
        });
    }
    Ok(result)
}

#[cfg(test)]
#[path = "structure_rules_tests.rs"]
mod tests;

fn naming(
    result: &mut super::diagnostic_budget::BuiltinResult,
    setting: &RuleSetting,
    changes: &[Change],
) -> Result<()> {
    let mut compiled = BTreeMap::new();
    for change in changes {
        result.check_deadline()?;
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
        if !compiled.contains_key(pattern) {
            compiled.insert(pattern, Regex::new(pattern)?);
        }
        let regex = &compiled[pattern];
        for test in &change.added {
            result.check_deadline()?;
            result.matched_entities += 1;
            if !regex.is_match(&test.name) {
                result.push(diagnostic(&result.id, Some(&change.path), Some(test.range.clone()), format!("Test '{}' does not match {pattern}", test.name), serde_json::json!({"symbol":test.symbol,"language":change.view.language,"pattern":pattern}), "Rename the test while preserving the framework's discovery convention", &test.symbol))?;
            }
        }
    }
    Ok(())
}

/// Enforces only repository-configured import boundaries. A syntax import is a
/// bounded source fact; it is not a replacement for resolved dependency or
/// runtime data-flow evidence.
fn imports(
    result: &mut super::diagnostic_budget::BuiltinResult,
    setting: &RuleSetting,
    changes: &[Change],
    snapshot: &Snapshot,
) -> Result<()> {
    let patterns = super::rules::compiled_patterns(setting, "forbidden_imports")?;
    for change in changes {
        result.check_deadline()?;
        let mut active = std::collections::BTreeSet::new();
        let selected: Vec<_> = patterns
            .get("all")
            .into_iter()
            .chain(patterns.get(&change.view.language))
            .flat_map(|entries| entries.iter())
            .filter(|(pattern, _)| active.insert(pattern.as_str()))
            .collect();
        if selected.is_empty() {
            continue;
        }
        let added_lines = &snapshot
            .changes
            .get(&change.path)
            .context("Changed syntax view is missing snapshot change lines")?
            .added_lines;
        for import in &change.view.imports {
            result.check_deadline()?;
            if added_lines
                .range(import.range.start_line..=import.range.end_line)
                .next()
                .is_none()
            {
                continue;
            }
            result.matched_entities += 1;
            for (pattern, regex) in &selected {
                if regex.is_match(&import.text) {
                    result.push(diagnostic(
                        &result.id,
                        Some(&change.path),
                        Some(import.range.clone()),
                        format!("Changed import matches configured forbidden boundary {pattern:?}"),
                        serde_json::json!({"language": change.view.language, "pattern": pattern}),
                        "Move the dependency behind an approved boundary or update the reviewed boundary policy",
                        &format!(
                            "{}:{}-{}:{pattern}",
                            change.path, import.range.start_line, import.range.end_line
                        ),
                    ))?;
                }
            }
        }
    }
    Ok(())
}

fn parameterized(
    result: &mut super::diagnostic_budget::BuiltinResult,
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
        result.check_deadline()?;
        let mut groups: BTreeMap<_, Vec<&Entity>> = BTreeMap::new();
        for test in &change.added {
            result.check_deadline()?;
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
            result.push(diagnostic(&result.id, Some(&change.path), Some(tests[0].range.clone()), format!("{} new tests have the same syntax structure; consider parameterization", tests.len()), serde_json::json!({"tests":tests.iter().map(|test| &test.symbol).collect::<Vec<_>>(),"heuristic":true}), "Review whether these cases can share a parameterized test", &tests[0].shape_digest))?;
        }
    }
    Ok(())
}

fn comments(
    result: &mut super::diagnostic_budget::BuiltinResult,
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
        result.check_deadline()?;
        for comment in &change.view.comments {
            result.check_deadline()?;
            if snapshot.changes[&change.path]
                .added_lines
                .range(comment.range.start_line..=comment.range.end_line)
                .next()
                .is_none()
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
                result.push(diagnostic(
                    &result.id,
                    Some(&change.path),
                    Some(comment.range.clone()),
                    format!("New comment does not follow the {language} convention"),
                    serde_json::json!({"comment":comment.text,"heuristic":true}),
                    "Use the team's comment language or a reviewed terminology exemption",
                    &comment.text,
                ))?;
            }
        }
    }
    Ok(())
}

fn markers(
    result: &mut super::diagnostic_budget::BuiltinResult,
    setting: &RuleSetting,
    changes: &[Change],
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
) -> Result<()> {
    let provenance = facts.provenance;
    let marker = setting
        .parameters
        .get("marker")
        .context("Source declarations require an explicit marker binding")?;
    let marker: crate::config::Marker = serde_json::from_value(marker.clone())?;
    let scope = setting
        .parameters
        .get("provenance_scope")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("all_added_tests");
    if scope != "all_added_tests" && provenance.is_none() {
        bail!("AI-only provenance scope requires a verified external execution record");
    }
    if marker.kind == "git_trailer" && facts.git_trailers.is_none() {
        bail!("Commit-to-entity provenance is required for git_trailer binding");
    }
    let name = &marker.name;
    for change in changes {
        result.check_deadline()?;
        for (test, retained) in change
            .added
            .iter()
            .map(|test| (test, false))
            .chain(change.removed_declarations.iter().map(|test| (test, true)))
        {
            result.check_deadline()?;
            let declaration = super::markers::bound_declaration(
                &marker,
                test,
                &change.view,
                &change.path,
                false,
                facts.git_trailers,
            )?;
            if scope == "ai_only"
                && !retained
                && declaration.is_none()
                && !provenance.is_some_and(|facts| facts.participated(&change.path, test))
            {
                continue;
            }
            result.matched_entities += 1;
            let values = declaration
                .as_deref()
                .map(super::markers::fields)
                .unwrap_or_default();
            let missing: Vec<_> = marker
                .fields
                .iter()
                .filter(|field| {
                    values
                        .get(*field)
                        .is_none_or(|value| value.trim().is_empty())
                })
                .collect();
            if declaration.is_none() || !missing.is_empty() {
                result.push(diagnostic(&result.id, Some(&change.path), if snapshot.delivery() && retained { None } else { Some(test.range.clone()) }, format!("Test {} lacks the required {name} source declaration or fields", test.name), serde_json::json!({"symbol":test.symbol,"marker":name,"missing_fields":missing,"declaration_only":true}), "Record the actual source using the configured binding and required fields", &test.symbol))?;
            }
        }
    }
    Ok(())
}
