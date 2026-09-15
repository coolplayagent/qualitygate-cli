//! Two bounded executions with explicit counterexample evidence per changed file.

use crate::{
    config::CommandCheck,
    domain::{
        test_effectiveness::{self as decision, TestCase},
        *,
    },
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn execute(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    protected_paths: &[String],
    max_bytes: usize,
) -> CheckResult {
    let mut result = CheckResult::pending(&check.id, check.required, check.severity);
    if let Err(error) = run(
        check,
        artifacts,
        snapshot,
        protected_paths,
        max_bytes,
        &mut result,
    )
    .await
    {
        result.block(ExecutionStatus::Blocked, format!("{error:#}"));
    }
    result
}

async fn run(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    protected_paths: &[String],
    max_bytes: usize,
    result: &mut CheckResult,
) -> Result<()> {
    let spec = check
        .test_effectiveness
        .as_ref()
        .context("Missing test effectiveness configuration")?;
    let (input, selectors, protected) =
        (Arc::clone(snapshot), spec.clone(), protected_paths.to_vec());
    let (prepared, selection_evidence) = tokio::task::spawn_blocking(move || {
        let prepared = snapshot::test_overlay::prepare(
            &input,
            &selectors.source_paths,
            &selectors.test_paths,
            &selectors.support_paths,
            &protected,
            max_bytes,
        )?;
        let evidence = json!({
            "source_changed":prepared.source_changed, "selected_test_files":prepared.tests,
            "overlay":prepared.overlay, "current_snapshot":input.identity,
            "baseline_snapshot":prepared.baseline.identity,
        });
        Ok::<_, anyhow::Error>((prepared, evidence))
    })
    .await??;
    result
        .metadata
        .insert("test_effectiveness".into(), selection_evidence);
    if !prepared.source_changed {
        result.skip("No production files changed in the declared source scope");
        return Ok(());
    }
    if prepared.tests.is_empty() {
        violation(
            result,
            None,
            "Production changes have no added or content-modified independent test file",
            "missing-tests",
        );
        result.complete();
        return Ok(());
    }
    if snapshot.files.contains_key(&check.reports[0].path)
        || prepared.baseline.files.contains_key(&check.reports[0].path)
    {
        bail!("Test report output would overwrite a captured input");
    }
    let baseline = Arc::new(prepared.baseline);
    let workspace = snapshot::materialize_shared(Arc::clone(snapshot)).await?;
    let current_inputs =
        snapshot::InputGuard::for_snapshot(workspace.path(), Arc::clone(snapshot)).await?;
    let deadline = Instant::now() + Duration::from_secs(check.timeout_seconds);
    let current = Arc::new(
        super::commands::execute_until(
            check,
            workspace.path(),
            artifacts,
            snapshot,
            &current_inputs,
            Some(deadline),
        )
        .await,
    );
    retain(result, "current", &current).await?;
    let (observed, selected, allowed) = (
        Arc::clone(&current),
        prepared.tests.clone(),
        spec.assertion_failure_types.clone(),
    );
    let passes = tokio::task::spawn_blocking(move || {
        decision::current_passes(&cases(&observed)?, &selected, &allowed)
            .map_err(anyhow::Error::msg)
    })
    .await??;
    if !passes {
        violation(
            result,
            None,
            "Tests fail on the current code",
            "current-failure",
        );
        result.complete();
        return Ok(());
    }
    if current.execution.exit_code != Some(0) {
        bail!("Current tests did not exit successfully");
    }
    // Release the current materialization before acquiring another full workspace.
    drop(current_inputs);
    drop(workspace);
    let workspace = snapshot::materialize_shared(Arc::clone(&baseline)).await?;
    let base_inputs =
        snapshot::InputGuard::for_snapshot(workspace.path(), Arc::clone(&baseline)).await?;
    // Evidence uses the existing hashed check naming scheme with a separate directory.
    let base_artifacts = artifacts.join(format!(
        "effectiveness-{}",
        snapshot::digest(check.id.as_bytes()).trim_start_matches("sha256:")
    ));
    tokio::fs::create_dir(&base_artifacts).await?;
    let previous = Arc::new(
        super::commands::execute_until(
            check,
            workspace.path(),
            &base_artifacts,
            &baseline,
            &base_inputs,
            Some(deadline),
        )
        .await,
    );
    retain(result, "baseline", &previous).await?;
    let (selected, allowed, exits) = (
        prepared.tests,
        spec.assertion_failure_types.clone(),
        check.findings_exit_codes.clone(),
    );
    let (proof, proof_evidence) = tokio::task::spawn_blocking(move || {
        let current_cases = cases(&current)?;
        let baseline_cases = cases(&previous)?;
        super::tool_evidence::comparable(
            current
                .metadata
                .get("tools")
                .context("Missing current tool evidence")?,
            &serde_json::from_value::<Vec<super::tool_evidence::ToolEvidence>>(
                previous
                    .metadata
                    .get("tools")
                    .context("Missing baseline tool evidence")?
                    .clone(),
            )?,
        )?;
        if current
            .metadata
            .get("command_executable")
            .and_then(|v| v.get("digest"))
            != previous
                .metadata
                .get("command_executable")
                .and_then(|v| v.get("digest"))
        {
            bail!("Current and baseline command executable identities differ");
        }
        let proof = decision::compare(&current_cases, &baseline_cases, &selected, &allowed)
            .map_err(anyhow::Error::msg)?;
        if baseline_cases
            .iter()
            .any(|case| matches!(case.outcome, decision::TestOutcome::Failure { .. }))
            && !previous
                .execution
                .exit_code
                .is_some_and(|code| exits.contains(&code))
        {
            bail!("Baseline assertion failures contradict its successful exit code");
        }
        let evidence = serde_json::to_value(&proof)?;
        Ok::<_, anyhow::Error>((proof, evidence))
    })
    .await??;
    result
        .metadata
        .insert("test_effectiveness_files".into(), proof_evidence);
    for file in &proof {
        if file.counterexamples == 0 {
            violation(
                result,
                Some(&file.file),
                "Changed test file has no assertion counterexample on the old code",
                "missing-counterexample",
            );
        }
    }
    result.matched_entities = proof.iter().map(|file| file.executed).sum();
    result.complete();
    Ok(())
}

async fn retain(result: &mut CheckResult, side: &str, execution: &Arc<CheckResult>) -> Result<()> {
    let observed = Arc::clone(execution);
    let evidence = tokio::task::spawn_blocking(move || json!({"execution":observed.execution, "metadata":observed.metadata, "diagnostics":observed.diagnostics, "matched_entities":observed.matched_entities})).await?;
    result
        .execution
        .artifacts
        .extend(execution.execution.artifacts.clone());
    if side == "current" {
        result.execution.argv = execution.execution.argv.clone();
        result.execution.cwd = execution.execution.cwd.clone();
        result.execution.started_at_ms = execution.execution.started_at_ms;
    }
    result.execution.ended_at_ms = execution.execution.ended_at_ms;
    result.execution.duration_ms = result
        .execution
        .duration_ms
        .unwrap_or(0)
        .checked_add(execution.execution.duration_ms.unwrap_or(0));
    result
        .metadata
        .insert(format!("{side}_test_execution"), evidence);
    result.diagnostics.extend(execution.diagnostics.clone());
    Ok(())
}

fn cases(result: &CheckResult) -> Result<Vec<TestCase>> {
    if result.execution.status != ExecutionStatus::Completed {
        bail!(
            "Test execution is incomplete: {}",
            result
                .execution
                .reason
                .as_deref()
                .unwrap_or("execution did not complete")
        );
    }
    Ok(serde_json::from_value(
        result
            .metadata
            .get("test_cases")
            .context("Missing test case evidence")?
            .clone(),
    )?)
}

fn violation(result: &mut CheckResult, file: Option<&str>, message: &str, kind: &str) {
    result.diagnostics.push(crate::adapters::rules::diagnostic(
        &result.id, file, None, message.into(), json!({"assertion":kind}),
        "Add or repair a test that passes on the current code and asserts the changed behavior on the old code", &format!("{kind}:{}", file.unwrap_or("")),
    ));
}
