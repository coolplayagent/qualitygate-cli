//! Bounded, read-only loading of a pilot inventory and its full report artifacts.
use crate::domain::pilot::{Manifest, Reports};
use anyhow::{Context, Result, bail};
use std::{io::Read, path::Path};

fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        bail!("Pilot input must be a regular non-symlink file");
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Pilot input changed file type");
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        bail!("Pilot input exceeds its byte budget");
    }
    Ok(bytes)
}

pub fn load(input: &Path) -> Result<(Manifest, Reports)> {
    // The operator may select an external archive. Artifact paths themselves are
    // relative to that archive and cannot traverse or follow symlink ancestors.
    let parent = dunce::canonicalize(
        input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let input = crate::paths::confined(
        &parent,
        Path::new(input.file_name().context("Manifest needs a filename")?),
    )?;
    let manifest: Manifest =
        serde_json::from_slice(&read(&input, 1024 * 1024)?).context("Invalid pilot manifest")?;
    crate::domain::pilot::validate(&manifest)?;
    let mut reports = Reports::new();
    let mut total = 0_u64;
    for artifact in manifest
        .observations
        .iter()
        .flat_map(|o| &o.attempts)
        .filter_map(|a| a.report.as_ref())
    {
        // Debit the worst-case read allowance, even for missing or malformed artifacts.
        total = total.saturating_add(artifact.bytes + 1);
        if total > 64 * 1024 * 1024 {
            bail!("Pilot report inventory exceeds 64 MiB");
        }
        let result = (|| {
            let path = crate::paths::confined(&parent, Path::new(&artifact.path))?;
            let bytes = read(&path, artifact.bytes)?;
            if bytes.len() as u64 != artifact.bytes
                || super::policy_store::digest(&bytes) != artifact.digest
            {
                bail!("Full report length or digest differs from the inventory");
            }
            serde_json::from_slice(&bytes).context("Invalid full report")
        })();
        reports.insert(
            artifact.digest.clone(),
            result.map_err(|e: anyhow::Error| e.to_string()),
        );
    }
    Ok((manifest, reports))
}
