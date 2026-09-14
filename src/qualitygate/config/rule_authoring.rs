//! Bounded, confined candidate-rule authoring; never changes policy or approvals.

use super::{CustomRule, Source, catalog::PROJECT_RULES_DIR, rule_schema};
use crate::{
    domain::{Decision, RuleFileValidation, RuleIssue, RuleValidationReport},
    paths,
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

pub(super) fn read_file(root: &Path, name: &str) -> Result<Vec<u8>> {
    let path = paths::confined(root, name.as_ref())?;
    if !std::fs::metadata(&path)
        .with_context(|| format!("Cannot read {name}"))?
        .is_file()
    {
        bail!("Rule inputs must be regular files: {name}");
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take(super::MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > super::MAX_CONFIG_BYTES {
        bail!("Rule input exceeds 1 MiB: {name}");
    }
    Ok(bytes)
}

fn files(root: &Path, name: &str, allow_empty: bool) -> Result<Vec<String>> {
    let first = paths::confined(root, name.as_ref())?;
    let single_file = std::fs::metadata(&first)?.is_file();
    let mut pending = vec![first];
    let mut files = Vec::new();
    let mut entries = 0;
    while let Some(path) = pending.pop() {
        let name = paths::from_native(path.strip_prefix(root)?)?;
        let path = paths::confined(root, name.as_ref())?;
        let metadata = std::fs::metadata(&path)?;
        if metadata.is_file() {
            if single_file
                || path
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                files.push(name);
                if files.len() > 256 {
                    bail!("Project rule package exceeds 256 files");
                }
            }
        } else if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                entries += 1;
                if entries > 4096 {
                    bail!("Project rule directory exceeds discovery budget");
                }
                pending.push(entry?.path());
            }
        } else {
            bail!("Rule inputs must be regular files or directories: {name}");
        }
    }
    files.sort();
    if files.is_empty() && !allow_empty {
        bail!("Project rule directory contains no YAML definitions");
    }
    Ok(files)
}

/// Same exact section bytes used by the immutable policy source-binding gate.
pub fn source(root: &Path, document: &str, section: &str) -> Result<Source> {
    if document.is_empty() || paths::relative(document.as_ref())? != document {
        bail!("Source document must be a normalized repository-relative path");
    }
    let bytes = read_file(root, document)?;
    source_bytes(&bytes, document, section)
}

fn source_bytes(bytes: &[u8], document: &str, section: &str) -> Result<Source> {
    let selected = crate::domain::normative::section(std::str::from_utf8(bytes)?, section)?;
    Ok(Source {
        document: document.into(),
        section: section.into(),
        content_hash: format!("sha256:{:x}", Sha256::digest(selected.as_bytes())),
    })
}

fn source_issue(root: &Path, rule: &CustomRule) -> Option<RuleIssue> {
    let bytes = match read_file(root, &rule.source.document) {
        Ok(bytes) => bytes,
        Err(error) => return Some(RuleIssue::new("input", format!("{error:#}"))),
    };
    match source_bytes(&bytes, &rule.source.document, &rule.source.section) {
        Ok(actual) if actual.content_hash != rule.source.content_hash => {
            Some(RuleIssue::new("source", "Normative section digest changed"))
        }
        Ok(_) => None,
        Err(error) => Some(RuleIssue::new("source", format!("{error:#}"))),
    }
}

pub fn validate(root: &Path, path: &str) -> Result<RuleValidationReport> {
    rule_schema::document()?;
    let mut report = RuleValidationReport {
        schema_version: 1,
        schema_id: rule_schema::SCHEMA_ID.into(),
        schema_digest: rule_schema::digest(),
        complete: true,
        decision: Decision::Pass,
        files: Vec::new(),
        issues: Vec::new(),
        review_trust: "local_candidate".into(),
    };
    let discovered = match files(root, path, false) {
        Ok(files) => files,
        Err(error) => {
            report.complete = false;
            report
                .issues
                .push(RuleIssue::new("input", format!("{error:#}")));
            Vec::new()
        }
    };
    let mut ids = BTreeSet::new();
    let mut total = 0;
    for file in discovered {
        let mut result = RuleFileValidation {
            file: file.clone(),
            rule_id: None,
            valid: false,
            issues: Vec::new(),
        };
        match read_file(root, &file) {
            Ok(bytes) => {
                total += bytes.len();
                if total > super::MAX_CONFIG_BYTES {
                    report.complete = false;
                    report.issues.push(RuleIssue::new(
                        "input",
                        "Project rule package exceeds 1 MiB",
                    ));
                    break;
                }
                let (rule, issues) = rule_schema::inspect(&bytes)?;
                result.issues = issues;
                if let Some(rule) = rule {
                    if !ids.insert(rule.id.clone()) {
                        result.issues.push(RuleIssue::new(
                            "semantic",
                            format!("Duplicate custom rule id: {}", rule.id),
                        ));
                    }
                    if let Some(issue) = source_issue(root, &rule) {
                        if issue.stage == "input" {
                            report.complete = false;
                        }
                        result.issues.push(issue);
                    }
                    result.rule_id = Some(rule.id);
                }
                result.valid = result.issues.is_empty();
            }
            Err(error) => {
                report.complete = false;
                result
                    .issues
                    .push(RuleIssue::new("input", format!("{error:#}")));
            }
        }
        report.files.push(result);
    }
    report.decision = if !report.complete {
        Decision::Incomplete
    } else if report.files.iter().any(|file| !file.valid) {
        Decision::Fail
    } else {
        Decision::Pass
    };
    Ok(report)
}

pub fn generate(root: &Path, input: &str) -> Result<serde_json::Value> {
    let rule = rule_schema::parse(&read_file(root, input)?)?;
    if let Some(issue) = source_issue(root, &rule) {
        bail!("{}", issue.message);
    }
    let data = serde_norway::to_string(&rule)?;
    rule_schema::parse(data.as_bytes())?;
    let directory = paths::confined(root, PROJECT_RULES_DIR.as_ref())?;
    if directory.try_exists()? {
        let existing = files(root, PROJECT_RULES_DIR, true)?;
        if existing.len() >= 256 {
            bail!("Project rule package would exceed 256 files");
        }
        let mut ids = BTreeSet::from([rule.id.clone()]);
        let mut total = data.len();
        for file in existing {
            let bytes = read_file(root, &file)?;
            total += bytes.len();
            if total > super::MAX_CONFIG_BYTES {
                bail!("Project rule package would exceed 1 MiB");
            }
            let existing = rule_schema::parse(&bytes)?;
            if !ids.insert(existing.id.clone()) {
                bail!("Duplicate custom rule id: {}", existing.id);
            }
        }
    }
    std::fs::create_dir_all(&directory)?;
    let name = format!("{PROJECT_RULES_DIR}/{}.yaml", rule.id);
    let target = paths::confined(root, name.as_ref())?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    temporary.write_all(data.as_bytes())?;
    temporary.as_file().sync_all()?;
    // Check the binding again immediately before publication; never approve it.
    if let Some(issue) = source_issue(root, &rule) {
        bail!("{}", issue.message);
    }
    temporary
        .persist_noclobber(target)
        .context("Cannot create candidate; existing rules are never overwritten")?;
    Ok(
        serde_json::json!({"schema_version":1,"schema_id":rule_schema::SCHEMA_ID,
        "schema_digest":rule_schema::digest(),"file":name,"rule_id":rule.id,
        "status":"candidate","policy_changed":false,"review_trust":"local_candidate"}),
    )
}
