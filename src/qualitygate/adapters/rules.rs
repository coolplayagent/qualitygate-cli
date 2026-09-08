//! Deterministic repository rules over immutable snapshots.

use crate::{
    config::RuleSetting,
    domain::*,
    snapshot::{Snapshot, digest},
};
use anyhow::{Result, bail};
use regex::Regex;

pub const BUILTINS: &[(&str, &str)] = &[
    (
        "test-naming",
        "Validate added tests using language-specific naming conventions",
    ),
    (
        "parameterized-tests",
        "Suggest parameterization for structurally similar new tests",
    ),
    (
        "comment-language",
        "Check changed comments using the configured language and exemptions",
    ),
    (
        "ai-code-traceability",
        "Validate explicitly configured source declarations for new tests",
    ),
    (
        "line-ending",
        "Preserve line endings; new text files use LF",
    ),
    (
        "commit-message",
        "Validate new commit subjects against a team pattern",
    ),
    ("diff-size", "Bound added lines in the selected change"),
];

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
    let mut result = CheckResult::pending(id, setting.required, setting.severity);
    let evaluation = match id {
        "line-ending" => {
            line_endings(&mut result, snapshot);
            Ok(())
        }
        "commit-message" => commits(&mut result, setting, snapshot),
        "diff-size" => diff_size(&mut result, setting, snapshot),
        "test-naming" | "parameterized-tests" | "comment-language" | "ai-code-traceability" => {
            super::structure_rules::evaluate(id, &mut result, setting, snapshot)
        }
        _ => Err(anyhow::anyhow!("Unknown or unavailable rule: {id}")),
    };
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
