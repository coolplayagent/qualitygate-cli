//! Selected runtime identity and fresh projections over the captured CLI channel.
use super::{agent_host::Options, agent_host_io::*};
use crate::{
    domain::{Artifact, agent_contract::Projection, runtime::Capabilities},
    runner, snapshot,
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

pub(super) async fn unchanged(options: &Options, expected: &Artifact) -> Result<()> {
    let actual = runner::identity::selected_evaluator(&options.qualitygate).await?;
    ensure!(
        actual.digest == expected.digest && actual.bytes == expected.bytes,
        "Selected Qualitygate executable changed during the agent session"
    );
    Ok(())
}

pub(super) async fn runtime(options: &Options, deadline: Instant) -> Result<Artifact> {
    let pinned = runner::identity::selected_evaluator(&options.qualitygate).await?;
    let output = command(
        &[
            options.qualitygate.display().to_string(),
            "capabilities".into(),
            "--format".into(),
            "json".into(),
        ],
        &options.root,
        None,
        deadline,
    )
    .await?;
    ensure!(
        output.exit_code == Some(0) && !output.timed_out && output.capture_error.is_none(),
        "Cannot establish selected Qualitygate capabilities"
    );
    let capabilities: Capabilities = serde_json::from_slice(&output.stdout)?;
    ensure!(
        capabilities.schema_version == 1
            && capabilities.executable.digest == pinned.digest
            && capabilities.executable.bytes == pinned.bytes
            && ["agent.contracts.v1", "agent.delivery.v1"]
                .iter()
                .all(|id| capabilities.capabilities.iter().any(|v| v == id)),
        "Selected Qualitygate lacks the matching agent contract/delivery protocols or identity"
    );
    unchanged(options, &pinned).await?;
    Ok(pinned)
}

pub(super) async fn project(
    options: &Options,
    evidence: &Path,
    label: &str,
    deadline: Instant,
    runtime: &Artifact,
) -> Result<Projection> {
    unchanged(options, runtime).await?;
    let argv = vec![
        options.qualitygate.display().to_string(),
        "--root".into(),
        options.root.display().to_string(),
        "--config".into(),
        options.config.clone(),
        "agent-contract".into(),
        "--worktree".into(),
        "--base".into(),
        options.base.clone(),
        "--policy-ref".into(),
        options.policy_ref.clone(),
        "--task".into(),
        options.task.clone(),
        "--role".into(),
        options.role.clone(),
        "--format".into(),
        "json".into(),
    ];
    let output = command(&argv, &options.root, None, deadline).await?;
    save(evidence, &format!("{label}-contract.json"), &output.stdout)?;
    save(
        evidence,
        &format!("{label}-contract-stderr.log"),
        &output.stderr,
    )?;
    if output.timed_out {
        return Err(TimeBudgetExpired.into());
    }
    ensure!(
        output.capture_error.is_none(),
        "Contract projection execution is incomplete"
    );
    let projection: Projection = serde_json::from_slice(&output.stdout)?;
    ensure!(
        projection.schema_version == 1
            && projection.kind == "agent_contract_projection"
            && projection.role == options.role
            && projection.snapshot.base == options.base
            && projection.policy.resolved_commit.as_deref() == Some(&options.policy_ref)
            && projection.policy.task_contract_source.as_deref() == Some(&options.task)
            && projection.prompt_digest == snapshot::digest(projection.prompt.as_bytes())
            && output.exit_code == Some(if projection.ready { 0 } else { 2 }),
        "Contract projection binding or exit status is inconsistent"
    );
    unchanged(options, runtime).await?;
    Ok(projection)
}

pub(super) fn resume(options: &Options, evidence: &Path) -> Result<Option<Value>> {
    let Some(path) = &options.resume else {
        return Ok(None);
    };
    let path = path.canonicalize()?;
    ensure!(
        !path.starts_with(&options.root),
        "Resume context must be outside checked inputs"
    );
    let bytes = read(&path, 2 * 1024 * 1024)?;
    let context: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        context["kind"] == "external_agent_loop"
            && context["root"].as_str() == options.root.to_str(),
        "Resume context belongs to another controller/repository"
    );
    let count = context["attempts"].as_array().map_or(0, Vec::len);
    let text = |key: &str| {
        context[key]
            .as_str()
            .unwrap_or("unknown")
            .chars()
            .take(256)
            .collect::<String>()
    };
    let changed_agent = context["agent_argv"]
        != serde_json::from_slice::<Value>(&read(&options.agent_command, 16 * 1024)?)?;
    Ok(Some(
        json!({"source":save(evidence,"resume-context.json",&bytes)?,"termination":text("termination"),
        "attempts":count,"previous_contract":context["contract_digest"].as_str().map(|s| s.chars().take(128).collect::<String>()),
        "agent_changed":changed_agent,"evidence_reused":false,"budget":"fresh explicit invocation"}),
    ))
}
