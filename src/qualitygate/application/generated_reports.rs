//! Fresh report collection and independently executed baseline analysis.

use crate::{
    adapters::reports::{self, Data},
    config::{CommandCheck, IncrementMode},
    domain::CheckResult,
    paths, runner,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

pub(super) async fn prepare(check: &CommandCheck, workspace: &Path, baseline: bool) -> Result<()> {
    let reports = check.reports.iter().map(|report| {
        if baseline {
            report.baseline.as_deref().unwrap_or(&report.path)
        } else {
            &report.path
        }
    });
    for project in &check.projects {
        if let crate::config::ProjectSpec::Python(project) = project {
            let target = paths::confined(workspace, Path::new(&project.install_target))?;
            match tokio::fs::symlink_metadata(target).await {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    bail!("Python installation target must be fresh and absent before the command")
                }
            }
            let report = paths::confined(workspace, Path::new(&project.install_report))?;
            tokio::fs::create_dir_all(
                report
                    .parent()
                    .context("Python report has no parent directory")?,
            )
            .await?;
        }
    }
    let projects = check.projects.iter().flat_map(|project| project.outputs());
    for name in reports.chain(projects) {
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
    snapshot: &Arc<Snapshot>,
    result: &mut CheckResult,
) -> Result<bool> {
    let mut baseline = if check
        .reports
        .iter()
        .any(|report| report.mode == IncrementMode::NewDiagnostics)
    {
        baseline(check, artifacts, snapshot, result).await?
    } else {
        BTreeMap::new()
    };
    let mut findings = false;
    for (index, spec) in check.reports.iter().enumerate() {
        let bytes = read_report(workspace, &spec.path).await?;
        persist(artifacts, &format!("report-{index}"), &bytes, result).await?;
        let previous = baseline.remove(&index);
        let spec = spec.clone();
        let mut next = result.clone();
        let snapshot = Arc::clone(snapshot);
        let workspace = workspace.to_owned();
        let (next, found) = tokio::task::spawn_blocking(move || {
            let data = reports::parse(spec.format, &bytes)?;
            let found = data.has_findings();
            let before = next.diagnostics.len();
            super::report_gate::apply(&mut next, &spec, data, previous, &snapshot, &workspace)?;
            let found = found || next.diagnostics.len() > before;
            Ok::<_, anyhow::Error>((next, found))
        })
        .await??;
        *result = next;
        findings |= found;
    }
    Ok(findings)
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
    base.identity.mode = "baseline".into();
    base.identity.merge_request = None;
    base.identity.content_digest = snapshot::content_digest(&base.files);
    let workspace = snapshot::materialize(&base).await?;
    let inputs = snapshot::InputGuard::new(workspace.path(), base.files.clone()).await?;
    result
        .metadata
        .insert("baseline_snapshot".into(), serde_json::json!(base.identity));
    let tools =
        super::tool_evidence::collect(check, workspace.path(), artifacts, &base, result, true)
            .await?;
    super::tool_evidence::comparable(
        result
            .metadata
            .get("tools")
            .context("Current analyzer version evidence is missing")?,
        &tools,
    )?;
    prepare(check, workspace.path(), true).await?;
    inputs
        .verify()
        .await
        .context("Baseline inputs changed during report preparation")?;
    let cwd = paths::confined(workspace.path(), Path::new(&check.cwd))?;
    let executable = runner::identity::executable(&check.argv[0], &cwd).await?;
    if result
        .metadata
        .get("command_executable")
        .and_then(|value| value.get("digest"))
        .and_then(serde_json::Value::as_str)
        != Some(executable.digest.as_str())
    {
        bail!("Baseline and current command executables differ");
    }
    let mut resolved_argv = check.argv.clone();
    resolved_argv[0] = executable.path.clone();
    let output = runner::capture(
        &resolved_argv,
        &cwd,
        None,
        Duration::from_secs(check.timeout_seconds),
    )
    .await?;
    persist(artifacts, "baseline-stdout", &output.stdout, result).await?;
    persist(artifacts, "baseline-stderr", &output.stderr, result).await?;
    result.metadata.insert("baseline_execution".into(), serde_json::json!({"argv":check.argv,"resolved_argv":resolved_argv,"executable":executable,"cwd":check.cwd,"snapshot":base.identity,"exit_code":output.exit_code,"started_at_ms":output.started_at_ms,"ended_at_ms":output.ended_at_ms,"duration_ms":output.duration_ms}));
    let after = runner::identity::executable(&check.argv[0], &cwd).await?;
    if after.path != executable.path || after.digest != executable.digest {
        bail!("Baseline command executable changed during execution");
    }
    super::tool_evidence::verify(check, workspace.path(), &tools).await?;
    if let Err(error) = inputs.verify().await {
        result.metadata.insert("baseline_input_integrity".into(), serde_json::json!({"snapshot_digest":inputs.digest,"status":"invalid","reason":format!("{error:#}")}));
        bail!("Baseline command changed its checked inputs: {error:#}");
    }
    result.metadata.insert(
        "baseline_input_integrity".into(),
        serde_json::json!({"snapshot_digest":inputs.digest,"status":"verified"}),
    );
    if output.timed_out || output.exit_code.is_none() || output.capture_error.is_some() {
        bail!("Baseline analysis did not complete");
    }
    if output.exit_code != Some(check.expected_exit_code)
        && !output
            .exit_code
            .is_some_and(|code| check.findings_exit_codes.contains(&code))
    {
        bail!(
            "Baseline analyzer returned unexpected exit code {:?}",
            output.exit_code
        );
    }
    let mut reports = BTreeMap::new();
    let mut findings = false;
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
            &format!("baseline-report-{index}"),
            &bytes,
            result,
        )
        .await?;
        let format = spec.format;
        let mut data =
            tokio::task::spawn_blocking(move || reports::parse(format, &bytes)).await??;
        findings |= data.has_findings();
        for issue in &mut data.issues {
            if let Some(file) = &issue.file
                && let Ok(relative) = Path::new(file).strip_prefix(workspace.path())
            {
                issue.file = Some(paths::from_native(relative)?);
            }
            for location in &mut issue.locations {
                if let Some(file) = &location.file
                    && let Ok(relative) = Path::new(file).strip_prefix(workspace.path())
                {
                    location.file = Some(paths::from_native(relative)?);
                }
            }
        }
        reports.insert(index, data);
    }
    if output
        .exit_code
        .is_some_and(|code| check.findings_exit_codes.contains(&code))
        && !findings
    {
        bail!("Baseline findings exit code is contradicted by reports without findings");
    }
    Ok(reports)
}

pub(super) async fn read_report(workspace: &Path, name: &str) -> Result<Vec<u8>> {
    let path = paths::confined(workspace, Path::new(name))?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .with_context(|| format!("Required report was not produced: {name}"))?;
    if !metadata.is_file() || metadata.len() > snapshot::MAX_FILE_BYTES as u64 {
        bail!("Report is not a bounded regular file: {name}");
    }
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    tokio::fs::File::open(path)
        .await?
        .take(snapshot::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > snapshot::MAX_FILE_BYTES {
        bail!("Report grew beyond its size budget: {name}");
    }
    Ok(bytes)
}

async fn persist(
    directory: &Path,
    name: &str,
    bytes: &[u8],
    result: &mut CheckResult,
) -> Result<()> {
    let artifact = super::evidence::persist(directory, &result.id, name, bytes).await?;
    result.execution.artifacts.push(artifact);
    Ok(())
}
