//! Fresh report collection and independently executed baseline analysis.

use crate::{
    adapters::reports::{self, Data},
    config::{CommandCheck, IncrementMode},
    domain::{Artifact, CheckResult},
    paths, runner,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub(super) async fn prepare(check: &CommandCheck, workspace: &Path, baseline: bool) -> Result<()> {
    for report in &check.reports {
        let name = if baseline {
            report.baseline.as_deref().unwrap_or(&report.path)
        } else {
            &report.path
        };
        let path = paths::confined(workspace, Path::new(name))?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot clear stale report: {name}"));
            }
        }
    }
    Ok(())
}

pub(super) async fn collect(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Snapshot,
    result: &mut CheckResult,
) -> Result<()> {
    let mut baseline = if check
        .reports
        .iter()
        .any(|report| report.mode == IncrementMode::NewDiagnostics)
    {
        baseline(check, artifacts, snapshot, result).await?
    } else {
        BTreeMap::new()
    };
    for (index, spec) in check.reports.iter().enumerate() {
        let bytes = read_report(workspace, &spec.path).await?;
        persist(
            artifacts,
            &format!("{}-report-{index}", check.id),
            &bytes,
            result,
        )
        .await?;
        let data = reports::parse(spec.format, &bytes)?;
        super::report_gate::apply(
            result,
            spec,
            data,
            baseline.remove(&index),
            snapshot,
            workspace,
        )?;
    }
    Ok(())
}

async fn baseline(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Snapshot,
    result: &mut CheckResult,
) -> Result<BTreeMap<usize, Data>> {
    let mut base = snapshot.clone();
    base.files = snapshot.base_files.clone();
    base.identity.head = snapshot.identity.base.clone();
    base.identity.content_digest = snapshot::content_digest(&base.files);
    let workspace = snapshot::materialize(&base).await?;
    prepare(check, workspace.path(), true).await?;
    let cwd = paths::confined(workspace.path(), Path::new(&check.cwd))?;
    let output = runner::capture(
        &check.argv,
        &cwd,
        None,
        Duration::from_secs(check.timeout_seconds),
    )
    .await?;
    persist(
        artifacts,
        &format!("{}-baseline-stdout", check.id),
        &output.stdout,
        result,
    )
    .await?;
    persist(
        artifacts,
        &format!("{}-baseline-stderr", check.id),
        &output.stderr,
        result,
    )
    .await?;
    result.metadata.insert("baseline_execution".into(), serde_json::json!({"argv":check.argv,"cwd":check.cwd,"snapshot":base.identity,"exit_code":output.exit_code,"started_at_ms":output.started_at_ms,"duration_ms":output.duration_ms}));
    if output.timed_out || output.exit_code.is_none() || output.capture_error.is_some() {
        bail!("Baseline analysis did not complete");
    }
    let mut reports = BTreeMap::new();
    for (index, spec) in check
        .reports
        .iter()
        .enumerate()
        .filter(|(_, report)| report.mode == IncrementMode::NewDiagnostics)
    {
        let bytes = read_report(
            workspace.path(),
            spec.baseline.as_deref().unwrap_or(&spec.path),
        )
        .await?;
        persist(
            artifacts,
            &format!("{}-baseline-report-{index}", check.id),
            &bytes,
            result,
        )
        .await?;
        let mut data = reports::parse(spec.format, &bytes)?;
        for issue in &mut data.issues {
            if let Some(file) = &issue.file
                && let Ok(relative) = Path::new(file).strip_prefix(workspace.path())
            {
                issue.file = Some(paths::from_native(relative)?);
            }
        }
        reports.insert(index, data);
    }
    Ok(reports)
}

async fn read_report(workspace: &Path, name: &str) -> Result<Vec<u8>> {
    let path = paths::confined(workspace, Path::new(name))?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .with_context(|| format!("Required report was not produced: {name}"))?;
    if !metadata.is_file() || metadata.len() > snapshot::MAX_FILE_BYTES as u64 {
        bail!("Report is not a bounded regular file: {name}");
    }
    Ok(tokio::fs::read(path).await?)
}

async fn persist(
    directory: &Path,
    name: &str,
    bytes: &[u8],
    result: &mut CheckResult,
) -> Result<()> {
    let path = directory.join(name);
    tokio::fs::write(&path, bytes).await?;
    result.execution.artifacts.push(Artifact {
        path: path.display().to_string(),
        digest: snapshot::digest(bytes),
        bytes: bytes.len() as u64,
    });
    Ok(())
}
