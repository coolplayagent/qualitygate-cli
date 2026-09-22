//! Explicit file contracts over captured inputs, including unchanged files.

use super::{CustomRule, Result, Snapshot, Subject, diagnostic, syntax};
use crate::domain::CheckResult;
use anyhow::bail;
use serde_json::json;
use std::time::{Duration, Instant};

pub(super) fn subjects(rule: &CustomRule, snapshot: &Snapshot) -> Result<Vec<Subject>> {
    collect(rule, snapshot, Instant::now() + Duration::from_secs(30))
}

fn deadline_check(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        bail!("File rule exceeded its 30-second analysis budget");
    }
    Ok(())
}

fn collect(rule: &CustomRule, snapshot: &Snapshot, deadline: Instant) -> Result<Vec<Subject>> {
    deadline_check(deadline)?;
    let mut filters = globset::GlobSetBuilder::new();
    for path in &rule.applies_to.paths {
        filters.add(globset::Glob::new(path)?);
    }
    let filters = filters.build()?;
    let change = rule.when.change.as_deref().unwrap_or("added");
    let mut bytes = 0usize;
    let mut subjects = Vec::new();
    let files: Box<dyn Iterator<Item = (&String, &crate::snapshot::File)>> = if change == "all" {
        Box::new(snapshot.files.iter())
    } else {
        Box::new(
            snapshot
                .changes
                .iter()
                .filter(|(_, delta)| change == "any" || delta.kind == change)
                .filter_map(|(path, _)| snapshot.files.get(path).map(|file| (path, file))),
        )
    };
    let aggregate = rule.then.min_count.is_some()
        || rule.then.max_count.is_some()
        || rule.then.max_total_words.is_some();
    for (path, file) in files {
        deadline_check(deadline)?;
        if !(if aggregate {
            snapshot.feedback_includes(path)
        } else {
            snapshot.includes(path)
        }) || (!filters.is_empty() && !filters.is_match(path))
            || (!rule.language.is_empty()
                && syntax::language(path)
                    .is_none_or(|language| !rule.language.iter().any(|value| value == language)))
        {
            continue;
        }
        bytes = bytes
            .checked_add(file.bytes.len())
            .ok_or_else(|| anyhow::anyhow!("File rule byte count overflow"))?;
        if subjects.len() >= 50_000 || bytes > 32 * 1024 * 1024 {
            bail!("File rule exceeds 50,000 files or 32 MiB of selected text");
        }
        subjects.push(Subject {
            file: Some(path.clone()),
            range: None,
            identity: path.clone(),
            name: path.clone(),
            text: std::str::from_utf8(&file.bytes)?.into(),
            declaration: None,
            triggered: true,
        });
    }
    deadline_check(deadline)?;
    Ok(subjects)
}

pub(super) fn assert(
    rule: &CustomRule,
    snapshot: &Snapshot,
    subjects: &[Subject],
    result: &mut CheckResult,
) -> Result<()> {
    measure(
        rule,
        snapshot,
        subjects,
        result,
        Instant::now() + Duration::from_secs(30),
    )
}

fn measure(
    rule: &CustomRule,
    snapshot: &Snapshot,
    subjects: &[Subject],
    result: &mut CheckResult,
    deadline: Instant,
) -> Result<()> {
    deadline_check(deadline)?;
    let mut words = 0usize;
    let mut lines = 0usize;
    for subject in subjects {
        deadline_check(deadline)?;
        let count = subject.text.lines().count();
        lines += count;
        words += subject.text.split_whitespace().count();
        if let Some(maximum) = rule.then.max_lines
            && count > maximum
        {
            result.diagnostics.push(diagnostic(
                &rule.id,
                subject.file.as_deref(),
                None,
                format!("File has {count} lines; max_lines is {maximum}"),
                json!({"assertion":"max_lines", "count":count,"maximum":maximum}),
                &rule.fix,
                "max_lines",
            ));
        }
    }
    for (assertion, count, limit, fails) in [
        (
            "min_count",
            subjects.len(),
            rule.then.min_count,
            rule.then
                .min_count
                .is_some_and(|minimum| subjects.len() < minimum),
        ),
        (
            "max_total_words",
            words,
            rule.then.max_total_words,
            rule.then
                .max_total_words
                .is_some_and(|maximum| words > maximum),
        ),
    ] {
        if fails {
            result.diagnostics.push(diagnostic(
                &rule.id,
                None,
                None,
                format!(
                    "File inventory {assertion} failed: observed {count}, limit {}",
                    limit.unwrap()
                ),
                json!({"assertion":assertion,"count":count,"limit":limit}),
                &rule.fix,
                assertion,
            ));
        }
    }
    for path in &rule.then.required_paths {
        if !snapshot.files.contains_key(path) {
            result.diagnostics.push(diagnostic(
                &rule.id,
                Some(path),
                None,
                format!("Required file is missing from the selected snapshot: {path}"),
                json!({"assertion":"required_paths","path":path}),
                &rule.fix,
                "required_paths",
            ));
        }
    }
    result.metadata.insert("file_inventory".into(), json!({
        "files":subjects.len(),"lines":lines,"words":words,
        "word_unit":"Unicode whitespace-separated segments",
        "required_paths":rule.then.required_paths,"snapshot_digest":snapshot.identity.content_digest
    }));
    deadline_check(deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::Severity,
        snapshot::{File, Identity},
    };

    #[test]
    fn complete_file_inventory_refuses_expired_and_oversized_work_without_partial_success() {
        let rule: CustomRule = serde_json::from_value(json!({
            "id":"inventory","version":1,"source":{"document":"AGENTS.md","section":"Files","content_hash":format!("sha256:{}", "0".repeat(64))},
            "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},
            "then":{"min_count":1},"fix":"Restore the required files"
        })).unwrap();
        let mut snapshot = Snapshot {
            scope_evidence: Default::default(),
            root: "/repo".into(),
            identity: Identity {
                verification_digest: None,
                mode: "worktree".into(),
                base: "base".into(),
                head: "head".into(),
                content_digest: "digest".into(),
                merge_request: None,
            },
            files: Default::default(),
            base_files: Default::default(),
            changes: Default::default(),
            path_filter: None,
            commits: vec![],
        };
        let expired = Instant::now();
        assert!(collect(&rule, &snapshot, expired).is_err());
        assert!(
            measure(
                &rule,
                &snapshot,
                &[],
                &mut CheckResult::pending("inventory", true, Severity::Error),
                expired
            )
            .is_err()
        );
        for index in 0..3 {
            snapshot.files.insert(
                format!("{index}.txt"),
                File {
                    bytes: vec![b'a'; 11 * 1024 * 1024],
                    executable: false,
                },
            );
        }
        assert!(
            subjects(&rule, &snapshot)
                .err()
                .unwrap()
                .to_string()
                .contains("32 MiB")
        );
        snapshot.files.clear();
        for index in 0..50_001 {
            snapshot.files.insert(
                format!("{index}.txt"),
                File {
                    bytes: vec![],
                    executable: false,
                },
            );
        }
        assert!(
            subjects(&rule, &snapshot)
                .err()
                .unwrap()
                .to_string()
                .contains("50,000 files")
        );
    }
}
