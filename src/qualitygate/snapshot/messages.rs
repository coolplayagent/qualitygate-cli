//! One bounded Git stream for the selected commit messages, preserving pretty-format semantics.
use anyhow::{Context, Result, ensure};
use std::path::Path;

const MAX_COMMITS: usize = 1000;
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

pub(super) async fn capture(root: &Path, base: &str, head: &str) -> Result<Vec<(String, String)>> {
    let range = format!("{base}..{head}");
    // %B uses the same encoding conversion and embedded-NUL behavior as the
    // former per-commit `git show -s --format=%B`. Git terminates each record
    // with NUL; its formatter never emits a message's embedded NUL as a field.
    let output = super::run_git(
        root,
        &[
            "log",
            "-z",
            "--encoding=UTF-8",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-color",
            "--max-count=1001",
            "--format=%H%x00%B",
            &range,
            "--",
        ],
        None,
    )
    .await?;
    tokio::task::spawn_blocking(move || parse(&output, MAX_MESSAGE_BYTES)).await?
}

fn parse(output: &[u8], budget: usize) -> Result<Vec<(String, String)>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    ensure!(output.last() == Some(&0), "Truncated Git message stream");
    let mut fields = output[..output.len() - 1].split(|byte| *byte == 0);
    let mut messages = Vec::new();
    let mut bytes = 0usize;
    while let Some(oid) = fields.next() {
        ensure!(
            messages.len() < MAX_COMMITS,
            "Commit range exceeds 1000 commits"
        );
        ensure!(
            [40, 64].contains(&oid.len()) && oid.iter().all(u8::is_ascii_hexdigit),
            "Invalid Git message identity"
        );
        let message = fields.next().context("Missing Git commit message")?;
        bytes = bytes.saturating_add(message.len()).saturating_add(1);
        ensure!(
            bytes <= budget,
            "Commit messages exceed the 8 MiB aggregate byte budget"
        );
        // `show --format=%B` appends a newline after the message, including when
        // the commit already has trailing newlines. Retain the existing contract.
        let mut message = std::str::from_utf8(message)?.to_owned();
        message.push('\n');
        messages.push((std::str::from_utf8(oid)?.into(), message));
    }
    Ok(messages)
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
