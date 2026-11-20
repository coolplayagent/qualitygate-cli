//! Bounded artifact checks over raw bytes from the selected Git snapshot.

use crate::adapters::diagnostic_budget::BuiltinResult;
use crate::domain::rule_parameters::ArtifactHygieneParameters;
use crate::report_evidence::report::diagnostic;
use crate::{config::RuleSetting, snapshot::Snapshot};
use anyhow::{Context, Result, ensure};
use serde_json::json;

const MAX_ADDED_FILES: usize = 10_000;
const MAX_SCANNED_BYTES: usize = 32 * 1024 * 1024;
const PREFIX_BYTES: usize = 4 * 1024;

pub(crate) fn evaluate(
    result: &mut BuiltinResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    result.check_deadline()?;
    let parameters = ArtifactHygieneParameters::from_parameters(&setting.parameters)?;
    ensure!(
        parameters.has_detector(),
        "artifact-hygiene requires at least one active detector"
    );
    let mut scanned_bytes = 0;
    for (path, change) in &snapshot.changes {
        result.check_deadline()?;
        if change.kind != "added"
            || !snapshot.includes(path)
            || parameters.allow_paths.is_match(path)
        {
            continue;
        }
        ensure!(
            result.matched_entities < MAX_ADDED_FILES,
            "Artifact hygiene exceeds the {MAX_ADDED_FILES} added file budget"
        );
        let file = snapshot
            .files
            .get(path)
            .context("Added artifact is missing from the selected snapshot")?;
        let bytes = &file.bytes;
        let extension = extension(path);

        let binary = if extension
            .as_ref()
            .is_some_and(|suffix| parameters.binary_extensions.contains(suffix))
        {
            Some((
                "binary_extension",
                "Added file has a prohibited binary extension",
            ))
        } else if parameters.scan_content {
            let prefix_len = bytes.len().min(PREFIX_BYTES);
            ensure!(
                prefix_len <= MAX_SCANNED_BYTES - scanned_bytes,
                "Artifact hygiene exceeds the {MAX_SCANNED_BYTES} byte content scan budget"
            );
            scanned_bytes += prefix_len;
            let prefix = &bytes[..prefix_len];
            binary_prefix(prefix, prefix_len == bytes.len()).then_some((
                "binary_content",
                "Added file contains NUL or invalid UTF-8 in its scanned prefix",
            ))
        } else {
            None
        };
        result.matched_entities += 1;
        if let Some((kind, message)) = binary {
            result.push(diagnostic(
                &result.id,
                Some(path),
                None,
                message.into(),
                json!({"kind": kind, "extension": extension.as_deref(), "bytes": bytes.len()}),
                "Remove the artifact from the change or use an approved path exception",
                kind,
            ))?;
        }
        if let Some(limit) = parameters.max_added_bytes
            && bytes.len() > limit
        {
            result.push(diagnostic(
                &result.id,
                Some(path),
                None,
                format!("Added file exceeds the configured limit of {limit} bytes"),
                json!({
                    "kind": "oversized_added_file",
                    "extension": extension.as_deref(),
                    "bytes": bytes.len(),
                    "limit_bytes": limit,
                }),
                "Reduce the file size or use an approved path exception",
                "oversized_added_file",
            ))?;
        }
    }
    Ok(())
}

fn extension(path: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .filter(|(_, suffix)| !suffix.is_empty())
        .map(|(_, suffix)| format!(".{}", suffix.to_ascii_lowercase()))
}

fn binary_prefix(prefix: &[u8], reaches_eof: bool) -> bool {
    prefix.contains(&0)
        || std::str::from_utf8(prefix)
            .is_err_and(|error| reaches_eof || error.error_len().is_some())
}

#[cfg(test)]
#[path = "artifact_hygiene_tests.rs"]
mod tests;
