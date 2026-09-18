//! Deterministic repository rules over immutable snapshots.

use crate::{
    config::RuleSetting,
    domain::*,
    snapshot::{Snapshot, digest},
};
use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

pub fn diagnostic(
    rule: &str,
    file: Option<&str>,
    range: Option<Range>,
    message: String,
    evidence: serde_json::Value,
    fix: &str,
    identity: &str,
) -> Diagnostic {
    let fingerprint = digest(format!("{rule}\0{}\0{identity}", file.unwrap_or("")).as_bytes());
    Diagnostic {
        id: format!("{rule}:{}", &fingerprint[7..23]),
        fingerprint,
        file: file.map(Into::into),
        range,
        message,
        evidence,
        fix: fix.into(),
        recheck: Recheck::default(),
    }
}

pub fn evaluate(id: &str, setting: &RuleSetting, snapshot: &Snapshot) -> CheckResult {
    evaluate_as(id, id, setting, snapshot)
}

pub fn evaluate_as(
    id: &str,
    implementation: &str,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> CheckResult {
    evaluate_with_projects(id, implementation, setting, snapshot, &[])
}

pub fn evaluate_with_projects(
    id: &str,
    implementation: &str,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
) -> CheckResult {
    evaluate_with_facts(id, implementation, setting, snapshot, projects, None)
}

pub fn evaluate_with_facts(
    id: &str,
    implementation: &str,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
    provenance: Option<&super::provenance::ProvenanceFacts>,
) -> CheckResult {
    evaluate_with_context(
        id,
        implementation,
        setting,
        snapshot,
        &super::facts::RuleFacts {
            projects,
            provenance,
            git_trailers: None,
        },
    )
}

pub fn evaluate_with_context(
    id: &str,
    implementation: &str,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
) -> CheckResult {
    let (projects, provenance) = (facts.projects, facts.provenance);
    let mut result = CheckResult::pending(id, setting.required, setting.severity);
    let evaluation = (|| -> Result<()> {
        if let Some(facts) = provenance {
            facts.ensure_binding(snapshot, id)?;
        }
        if setting.provenance.is_some() && provenance.is_none() {
            bail!("Configured provenance evidence is unavailable");
        }
        if let Some(git) = facts.git_trailers {
            git.ensure_binding(snapshot)?;
            result
                .metadata
                .insert("git_trailers".into(), git.evidence().clone());
        }
        match implementation {
            "line-ending" => {
                line_endings(&mut result, snapshot);
                Ok(())
            }
            "commit-message" => commits(&mut result, setting, snapshot),
            "test-annotation-dependency" => super::builtin_conventions::annotation_dependency(
                &mut result,
                setting,
                snapshot,
                projects,
            ),
            "file-pattern" => {
                super::builtin_conventions::file_pattern(&mut result, setting, snapshot)
            }
            "shell-shebang" | "shell-commented-code" => {
                super::shell_conventions::evaluate(implementation, &mut result, setting, snapshot)
            }
            "diff-size" => diff_size(&mut result, setting, snapshot),
            "module-boundary" => {
                super::project_rules::module_boundary(&mut result, setting, snapshot, projects)
            }
            "used-undeclared" => {
                super::project_rules::used_undeclared(&mut result, setting, snapshot, projects)
            }
            "source-pattern" => source_patterns(&mut result, setting, snapshot),
            "test-naming"
            | "test-naming-strict"
            | "parameterized-tests"
            | "comment-language"
            | "ai-code-traceability"
            | "import-boundary" => super::structure_rules::evaluate_with_provenance(
                implementation,
                &mut result,
                setting,
                snapshot,
                facts,
            ),
            _ => Err(anyhow::anyhow!("Unknown or unavailable rule: {id}")),
        }
    })();
    match evaluation {
        Ok(()) if result.verdict != Some(Verdict::Skipped) => result.complete(),
        Ok(()) => {}
        Err(error) => result.block(ExecutionStatus::Blocked, error.to_string()),
    }
    result
}

fn line_endings(result: &mut CheckResult, snapshot: &Snapshot) {
    for (path, change) in &snapshot.changes {
        if !snapshot.includes(path) {
            continue;
        }
        let Some(file) = snapshot.files.get(path) else {
            continue;
        };
        if file.bytes.contains(&0) {
            continue;
        }
        let expected = change
            .old_path
            .as_ref()
            .and_then(|path| snapshot.base_files.get(path))
            .map(|file| eol(&file.bytes))
            .unwrap_or("lf");
        let actual = eol(&file.bytes);
        result.matched_entities += 1;
        if actual != "none" && expected != "none" && actual != expected {
            result.diagnostics.push(diagnostic(
                &result.id,
                Some(path),
                None,
                format!("Line endings changed from {expected} to {actual}"),
                serde_json::json!({"expected": expected, "actual": actual}),
                "Restore the original line endings; use LF for a new file",
                "line-ending",
            ));
        }
    }
}

fn eol(bytes: &[u8]) -> &'static str {
    let mut crlf = false;
    let mut lf = false;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            if index > 0 && bytes[index - 1] == b'\r' {
                crlf = true;
            } else {
                lf = true;
            }
        }
    }
    match (crlf, lf) {
        (true, true) => "mixed",
        (true, false) => "crlf",
        (false, true) => "lf",
        _ => "none",
    }
}

fn commits(result: &mut CheckResult, setting: &RuleSetting, snapshot: &Snapshot) -> Result<()> {
    let pattern = setting
        .parameters
        .get("pattern")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(r"^\[[^\]]+\](feat|fix|docs|test|refactor|build|ci|chore): .+");
    let pattern = Regex::new(pattern)?;
    if snapshot.commits.is_empty() {
        result.skip("The selected comparison contains no new commits");
        return Ok(());
    }
    for (oid, message) in &snapshot.commits {
        result.matched_entities += 1;
        let subject = message.lines().next().unwrap_or_default();
        if !pattern.is_match(subject) {
            result.diagnostics.push(diagnostic(
                &result.id,
                None,
                None,
                "Commit subject does not match the configured pattern".into(),
                serde_json::json!({"commit":oid,"subject":subject,"pattern":pattern.as_str()}),
                "Update the commit subject to the team convention",
                oid,
            ));
        }
    }
    Ok(())
}

fn diff_size(result: &mut CheckResult, setting: &RuleSetting, snapshot: &Snapshot) -> Result<()> {
    let maximum = setting
        .parameters
        .get("max_added_lines")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("max_added_lines must be a positive integer"))
        })
        .transpose()?
        .unwrap_or(500);
    if maximum == 0 {
        bail!("max_added_lines must be positive");
    }
    let lines: usize = snapshot
        .changes
        .iter()
        .filter(|(path, _)| snapshot.includes(path))
        .map(|(_, change)| change.added_lines.len())
        .sum();
    result.matched_entities = lines;
    if lines as u64 > maximum {
        result.diagnostics.push(diagnostic(
            &result.id,
            None,
            None,
            format!("Change adds {lines} lines; limit is {maximum}"),
            serde_json::json!({"added_lines":lines,"limit":maximum}),
            "Split independently reviewable changes when practical",
            "added-lines",
        ));
    }
    Ok(())
}

/// Checks explicit, bounded review patterns on added source lines. This is a
/// review signal: a matching API or marker is evidence for inspection, not a
/// proof of exploitability, incomplete work, or policy noncompliance.
fn source_patterns(
    result: &mut CheckResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    let deadline = super::parallel::deadline();
    let patterns = compiled_patterns(setting, "prohibited_patterns")?;
    let paths = path_filter(setting)?;
    let languages = string_parameter(setting, "languages")?;
    for (path, change) in &snapshot.changes {
        if !snapshot.includes(path) || !paths.as_ref().is_none_or(|filter| filter.is_match(path)) {
            continue;
        }
        let Some(file) = snapshot.files.get(path) else {
            if change.kind != "deleted" {
                bail!("Changed source file is absent from snapshot: {path}");
            }
            continue;
        };
        let Some(language) =
            crate::domain::language::for_source(path, &file.bytes).map(|found| found.name)
        else {
            continue;
        };
        if !languages.is_empty() && !languages.iter().any(|value| value == language) {
            continue;
        }
        let text = std::str::from_utf8(&file.bytes)
            .map_err(|_| anyhow::anyhow!("Changed source file is not UTF-8: {path}"))?;
        let mut active = BTreeSet::new();
        let selected = patterns
            .get("all")
            .into_iter()
            .chain(patterns.get(language))
            .flat_map(|entries| entries.iter())
            .filter(|(pattern, _)| active.insert(pattern.as_str()));
        let selected: Vec<_> = selected.collect();
        if selected.is_empty() {
            continue;
        }
        for (line_number, line) in text
            .lines()
            .enumerate()
            .map(|(index, line)| (index + 1, line))
        {
            if line_number % 256 == 0 && std::time::Instant::now() >= deadline {
                bail!("Source pattern analysis exceeded 30 seconds");
            }
            if !change.added_lines.contains(&line_number) {
                continue;
            }
            result.matched_entities += 1;
            if result.matched_entities > 1_000_000 {
                bail!("Source pattern scope exceeds 1000000 added lines");
            }
            for (pattern, regex) in &selected {
                if regex.is_match(line) {
                    if result.diagnostics.len() >= 10_000 {
                        bail!("Source pattern diagnostics exceed 10000");
                    }
                    result.diagnostics.push(diagnostic(
                        &result.id,
                        Some(path),
                        Some(Range {
                            start_line: line_number,
                            end_line: line_number,
                        }),
                        format!("Changed source line matches configured review pattern {pattern:?}"),
                        serde_json::json!({"language": language, "pattern": pattern}),
                        "Review the changed API or marker against the repository's security and quality policy",
                        &format!("{path}:{line_number}:{pattern}"),
                    ));
                }
            }
        }
    }
    if std::time::Instant::now() >= deadline {
        bail!("Source pattern analysis exceeded 30 seconds");
    }
    Ok(())
}

pub(super) fn compiled_patterns(
    setting: &RuleSetting,
    key: &str,
) -> Result<BTreeMap<String, Vec<(String, Regex)>>> {
    let values: BTreeMap<String, Vec<String>> = setting
        .parameters
        .get(key)
        .context(format!("{key} requires language-keyed regex arrays"))
        .and_then(|value| serde_json::from_value(value.clone()).map_err(Into::into))?;
    if values.is_empty() || values.len() > 32 {
        bail!("{key} requires 1..32 language entries");
    }
    let mut result = BTreeMap::new();
    for (language, entries) in values {
        if language != "all"
            && !["java", "python", "typescript", "go", "rust", "shell"].contains(&language.as_str())
        {
            bail!("{key} has unsupported language: {language}");
        }
        if entries.is_empty() || entries.len() > 32 {
            bail!("{key}.{language} requires 1..32 regex patterns");
        }
        let mut unique = BTreeSet::new();
        let mut compiled = Vec::new();
        for pattern in entries {
            if pattern.is_empty() || pattern.len() > 512 || !unique.insert(pattern.clone()) {
                bail!("{key}.{language} requires distinct regex patterns of 1..512 bytes");
            }
            compiled.push((pattern.clone(), Regex::new(&pattern)?));
        }
        result.insert(language, compiled);
    }
    Ok(result)
}

fn string_parameter(setting: &RuleSetting, key: &str) -> Result<Vec<String>> {
    setting
        .parameters
        .get(key)
        .map(|value| serde_json::from_value(value.clone()).map_err(Into::into))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn path_filter(setting: &RuleSetting) -> Result<Option<GlobSet>> {
    let paths = string_parameter(setting, "paths")?;
    if paths.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for path in paths {
        builder.add(Glob::new(&path)?);
    }
    Ok(Some(builder.build()?))
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
