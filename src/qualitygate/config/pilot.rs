//! Bounded, read-only loading of a pilot inventory and its full report artifacts.
use crate::domain::pilot::{ExecutionCapture, Manifest, ModelCapture, Reports};
use anyhow::{Context, Result, bail};
use std::{io::Read, path::Path};

fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    read_checked(path, max).map_err(|error| {
        let missing = error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|cause| cause.kind() == std::io::ErrorKind::NotFound);
        crate::domain::prerequisites::PrerequisiteIssue::new(
            if missing {
                crate::domain::prerequisites::FailureCode::InputMissing
            } else {
                crate::domain::prerequisites::FailureCode::InputUnreadable
            },
            crate::domain::prerequisites::Phase::Inputs,
            "Cannot read pilot input",
        )
        .resource(path.display().to_string())
        .instruction("Supply the selected pilot input and readable bounded report artifacts.")
        .wrap(error)
    })
}

fn read_checked(path: &Path, max: u64) -> Result<Vec<u8>> {
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

fn manifest_path(input: &Path) -> Result<std::path::PathBuf> {
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
    Ok(input)
}

pub fn load_manifest(input: &Path) -> Result<Manifest> {
    let input = manifest_path(input)?;
    let manifest: Manifest =
        serde_json::from_slice(&read(&input, 1024 * 1024)?).context("Invalid pilot manifest")?;
    crate::domain::pilot::validate(&manifest)?;
    verify_sources(&input, &manifest)?;
    verify_initial_reports(&input, &manifest)?;
    verify_model_evidence(&input, &manifest)?;
    verify_execution_evidence(&input, &manifest)?;
    Ok(manifest)
}

pub fn verify_model_evidence(input: &Path, manifest: &Manifest) -> Result<()> {
    if manifest.schema_version < 7 {
        return Ok(());
    }
    let input = manifest_path(input)?;
    let parent = input
        .parent()
        .context("Manifest needs a parent directory")?;
    let mut total = 0_u64;
    for observation in &manifest.observations {
        let evidence: Vec<_> = if manifest.schema_version == 7 {
            observation.model_evidence.iter().collect()
        } else {
            observation
                .attempts
                .iter()
                .filter_map(|attempt| attempt.model_evidence.as_ref())
                .collect()
        };
        for evidence in evidence {
            let artifact = &evidence.artifact;
            total = total.saturating_add(artifact.bytes + 1);
            if total > 8 * 1024 * 1024 {
                bail!("Pilot model evidence inventory exceeds 8 MiB");
            }
            let path = crate::paths::confined(parent, Path::new(&artifact.path))?;
            let bytes = read(&path, artifact.bytes).with_context(|| {
                format!(
                    "Model evidence artifact is unavailable: {}",
                    observation.assignment_id
                )
            })?;
            if bytes.len() as u64 != artifact.bytes
                || super::policy_store::digest(&bytes) != artifact.digest
            {
                bail!(
                    "Model evidence length or digest differs: {}",
                    observation.assignment_id
                );
            }
            let capture: ModelCapture =
                serde_json::from_slice(&bytes).context("Invalid model evidence JSON")?;
            if capture != evidence.capture {
                bail!("Model evidence JSON differs from the manifest claim");
            }
        }
    }
    Ok(())
}

pub fn verify_execution_evidence(input: &Path, manifest: &Manifest) -> Result<()> {
    if manifest.schema_version < 9 {
        return Ok(());
    }
    let input = manifest_path(input)?;
    let parent = input
        .parent()
        .context("Manifest needs a parent directory")?;
    let mut total = 0_u64;
    for observation in &manifest.observations {
        for evidence in observation
            .attempts
            .iter()
            .filter_map(|attempt| attempt.execution_evidence.as_ref())
        {
            let artifact = &evidence.artifact;
            total = total.saturating_add(artifact.bytes + 1);
            if total > 8 * 1024 * 1024 {
                bail!("Pilot execution evidence inventory exceeds 8 MiB");
            }
            let path = crate::paths::confined(parent, Path::new(&artifact.path))?;
            let bytes = read(&path, artifact.bytes).with_context(|| {
                format!(
                    "Execution evidence artifact is unavailable: {}",
                    observation.assignment_id
                )
            })?;
            if bytes.len() as u64 != artifact.bytes
                || super::policy_store::digest(&bytes) != artifact.digest
            {
                bail!(
                    "Execution evidence length or digest differs: {}",
                    observation.assignment_id
                );
            }
            let capture: ExecutionCapture =
                serde_json::from_slice(&bytes).context("Invalid execution evidence JSON")?;
            if capture != evidence.capture {
                bail!("Execution evidence JSON differs from the manifest claim");
            }
        }
    }
    Ok(())
}

pub fn verify_sources(input: &Path, manifest: &Manifest) -> Result<()> {
    if manifest.sources.is_empty() {
        return Ok(());
    }
    let input = manifest_path(input)?;
    let parent = input
        .parent()
        .context("Manifest needs a parent directory")?;
    for source in &manifest.sources {
        let path = crate::paths::confined(parent, Path::new(&source.path))?;
        let bytes = read(&path, source.bytes)
            .with_context(|| format!("Task source artifact is unavailable: {}", source.input_id))?;
        if bytes.len() as u64 != source.bytes
            || super::policy_store::digest(&bytes) != source.digest
        {
            bail!(
                "Task source artifact length or digest differs: {}",
                source.input_id
            );
        }
    }
    Ok(())
}

pub fn verify_initial_reports(input: &Path, manifest: &Manifest) -> Result<()> {
    if manifest.schema_version < 6 {
        return Ok(());
    }
    let input = manifest_path(input)?;
    let parent = input
        .parent()
        .context("Manifest needs a parent directory")?;
    let mut total = 0_u64;
    for assignment in &manifest.assignments {
        let artifact = assignment
            .initial_report
            .as_ref()
            .context("Pilot v6 initial report is missing")?;
        total = total.saturating_add(artifact.bytes + 1);
        if total > 64 * 1024 * 1024 {
            bail!("Pilot initial report inventory exceeds 64 MiB");
        }
        let path = crate::paths::confined(parent, Path::new(&artifact.path))?;
        let bytes = read(&path, artifact.bytes).with_context(|| {
            format!("Initial report artifact is unavailable: {}", assignment.id)
        })?;
        if bytes.len() as u64 != artifact.bytes
            || super::policy_store::digest(&bytes) != artifact.digest
        {
            bail!("Initial report length or digest differs: {}", assignment.id);
        }
    }
    Ok(())
}

pub fn load(input: &Path) -> Result<(Manifest, Reports)> {
    let input = manifest_path(input)?;
    let parent = input
        .parent()
        .context("Manifest needs a parent directory")?;
    let manifest = load_manifest(&input)?;
    let mut reports = Reports::new();
    let mut total = 0_u64;
    for artifact in manifest
        .assignments
        .iter()
        .filter_map(|a| a.initial_report.as_ref())
        .chain(
            manifest
                .observations
                .iter()
                .flat_map(|o| &o.attempts)
                .filter_map(|a| a.report.as_ref()),
        )
    {
        // Debit the worst-case read allowance, even for missing or malformed artifacts.
        total = total.saturating_add(artifact.bytes + 1);
        if total > 64 * 1024 * 1024 {
            bail!("Pilot report inventory exceeds 64 MiB");
        }
        let result = (|| {
            let path = crate::paths::confined(parent, Path::new(&artifact.path))?;
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
