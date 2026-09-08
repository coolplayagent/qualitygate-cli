//! Finite DSL version 1 over snapshot, Git and language facts.

use super::{entity_changes, markers, rules::diagnostic, syntax};
use crate::{
    config::{CustomRule, RuleSetting},
    domain::*,
    snapshot::Snapshot,
};
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::BTreeMap;

struct Subject {
    file: Option<String>,
    range: Option<Range>,
    identity: String,
    name: String,
    text: String,
    declaration: Option<String>,
}

pub fn evaluate(rule: &CustomRule, setting: &RuleSetting, snapshot: &Snapshot) -> CheckResult {
    let mut result = CheckResult::pending(&rule.id, setting.required, setting.severity);
    result.rule_version = rule.version;
    match run(rule, snapshot, &mut result) {
        Ok(()) => result.complete(),
        Err(error) => result.block(ExecutionStatus::Blocked, format!("{error:#}")),
    }
    result
}

fn run(rule: &CustomRule, snapshot: &Snapshot, result: &mut CheckResult) -> Result<()> {
    for language in &rule.language {
        if !["java", "python", "typescript", "go", "rust", "shell"].contains(&language.as_str()) {
            bail!("Syntax capability unavailable for language: {language}");
        }
    }
    if let Some(capability) = rule.requires_capabilities.iter().find(|capability| {
        ["dependency_resolution", "external_provenance"].contains(&capability.as_str())
    }) {
        bail!("Required project capability unavailable: {capability}");
    }
    if rule.applies_to.provenance_scope.as_deref() == Some("ai_only") {
        bail!("AI-only scope requires verified external execution provenance");
    }
    let subjects = subjects(rule, snapshot)?;
    result.matched_entities = subjects.len();
    result
        .metadata
        .insert("increment_mode".into(), serde_json::json!("entity_changes"));
    result.metadata.insert(
        "change".into(),
        serde_json::json!(rule.when.change.as_deref().unwrap_or("added")),
    );
    result.metadata.insert(
        "requires_capabilities".into(),
        serde_json::json!(rule.requires_capabilities),
    );
    let name = rule
        .then
        .name_pattern
        .as_deref()
        .map(Regex::new)
        .transpose()?;
    let forbid = rule
        .then
        .forbid_pattern
        .as_deref()
        .map(Regex::new)
        .transpose()?;
    for subject in &subjects {
        let mut violations = Vec::new();
        if name
            .as_ref()
            .is_some_and(|pattern| !pattern.is_match(&subject.name))
        {
            violations.push((
                "name_pattern",
                format!(
                    "{} does not match the configured name pattern",
                    subject.name
                ),
            ));
        }
        if forbid
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(&subject.text))
        {
            violations.push((
                "forbid_pattern",
                "Entity contains text forbidden by this rule".into(),
            ));
        }
        if rule.then.require_marker {
            let marker = &rule
                .binding
                .as_ref()
                .expect("validated marker binding")
                .marker;
            let values = subject
                .declaration
                .as_deref()
                .map(markers::fields)
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
            if subject.declaration.is_none() || !missing.is_empty() {
                violations.push((
                    "require_marker",
                    format!(
                        "Missing {} declaration or nonempty fields: {}",
                        marker.name,
                        missing.into_iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
        }
        for (kind, message) in violations {
            result.diagnostics.push(diagnostic(&rule.id, subject.file.as_deref(), subject.range.clone(), message,
                serde_json::json!({"symbol":subject.identity,"assertion":kind,"entity":rule.when.entity,"declaration_only":rule.then.require_marker}),
                &rule.fix, &format!("{}:{kind}", subject.identity)));
        }
    }
    if let Some(maximum) = rule.then.max_count
        && subjects.len() > maximum
    {
        result.diagnostics.push(diagnostic(
            &rule.id,
            None,
            None,
            format!(
                "{} matching entities exceeds max_count {maximum}",
                subjects.len()
            ),
            serde_json::json!({"count":subjects.len(),"maximum":maximum,"entity":rule.when.entity}),
            &rule.fix,
            "max_count",
        ));
    }
    Ok(())
}

fn subjects(rule: &CustomRule, snapshot: &Snapshot) -> Result<Vec<Subject>> {
    let change = rule.when.change.as_deref().unwrap_or("added");
    if rule.when.entity == "commit" {
        return Ok(snapshot
            .commits
            .iter()
            .map(|(oid, message)| Subject {
                file: None,
                range: None,
                identity: oid.clone(),
                name: message.lines().next().unwrap_or_default().into(),
                text: message.clone(),
                declaration: None,
            })
            .collect());
    }
    if rule.when.entity == "file" {
        let mut filters = globset::GlobSetBuilder::new();
        for path in &rule.applies_to.paths {
            filters.add(globset::Glob::new(path)?);
        }
        let filters = filters.build()?;
        return snapshot
            .changes
            .iter()
            .filter(|(path, delta)| {
                snapshot.files.contains_key(*path)
                    && snapshot.includes(path)
                    && (change == "any" || delta.kind == change)
                    && (filters.is_empty() || filters.is_match(path))
                    && (rule.language.is_empty()
                        || syntax::language(path).is_some_and(|language| {
                            rule.language.iter().any(|value| value == language)
                        }))
            })
            .map(|(path, _)| {
                Ok(Subject {
                    file: Some(path.clone()),
                    range: None,
                    identity: path.clone(),
                    name: path.clone(),
                    text: std::str::from_utf8(&snapshot.files[path].bytes)?.into(),
                    declaration: None,
                })
            })
            .collect();
    }
    let files = entity_changes::collect(snapshot, &rule.applies_to.paths, &rule.language)?;
    let mut subjects = Vec::new();
    for file in files {
        if rule
            .requires_capabilities
            .iter()
            .any(|value| value == "annotations")
            && !["java", "python", "rust"].contains(&file.view.language.as_str())
        {
            bail!(
                "annotations capability unavailable for {} in {}",
                file.view.language,
                file.path
            );
        }
        if rule.when.entity == "test_method" && file.view.language == "shell" {
            bail!(
                "test_methods capability unavailable for shell in {}",
                file.path
            );
        }
        match rule.when.entity.as_str() {
            "test_method" => {
                let bytes = &snapshot.files[&file.path].bytes;
                for test in file
                    .tests
                    .iter()
                    .filter(|test| change == "any" || test.kind == change)
                {
                    let entity = &test.entity;
                    subjects.push(Subject {
                        file: Some(file.path.clone()),
                        range: Some(entity.range.clone()),
                        identity: entity.symbol.clone(),
                        name: entity.name.clone(),
                        text: std::str::from_utf8(&bytes[entity.byte_range.clone()])?.into(),
                        declaration: rule
                            .binding
                            .as_ref()
                            .map(|binding| {
                                markers::declaration(
                                    &binding.marker,
                                    entity,
                                    &file.view,
                                    &file.path,
                                    snapshot,
                                )
                            })
                            .transpose()?
                            .flatten(),
                    });
                }
            }
            "comment" | "import" => {
                let entities = |view: &syntax::Structure| -> Vec<(String, Range)> {
                    if rule.when.entity == "comment" {
                        view.comments
                            .iter()
                            .map(|value| (value.text.clone(), value.range.clone()))
                            .collect()
                    } else {
                        view.imports
                            .iter()
                            .map(|value| (value.text.clone(), value.range.clone()))
                            .collect()
                    }
                };
                let mut counts = BTreeMap::new();
                if let Some(previous) = &file.previous {
                    for (text, _) in entities(previous) {
                        *counts.entry(text).or_insert(0usize) += 1;
                    }
                }
                for (text, range) in entities(&file.view) {
                    if change != "any"
                        && let Some(count) = counts.get_mut(&text)
                        && *count > 0
                    {
                        *count -= 1;
                        continue;
                    }
                    subjects.push(Subject {
                        file: Some(file.path.clone()),
                        range: Some(range),
                        identity: text.clone(),
                        name: text.clone(),
                        text,
                        declaration: None,
                    });
                }
            }
            other => bail!("Unsupported DSL entity: {other}"),
        }
    }
    Ok(subjects)
}
