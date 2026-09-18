//! Built-in conventions over captured files and resolved Maven facts.

use super::{entity_changes, parallel, rules::diagnostic};
use crate::{
    config::RuleSetting,
    domain::{CheckResult, ProjectFacts, Range},
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSetBuilder};
use regex::Regex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
#[path = "builtin_conventions_tests.rs"]
mod tests;

fn parameter<'a>(setting: &'a RuleSetting, key: &str) -> Result<&'a str> {
    setting
        .parameters
        .get(key)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("Missing or invalid {key} parameter"))
}

fn paths(setting: &RuleSetting) -> Result<Vec<String>> {
    setting
        .parameters
        .get("paths")
        .map(|value| serde_json::from_value(value.clone()).map_err(Into::into))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn file_languages(setting: &RuleSetting) -> Result<Vec<String>> {
    let languages: Vec<String> = serde_json::from_value(
        setting
            .parameters
            .get("languages")
            .context("Missing file-pattern languages parameter")?
            .clone(),
    )?;
    for language in &languages {
        if !matches!(language.as_str(), "c" | "cpp")
            && crate::domain::language::named(language).is_none()
        {
            bail!("Unsupported file-pattern language: {language}");
        }
    }
    Ok(languages)
}

fn matches_file_language(path: &str, languages: &[String]) -> bool {
    if languages.is_empty() {
        return true;
    }
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    languages.iter().any(|language| match language.as_str() {
        "c" => matches!(extension, Some("c" | "h")),
        "cpp" => matches!(
            extension,
            Some("cc" | "cpp" | "cxx" | "h" | "hh" | "hpp" | "hxx")
        ),
        name => crate::domain::language::for_path(path).is_some_and(|found| found.name == name),
    })
}

fn file_pattern_guidance(id: &str) -> (&'static str, &'static str) {
    match id {
        "no-hardcoded-secrets" => (
            "Added Java file contains a configured literal credential pattern",
            "Move the credential out of source and review any exposed value",
        ),
        "no-printf-log" => (
            "Added C/C++ file contains a printf/fprintf call pattern",
            "Use the project's structured logging interface",
        ),
        "no-unsafe-string" => (
            "Added C/C++ file contains an unsafe string API call pattern",
            "Use a length-bounded API and verify the destination capacity",
        ),
        "no-test-sleep" => (
            "Added test file contains a sleep call pattern",
            "Control test timing with a mock, callback, or synchronization primitive",
        ),
        "no-bare-except" => (
            "Added Python file contains a bare except clause pattern",
            "Catch a specific exception or document a deliberate broad exception boundary",
        ),
        "no-os-path" => (
            "Added Python file contains a configured os.path call pattern",
            "Use pathlib when it fits this repository's path convention",
        ),
        "no-print" => (
            "Added Python file contains a print call pattern",
            "Use a logger for production diagnostics or narrow this rule's paths",
        ),
        "no-emoji" => (
            "Added file contains a character in the configured emoji ranges",
            "Remove the character or narrow this repository's style rule",
        ),
        _ => (
            "Added file contains a forbidden source pattern",
            "Remove the forbidden pattern or adjust the rule's intended scope",
        ),
    }
}

pub(super) fn annotation_dependency(
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
) -> Result<()> {
    let annotation = parameter(setting, "annotation")?;
    let group = parameter(setting, "group")?;
    let artifact = parameter(setting, "artifact")?;
    let changes = entity_changes::collect(snapshot, &paths(setting)?, &["java".into()])?;
    let has_selected_test = changes.iter().any(|file| {
        file.tests
            .iter()
            .any(|test| test.kind == "added" && test.entity.annotations.contains_key(annotation))
    });
    if !has_selected_test {
        result.metadata.insert(
            "dependency_scope".into(),
            serde_json::json!({"mode":"added_annotated_tests","annotation":annotation,"tests":0}),
        );
        return Ok(());
    }
    if projects.is_empty() {
        bail!(
            "Required project capability unavailable: dependency_resolution; declare depends_on for a Maven project facts command"
        );
    }
    if projects.iter().any(|project| {
        project.schema_version != 1
            || project.ecosystem != "maven"
            || project.snapshot_digest != snapshot.identity.content_digest
    }) {
        bail!("Maven project facts are unsupported or belong to another snapshot");
    }
    let mut matched = 0usize;
    let mut diagnostics = Vec::new();
    for file in changes {
        let selected: Vec<_> = file
            .tests
            .iter()
            .filter(|test| test.kind == "added" && test.entity.annotations.contains_key(annotation))
            .collect();
        if selected.is_empty() {
            continue;
        }
        let owners: Vec<_> = projects
            .iter()
            .filter(|project| project.owns_test(&file.path))
            .collect();
        if owners.len() != 1 {
            bail!(
                "Expected one resolved Maven test module for {}, found {}",
                file.path,
                owners.len()
            );
        }
        let project = owners[0];
        for test in selected {
            matched += 1;
            if matched > 10_000 {
                bail!("Annotation dependency scope exceeds 10000 tests");
            }
            if !project.has_test_dependency(group, artifact) {
                diagnostics.push(diagnostic(
                    &result.id,
                    Some(&file.path),
                    Some(test.entity.range.clone()),
                    format!(
                        "Annotation @{annotation} requires test dependency {group}:{artifact} in {}",
                        project.coordinate
                    ),
                    serde_json::json!({"assertion":"require_dependency","annotation":annotation,"symbol":test.entity.symbol,"project":project.coordinate,"manifest":project.manifest,"producer":project.producer_check,"group":group,"artifact":artifact}),
                    "Declare and resolve the configured annotation dependency for this test module",
                    &format!("{}:require_dependency", test.entity.symbol),
                ));
            }
        }
    }
    result.diagnostics.extend(diagnostics);
    result.matched_entities = matched;
    result.metadata.insert(
        "dependency_scope".into(),
        serde_json::json!({"mode":"added_annotated_tests","annotation":annotation,"tests":matched,"producers":projects.iter().map(|project| &project.producer_check).collect::<Vec<_>>() }),
    );
    Ok(())
}

pub(super) fn file_pattern(
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    let pattern = parameter(setting, "pattern")?;
    let regex = Regex::new(pattern)?;
    let languages = file_languages(setting)?;
    let mut filter = GlobSetBuilder::new();
    for path in paths(setting)? {
        filter.add(Glob::new(&path)?);
    }
    let filter = filter.build()?;
    let selected: Vec<_> = snapshot
        .changes
        .iter()
        .filter(|(path, change)| {
            change.kind == "added"
                && matches_file_language(path, &languages)
                && snapshot.includes(path)
                && (filter.is_empty() || filter.is_match(path))
        })
        .map(|(path, _)| {
            let file = snapshot
                .files
                .get(path)
                .with_context(|| format!("Added file is absent from snapshot: {path}"))?;
            Ok((path, file))
        })
        .collect::<Result<Vec<_>>>()?;
    let (message, fix) = file_pattern_guidance(&result.id);
    let matches = AtomicUsize::new(0);
    let batches = parallel::map(&selected, parallel::deadline(), |(path, file)| {
        let source = std::str::from_utf8(&file.bytes)
            .with_context(|| format!("Added source is not UTF-8: {path}"))?;
        let mut diagnostics = Vec::new();
        for (index, line) in source.lines().enumerate() {
            if regex.is_match(line) {
                if matches.fetch_add(1, Ordering::Relaxed) >= 10_000 {
                    bail!("File pattern diagnostics exceed 10000");
                }
                let line_number = index + 1;
                diagnostics.push(diagnostic(
                    &result.id,
                    Some(path),
                    Some(Range {
                        start_line: line_number,
                        end_line: line_number,
                    }),
                    message.into(),
                    serde_json::json!({"assertion":"forbid_pattern","pattern":pattern}),
                    fix,
                    &format!("{path}:{line_number}:forbid_pattern"),
                ));
            }
        }
        Ok(diagnostics)
    })?;
    result.matched_entities = selected.len();
    result.diagnostics.extend(batches.into_iter().flatten());
    result
        .metadata
        .insert("change".into(), serde_json::json!("added"));
    Ok(())
}
