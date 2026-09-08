//! Execute declared version probes; never accept a configured version string.

use crate::{
    config::CommandCheck,
    domain::{Artifact, CheckResult},
    paths, runner,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ToolEvidence {
    id: String,
    argv: Vec<String>,
    resolved_argv: Vec<String>,
    cwd: String,
    executable: Artifact,
    version: String,
    inputs: BTreeMap<String, String>,
    started_at_ms: u64,
    ended_at_ms: u64,
    duration_ms: u64,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: Artifact,
    stderr: Artifact,
    snapshot_digest: String,
}

pub(super) async fn collect(
    check: &CommandCheck,
    workspace: &Path,
    directory: &Path,
    snapshot: &Snapshot,
    result: &mut CheckResult,
    baseline: bool,
) -> Result<Vec<ToolEvidence>> {
    let cwd = paths::confined(workspace, Path::new(&check.cwd))?;
    let mut evidence = Vec::new();
    for (index, tool) in check.tools.iter().enumerate() {
        let mut inputs = BTreeMap::new();
        for path in &tool.inputs {
            let file = snapshot
                .files
                .get(path)
                .with_context(|| format!("Tool input missing from checked snapshot: {path}"))?;
            inputs.insert(path.clone(), snapshot::digest(&file.bytes));
        }
        let executable = runner::identity::executable(&tool.argv[0], &cwd).await?;
        let mut resolved_argv = tool.argv.clone();
        resolved_argv[0] = executable.path.clone();
        let output = runner::capture(
            &resolved_argv,
            &cwd,
            None,
            Duration::from_secs(tool.timeout_seconds),
        )
        .await?;
        let prefix = format!("{}tool-{}", if baseline { "baseline-" } else { "" }, index);
        let stdout = super::evidence::persist(
            directory,
            &check.id,
            &format!("{prefix}-stdout.log"),
            &output.stdout,
        )
        .await?;
        let stderr = super::evidence::persist(
            directory,
            &check.id,
            &format!("{prefix}-stderr.log"),
            &output.stderr,
        )
        .await?;
        let version = if output.stdout.len() + output.stderr.len() <= 65_536 {
            std::str::from_utf8(&output.stdout)
                .ok()
                .zip(std::str::from_utf8(&output.stderr).ok())
                .map(|(stdout, stderr)| format!("{stdout}{stderr}").trim().to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        evidence.push(ToolEvidence {
            id: tool.id.clone(),
            argv: tool.argv.clone(),
            resolved_argv,
            cwd: check.cwd.clone(),
            executable,
            version: version.clone(),
            inputs,
            started_at_ms: output.started_at_ms,
            ended_at_ms: output.ended_at_ms,
            duration_ms: output.duration_ms,
            exit_code: output.exit_code,
            timed_out: output.timed_out,
            stdout,
            stderr,
            snapshot_digest: snapshot.identity.content_digest.clone(),
        });
        result.metadata.insert(
            if baseline { "baseline_tools" } else { "tools" }.into(),
            serde_json::to_value(&evidence)?,
        );
        if output.timed_out
            || output.capture_error.is_some()
            || output.exit_code != Some(0)
            || version.is_empty()
            || version.contains('\0')
        {
            bail!(
                "Tool version probe {} did not produce a successful bounded version response",
                tool.id
            );
        }
        let after = runner::identity::executable(&tool.argv[0], &cwd).await?;
        if after.digest != evidence.last().unwrap().executable.digest
            || after.path != evidence.last().unwrap().executable.path
        {
            bail!(
                "Tool executable changed during version probing: {}",
                tool.id
            );
        }
    }
    Ok(evidence)
}

pub(super) async fn verify(
    check: &CommandCheck,
    workspace: &Path,
    evidence: &[ToolEvidence],
) -> Result<()> {
    let cwd = paths::confined(workspace, Path::new(&check.cwd))?;
    for tool in evidence {
        let current = runner::identity::executable(&tool.argv[0], &cwd).await?;
        if current.path != tool.executable.path || current.digest != tool.executable.digest {
            bail!(
                "Tool executable changed during command execution: {}",
                tool.id
            );
        }
    }
    Ok(())
}

pub(super) fn comparable(current: &serde_json::Value, baseline: &[ToolEvidence]) -> Result<()> {
    let current: Vec<ToolEvidence> = serde_json::from_value(current.clone())?;
    if current.len() != baseline.len()
        || current.iter().zip(baseline).any(|(a, b)| {
            a.id != b.id
                || a.version != b.version
                || a.executable.digest != b.executable.digest
                || a.inputs != b.inputs
        })
    {
        bail!("Baseline and current analyzer versions or tool inputs differ");
    }
    Ok(())
}
