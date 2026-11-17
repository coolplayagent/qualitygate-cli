//! Captured-file conversion. Offline provenance remains untrusted.

use anyhow::{Context, Result, bail, ensure};
use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

pub fn convert(format: &str, input: &Path, context: &Path) -> Result<String> {
    use crate::tool_evidence::{
        contracts::ToolFormat,
        normalization::{NormalizeContext, normalize_unbound},
    };
    let started = Instant::now();
    let format = match format {
        "crapper" => ToolFormat::Crapper,
        "dryer" => ToolFormat::Dryer,
        _ => bail!("Unsupported bridge format: {format}"),
    };
    let context_bytes = read_regular(context, MAX_INPUT_BYTES)?;
    let raw = read_regular(input, MAX_INPUT_BYTES - context_bytes.len())?;
    let mut context: NormalizeContext =
        serde_json::from_slice(&context_bytes).context("Invalid bridge normalization context")?;
    ensure!(
        context.spec.adapter == format,
        "Bridge format differs from the declared output adapter"
    );
    // The bridge can authenticate the bytes it reads, never the caller's run or snapshot.
    context.artifact_digest = crate::content_digest::sha256(&raw);
    let facts = normalize_unbound(&raw, &context)?;
    ensure!(
        started.elapsed() < Duration::from_secs(30),
        "Bridge conversion exceeded its work deadline"
    );
    Ok(serde_json::to_string(&facts)?)
}

fn read_regular(path: &Path, remaining: usize) -> Result<Vec<u8>> {
    let validate = |metadata: &std::fs::Metadata| -> Result<()> {
        ensure!(
            metadata.is_file(),
            "Bridge input must be a regular file: {}",
            path.display()
        );
        ensure!(
            metadata.len() <= remaining as u64,
            "Bridge inputs exceed the shared 32 MiB budget"
        );
        Ok(())
    };
    validate(
        &std::fs::metadata(path)
            .with_context(|| format!("Cannot inspect bridge input {}", path.display()))?,
    )?;
    let file = std::fs::File::open(path)
        .with_context(|| format!("Cannot open bridge input {}", path.display()))?;
    validate(&file.metadata()?)?;
    let mut bytes = Vec::new();
    file.take(remaining as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= remaining,
        "Bridge inputs exceed the shared 32 MiB budget"
    );
    Ok(bytes)
}

#[cfg(test)]
#[path = "conversion_tests.rs"]
mod tests;
