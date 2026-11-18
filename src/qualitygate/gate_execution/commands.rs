use super::execution_outcome::{ExecutionOutcome, ProducedEvidence};
use crate::{
    config::CommandCheck, domain::*, paths, report_evidence::report::diagnostic, runner, snapshot,
};
use std::{path::Path, sync::Arc, time::Duration};

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

pub(crate) async fn execute(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    inputs: &snapshot::InputGuard,
) -> CheckResult {
    execute_until(check, workspace, artifacts, snapshot, inputs, None).await
}

pub(crate) async fn execute_until(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    inputs: &snapshot::InputGuard,
    deadline: Option<std::time::Instant>,
) -> CheckResult {
    execute_outcome(
        check, workspace, artifacts, snapshot, inputs, deadline, None,
    )
    .await
    .check_result
}

pub(crate) async fn execute_outcome(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    inputs: &snapshot::InputGuard,
    deadline: Option<std::time::Instant>,
    coverage: Option<&crate::tool_evidence::contracts::CoverageEvidence>,
) -> ExecutionOutcome {
    let mut evidence = ProducedEvidence::default();
    let context = ExecutionInputs {
        check,
        workspace,
        artifacts,
        snapshot,
        inputs,
        deadline,
        coverage,
    };
    let result = execute_guarded(context, &mut evidence).await;
    ExecutionOutcome::new(result, evidence)
}

#[derive(Clone, Copy)]
struct ExecutionInputs<'a> {
    check: &'a CommandCheck,
    workspace: &'a Path,
    artifacts: &'a Path,
    snapshot: &'a Arc<snapshot::Snapshot>,
    inputs: &'a snapshot::InputGuard,
    deadline: Option<std::time::Instant>,
    coverage: Option<&'a crate::tool_evidence::contracts::CoverageEvidence>,
}

async fn execute_guarded(
    context: ExecutionInputs<'_>,
    evidence: &mut ProducedEvidence,
) -> CheckResult {
    let ExecutionInputs { check, inputs, .. } = context;
    if let Some(spec) = &check.tool_facts
        && let Err(error) =
            crate::tool_evidence::normalization::validate_invocation(spec, &check.argv)
    {
        let mut result = CheckResult::pending(&check.id, check.required, check.severity);
        result.block(
            ExecutionStatus::Blocked,
            format!("Fact producer invocation is invalid: {error:#}"),
        );
        return result;
    }
    let missing = crate::env::missing_required(&check.required_env);
    if !missing.is_empty() {
        let mut result = CheckResult::pending(&check.id, check.required, check.severity);
        result.block(
            ExecutionStatus::Blocked,
            format!(
                "Required environment variables are missing or empty: {}",
                missing.join(", ")
            ),
        );
        result
            .metadata
            .insert("required_env_missing".into(), serde_json::json!(missing));
        return result;
    }
    if let Err(error) = inputs.verify().await {
        let mut result = CheckResult::pending(&check.id, check.required, check.severity);
        result.block(
            ExecutionStatus::Blocked,
            format!("Command was not executed: {error:#}"),
        );
        result.metadata.insert("input_integrity".into(), serde_json::json!({"snapshot_digest":inputs.digest,"before":"invalid","reason":format!("{error:#}")}));
        return result;
    }
    let mut result = execute_checked(context, evidence).await;
    match inputs.verify().await {
        Ok(()) => {
            result.metadata.insert("input_integrity".into(), serde_json::json!({"snapshot_digest":inputs.digest,"before":"verified","after":"verified"}));
        }
        Err(error) => {
            let previous = result
                .execution
                .reason
                .as_ref()
                .map(|reason| format!("; prior result: {reason}"))
                .unwrap_or_default();
            result.metadata.insert("input_integrity".into(), serde_json::json!({"snapshot_digest":inputs.digest,"before":"verified","after":"invalid","prior_status":result.execution.status,"prior_reason":result.execution.reason,"reason":format!("{error:#}")}));
            result.block(
                ExecutionStatus::Blocked,
                format!("Command evidence is invalid: {error:#}{previous}"),
            );
        }
    }
    result
}

async fn execute_checked(
    context: ExecutionInputs<'_>,
    evidence: &mut ProducedEvidence,
) -> CheckResult {
    let ExecutionInputs {
        check,
        workspace,
        artifacts,
        snapshot,
        inputs,
        deadline,
        coverage,
    } = context;
    let mut result = CheckResult::pending(&check.id, check.required, check.severity);
    result.applicability = Applicability::Applicable;
    result.execution.argv = check.argv.clone();
    result.execution.cwd = Some(check.cwd.clone());
    let cwd = match paths::confined(workspace, Path::new(&check.cwd)) {
        Ok(path) => path,
        Err(error) => {
            result.block(ExecutionStatus::Blocked, error.to_string());
            return result;
        }
    };
    for arg in &check.required_args {
        if !check.argv.contains(arg) {
            result.diagnostics.push(diagnostic(
                &check.id,
                None,
                None,
                format!("Required command argument is absent: {arg}"),
                serde_json::json!({"argv":check.argv}),
                "Restore the required build arguments",
                arg,
            ));
        }
    }
    if !result.diagnostics.is_empty() {
        result.block(
            ExecutionStatus::Blocked,
            "Command was not executed because required arguments are missing",
        );
        return result;
    }
    if (!check.reports.is_empty() || !check.projects.is_empty() || check.tool_facts.is_some())
        && check.tools.is_empty()
    {
        result.block(
            ExecutionStatus::Blocked,
            "Report-producing commands require tools with executable version probes",
        );
        return result;
    }
    let executable = match runner::identity::executable(&check.argv[0], &cwd).await {
        Ok(identity) => identity,
        Err(error) => {
            result.block(ExecutionStatus::ToolError, format!("{error:#}"));
            return result;
        }
    };
    let mut resolved_argv = check.argv.clone();
    resolved_argv[0] = executable.path.clone();
    result
        .metadata
        .insert("resolved_argv".into(), serde_json::json!(resolved_argv));
    result
        .metadata
        .insert("command_executable".into(), serde_json::json!(executable));
    result.metadata.insert(
        "execution_snapshot".into(),
        serde_json::json!(snapshot.identity),
    );
    let (os, arch) = crate::env::platform();
    let dependency_inputs: std::collections::BTreeMap<_, _> = snapshot
        .files
        .iter()
        .filter(|(name, _)| {
            Path::new(name).file_name().is_some_and(|name| {
                [
                    "Cargo.lock",
                    "Cargo.toml",
                    "pom.xml",
                    "pyproject.toml",
                    "requirements.txt",
                    "poetry.lock",
                    "uv.lock",
                    "Pipfile.lock",
                    "package.json",
                    "package-lock.json",
                    "yarn.lock",
                    "pnpm-lock.yaml",
                    "go.mod",
                    "go.sum",
                    "gradle.lockfile",
                ]
                .iter()
                .any(|known| name == *known)
            })
        })
        .map(|(name, file)| (name, snapshot::digest(&file.bytes)))
        .collect();
    result.metadata.insert("environment".into(), serde_json::json!({"os":os,"architecture":arch,"qualitygate_version":env!("CARGO_PKG_VERSION"),"dependency_inputs":dependency_inputs}));
    let tools = match crate::gate_execution::tool_evidence::collect_until(
        check,
        workspace,
        artifacts,
        snapshot,
        &mut result,
        false,
        deadline,
    )
    .await
    {
        Ok(tools) => tools,
        Err(error) => {
            result.block(ExecutionStatus::ToolError, format!("{error:#}"));
            return result;
        }
    };
    if let Some(spec) = &check.tool_facts
        && !tools
            .iter()
            .any(|tool| tool.id == spec.tool_id && tool.version == spec.tool_version)
    {
        result.block(
            ExecutionStatus::ToolError,
            "Tool version probe does not match the pinned fact output contract",
        );
        return result;
    }
    if let Some(spec) = &check.tool_facts
        && spec.adapter != crate::tool_evidence::contracts::ToolFormat::ToolFacts
        && !tools.iter().any(|tool| {
            tool.id == "tree-sitter-language-pack"
                && tool.version == crate::tool_evidence::contracts::LANGUAGE_PACK_VERSION
        })
    {
        result.block(
            ExecutionStatus::ToolError,
            "Native language-pack version probe does not match the audited runtime",
        );
        return result;
    }
    if let Err(error) = async {
        let current = runner::identity::executable(&check.argv[0], &cwd).await?;
        if current.path != executable.path || current.digest != executable.digest {
            anyhow::bail!("Command executable changed during version probing");
        }
        crate::gate_execution::tool_evidence::verify(check, workspace, &tools).await
    }
    .await
    {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    if let Err(error) =
        crate::application::generated_reports::prepare(check, workspace, &snapshot.files, false)
            .await
    {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    if let Err(error) = inputs.verify().await {
        result.block(
            ExecutionStatus::Blocked,
            format!("Tool probes or report preparation changed checked inputs: {error:#}"),
        );
        return result;
    }
    if let Err(error) = super::fact_revalidation::coverage_inputs(check, workspace, coverage).await
    {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    let fact_context = match super::fact_execution::prepare_context(
        check,
        workspace,
        artifacts,
        snapshot,
        &mut result,
    )
    .await
    {
        Ok(context) => context,
        Err(error) => {
            result.block(ExecutionStatus::ToolError, format!("{error:#}"));
            return result;
        }
    };
    let native_runtime = match async {
        if let Some(spec) = &check.tool_facts {
            super::native_runtime::prepare(spec, snapshot, artifacts, &check.id).await
        } else {
            Ok(None)
        }
    }
    .await
    {
        Ok(runtime) => runtime,
        Err(error) => {
            result.block(ExecutionStatus::ToolError, format!("{error:#}"));
            return result;
        }
    };
    if let Some(runtime) = &native_runtime {
        result.execution.artifacts.extend(runtime.artifacts.clone());
        result.metadata.insert("native_runtime".into(), serde_json::json!({
            "profile":"linux-x86_64-rust-v1", "pack_version":crate::tool_evidence::contracts::LANGUAGE_PACK_VERSION,
            "parser_digest":crate::tool_evidence::contracts::LINUX_RUST_PARSER_DIGEST,
            "downloads":"disabled", "artifacts":runtime.artifacts,
        }));
    }
    let no_overrides = std::collections::BTreeMap::new();
    let remaining = deadline
        .map(|end| end.saturating_duration_since(std::time::Instant::now()))
        .unwrap_or(Duration::from_secs(check.timeout_seconds));
    if remaining.is_zero() {
        result.block(
            ExecutionStatus::TimedOut,
            "Paired test execution exhausted its shared deadline",
        );
        return result;
    }
    match runner::capture_with_env(
        &resolved_argv,
        &cwd,
        None,
        remaining.min(Duration::from_secs(check.timeout_seconds)),
        native_runtime
            .as_ref()
            .map_or(&no_overrides, |runtime| &runtime.env),
    )
    .await
    {
        Ok(output) => {
            result.execution.started_at_ms = Some(output.started_at_ms);
            result.execution.ended_at_ms = Some(output.ended_at_ms);
            result.execution.duration_ms = Some(output.duration_ms);
            result.execution.exit_code = output.exit_code;
            let (output, test_counts) =
                crate::application::test_counts::from_capture(check, output).await;
            for (suffix, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                match crate::application::evidence::persist(
                    artifacts,
                    &check.id,
                    &format!("{suffix}.log"),
                    bytes,
                )
                .await
                {
                    Ok(artifact) => result.execution.artifacts.push(artifact),
                    Err(error) => {
                        result.block(
                            ExecutionStatus::ToolError,
                            format!("Cannot persist process evidence: {error}"),
                        );
                        return result;
                    }
                }
            }
            if let Err(error) = async {
                let after = runner::identity::executable(&check.argv[0], &cwd).await?;
                if after.path != executable.path || after.digest != executable.digest {
                    anyhow::bail!("Command executable changed during execution");
                }
                crate::gate_execution::tool_evidence::verify(check, workspace, &tools).await
            }
            .await
            {
                result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                return result;
            }
            if output.timed_out {
                result.block(
                    ExecutionStatus::TimedOut,
                    "Command exceeded its configured deadline",
                );
                return result;
            }
            if let Some(error) = &output.capture_error {
                result.block(ExecutionStatus::ToolError, error.as_str());
                return result;
            }
            if output.exit_code.is_none() {
                result.block(
                    ExecutionStatus::ToolError,
                    "Command terminated without an exit code",
                );
                return result;
            }
            if check.tool_facts.is_some() {
                if output.exit_code != Some(check.expected_exit_code)
                    && !output
                        .exit_code
                        .is_some_and(|code| check.findings_exit_codes.contains(&code))
                {
                    result.block(
                        ExecutionStatus::ToolError,
                        "Fact producer returned an undeclared exit code",
                    );
                    return result;
                }
                let facts = async {
                    super::native_runtime::verify(native_runtime.as_ref()).await?;
                    super::fact_execution::verify_context(fact_context.as_ref()).await?;
                    super::fact_revalidation::coverage_inputs(check, workspace, coverage).await?;
                    super::fact_execution::collect(
                        check,
                        workspace,
                        artifacts,
                        snapshot,
                        &mut result,
                        super::fact_execution::Capture {
                            stdout: &output.stdout,
                            stderr: &output.stderr,
                            exit_code: output.exit_code.expect("checked exit"),
                            tools: &tools,
                            coverage,
                        },
                    )
                    .await
                }
                .await;
                match facts {
                    Ok(facts) => {
                        evidence.tool_facts = Some(facts);
                        result.complete();
                        evidence.receipt = Some(super::fact_execution::Receipt::new(
                            executable, tools, &result,
                        ));
                    }
                    Err(error) => result.block(ExecutionStatus::ToolError, format!("{error:#}")),
                }
                return result;
            }
            match test_counts {
                Ok(Some(tests)) => {
                    result
                        .metadata
                        .insert("tests".into(), serde_json::json!({"executed":tests}));
                    if tests == 0 {
                        result.diagnostics.push(diagnostic(
                            &check.id,
                            None,
                            None,
                            "No tests executed".into(),
                            serde_json::json!({"executed":0}),
                            "Ensure the intended tests are discovered and executed",
                            "test-count",
                        ));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    result.block(ExecutionStatus::ToolError, error.to_string());
                    return result;
                }
            }
            if output.exit_code != Some(check.expected_exit_code)
                && !output
                    .exit_code
                    .is_some_and(|code| check.findings_exit_codes.contains(&code))
            {
                result.diagnostics.push(diagnostic(
                    &check.id,
                    None,
                    None,
                    format!(
                        "Command exited {:?}; expected {}",
                        output.exit_code, check.expected_exit_code
                    ),
                    serde_json::json!({"argv":check.argv,"exit_code":output.exit_code}),
                    "Inspect the attached logs, repair the failure and rerun this check",
                    "exit-code",
                ));
            }
            let reports = match crate::application::generated_reports::collect_with_evidence(
                check,
                workspace,
                artifacts,
                snapshot,
                &mut result,
                &output.stdout,
            )
            .await
            {
                Ok(reports) => reports,
                Err(error) => {
                    result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                    return result;
                }
            };
            let findings = reports.findings;
            evidence.coverage = reports.coverage;
            if output
                .exit_code
                .is_some_and(|code| check.findings_exit_codes.contains(&code))
            {
                if !findings {
                    result.block(
                        ExecutionStatus::ToolError,
                        "Findings exit code is contradicted by reports without findings",
                    );
                    return result;
                }
            } else if output.exit_code != Some(check.expected_exit_code)
                && !check.reports.is_empty()
            {
                result.block(
                    ExecutionStatus::ToolError,
                    "Report-producing command returned an unexpected analyzer exit code",
                );
                return result;
            }
            if !check.projects.is_empty() {
                if output.exit_code != Some(0) {
                    result.block(
                        ExecutionStatus::ToolError,
                        "Project analysis did not succeed",
                    );
                    return result;
                }
                match crate::application::project_reports::collect(
                    check,
                    workspace,
                    artifacts,
                    snapshot,
                    &output.stdout,
                    &mut result,
                )
                .await
                {
                    Ok(projects) => evidence.projects = projects,
                    Err(error) => {
                        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                        return result;
                    }
                }
            }
            result.complete();
            if evidence.coverage.is_some() || !evidence.projects.is_empty() {
                evidence.receipt = Some(super::fact_execution::Receipt::new(
                    executable, tools, &result,
                ));
            }
        }
        Err(error) => result.block(ExecutionStatus::ToolError, format!("{error:#}")),
    }
    result
}
