//! Bind normalized observations to the actual snapshot, probe and retained bytes.
use super::tool_evidence::ToolEvidence;
use crate::{
    config::CommandCheck,
    domain::{Artifact, CheckResult},
    snapshot::Snapshot,
    tool_evidence::{
        contracts::{CoverageEvidence, ToolFacts, ToolFormat},
        normalization::{ExecutionReceipt, NormalizeContext},
    },
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

#[cfg(test)]
#[path = "fact_execution_tests.rs"]
mod tests;

#[derive(Clone)]
pub(crate) struct Receipt {
    pub command: Artifact,
    pub tools: Vec<ToolEvidence>,
    pub artifacts: Vec<Artifact>,
}

impl Receipt {
    pub(crate) fn new(command: Artifact, tools: Vec<ToolEvidence>, result: &CheckResult) -> Self {
        let mut artifacts = result.execution.artifacts.clone();
        artifacts.extend(
            tools
                .iter()
                .flat_map(|tool| tool.artifacts().map(Clone::clone)),
        );
        Self {
            command,
            tools,
            artifacts,
        }
    }
}

pub(crate) struct Capture<'a> {
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub exit_code: i32,
    pub tools: &'a [ToolEvidence],
    pub coverage: Option<&'a CoverageEvidence>,
}

pub(crate) async fn collect(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    result: &mut CheckResult,
    capture: Capture<'_>,
) -> Result<ToolFacts> {
    let spec = check
        .tool_facts
        .as_ref()
        .context("Missing fact output contract")?;
    let tool = capture
        .tools
        .iter()
        .find(|tool| tool.id == spec.tool_id)
        .context("Declared fact tool was not probed")?;
    ensure!(
        tool.version == spec.tool_version,
        "Tool version probe does not match the pinned fact output contract"
    );
    if spec.adapter != ToolFormat::ToolFacts {
        ensure!(
            check.cwd == ".",
            "Native fact collection requires repository-root cwd"
        );
    }
    let bytes = if spec.from_stdout {
        ensure!(
            capture.stdout.len() <= crate::tool_evidence::contracts::MAX_FACT_BYTES,
            "Tool facts exceed byte budget"
        );
        capture.stdout.to_vec()
    } else {
        crate::report_evidence::generated_reports::read_report(workspace, &spec.path).await?
    };
    let artifact =
        crate::application::evidence::persist(artifacts, &check.id, "tool-facts-raw", &bytes)
            .await?;
    result.execution.artifacts.push(artifact.clone());
    let spec = spec.clone();
    let snapshot = Arc::clone(snapshot);
    let producer = check.id.clone();
    let coverage = capture.coverage.cloned();
    let receipt = ExecutionReceipt {
        argv: check.argv.clone(),
        exit_code: capture.exit_code,
        findings_exit: check.findings_exit_codes.contains(&capture.exit_code),
        stderr: std::str::from_utf8(capture.stderr)
            .context("Tool stderr must be UTF-8")?
            .to_owned(),
        tool_id: tool.id.clone(),
        tool_version: tool.version.clone(),
        scope_files: spec.paths.clone(),
    };
    let work = tokio::task::spawn_blocking(move || {
        let mut total = 0usize;
        let mut sources = BTreeMap::new();
        for path in &spec.paths {
            let file = snapshot.files.get(path).with_context(|| {
                format!("Declared fact source is absent from selected snapshot: {path}")
            })?;
            total = total
                .checked_add(file.bytes.len())
                .context("Source inventory overflow")?;
            ensure!(
                total <= 32 * 1024 * 1024,
                "Fact source inventory exceeds 32 MiB"
            );
            sources.insert(path.clone(), file.bytes.clone());
        }
        let inventory = crate::tool_evidence::inventory::expected_inventory(&spec, &sources)?;
        crate::tool_evidence::normalization::normalize(
            &bytes,
            &NormalizeContext {
                spec,
                producer,
                snapshot_digest: snapshot.identity.content_digest.clone(),
                artifact_digest: artifact.digest,
                inventory,
                coverage,
                execution: Some(receipt),
            },
        )
    });
    let facts = tokio::time::timeout(Duration::from_secs(30), work)
        .await
        .context("Fact collection exceeded 30 seconds")???;
    ensure!(facts.complete, "Tool fact collection is incomplete");
    let bytes = serde_json::to_vec(&facts)?;
    ensure!(
        bytes.len() <= crate::tool_evidence::contracts::MAX_FACT_BYTES,
        "Materialized tool facts exceed byte budget"
    );
    let artifact =
        crate::application::evidence::persist(artifacts, &check.id, "tool-facts.json", &bytes)
            .await?;
    result.execution.artifacts.push(artifact.clone());
    result.metadata.insert(
        "tool_facts".into(),
        serde_json::json!({
            "protocol_version":facts.protocol_version,"schema":facts.schema,"scope":facts.scope,
            "records":facts.records.len(),"complete":facts.complete,"artifact":artifact,
            "snapshot_digest":facts.snapshot_digest,"producer":facts.producer,
        }),
    );
    Ok(facts)
}

pub(crate) async fn prepare_context(
    check: &CommandCheck,
    workspace: &Path,
    directory: &Path,
    snapshot: &Snapshot,
    result: &mut CheckResult,
) -> Result<Option<Artifact>> {
    let Some(spec) = &check.tool_facts else {
        return Ok(None);
    };
    let Some(path) = &spec.context_path else {
        return Ok(None);
    };
    ensure!(
        !snapshot.files.contains_key(path)
            && !check.file_outputs(false).any(|output| output == path),
        "Fact context would overwrite a checked input or producer output"
    );
    let context = serde_json::json!({"protocol_version":1,"producer":check.id,"snapshot_digest":snapshot.identity.content_digest,"spec":spec});
    let bytes = serde_json::to_vec(&context)?;
    ensure!(
        bytes.len() <= crate::config::MAX_CONFIG_BYTES,
        "Fact execution context exceeds configuration budget"
    );
    let destination = crate::paths::confined(workspace, Path::new(path))?;
    tokio::fs::create_dir_all(destination.parent().context("Context has no parent")?).await?;
    match tokio::fs::remove_file(&destination).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    let retained = crate::application::evidence::persist(
        directory,
        &check.id,
        "tool-facts-context.json",
        &bytes,
    )
    .await?;
    result.execution.artifacts.push(retained.clone());
    Ok(Some(Artifact {
        path: destination.display().to_string(),
        ..retained
    }))
}

pub(crate) async fn verify_context(context: Option<&Artifact>) -> Result<()> {
    if let Some(context) = context {
        let actual = crate::runner::identity::evidence(Path::new(&context.path)).await?;
        ensure!(
            actual.digest == context.digest && actual.bytes == context.bytes,
            "Producer modified its declared execution context"
        );
    }
    Ok(())
}
