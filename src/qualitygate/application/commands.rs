use crate::{
    adapters::rules::diagnostic,
    config::{CheckKind, CommandCheck},
    domain::*,
    paths, runner, snapshot,
};
use std::{path::Path, sync::Arc, time::Duration};

pub(super) async fn execute(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    inputs: &snapshot::InputGuard,
) -> CheckResult {
    if let Err(error) = inputs.verify().await {
        let mut result = CheckResult::pending(&check.id, check.required, check.severity);
        result.block(
            ExecutionStatus::Blocked,
            format!("Command was not executed: {error:#}"),
        );
        result.metadata.insert("input_integrity".into(), serde_json::json!({"snapshot_digest":inputs.digest,"before":"invalid","reason":format!("{error:#}")}));
        return result;
    }
    let mut result = execute_checked(check, workspace, artifacts, snapshot, inputs).await;
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
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    inputs: &snapshot::InputGuard,
) -> CheckResult {
    let mut result = CheckResult::pending(&check.id, check.required, check.severity);
    result.applicability = Applicability::Applicable;
    result.execution.argv = check.argv.clone();
    result.execution.cwd = Some(check.cwd.clone());
    if check.kind == CheckKind::Manual {
        result.block(
            ExecutionStatus::Blocked,
            "Manual acceptance requires a verified external record",
        );
        return result;
    }
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
    if (!check.reports.is_empty() || !check.projects.is_empty()) && check.tools.is_empty() {
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
    let tools = match super::tool_evidence::collect(
        check,
        workspace,
        artifacts,
        snapshot,
        &mut result,
        false,
    )
    .await
    {
        Ok(tools) => tools,
        Err(error) => {
            result.block(ExecutionStatus::ToolError, format!("{error:#}"));
            return result;
        }
    };
    if let Err(error) = inputs.verify().await {
        result.block(
            ExecutionStatus::Blocked,
            format!("Tool version probes changed checked inputs: {error:#}"),
        );
        return result;
    }
    if let Err(error) = async {
        let current = runner::identity::executable(&check.argv[0], &cwd).await?;
        if current.path != executable.path || current.digest != executable.digest {
            anyhow::bail!("Command executable changed during version probing");
        }
        super::tool_evidence::verify(check, workspace, &tools).await
    }
    .await
    {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    if let Err(error) = super::generated_reports::prepare(check, workspace, false).await {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    match runner::capture(
        &resolved_argv,
        &cwd,
        None,
        Duration::from_secs(check.timeout_seconds),
    )
    .await
    {
        Ok(output) => {
            result.execution.started_at_ms = Some(output.started_at_ms);
            result.execution.ended_at_ms = Some(output.ended_at_ms);
            result.execution.duration_ms = Some(output.duration_ms);
            result.execution.exit_code = output.exit_code;
            let test_counts = super::test_counts::from_output(check, &output.stdout);
            for (suffix, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                match super::evidence::persist(
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
                super::tool_evidence::verify(check, workspace, &tools).await
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
            if let Some(error) = output.capture_error {
                result.block(ExecutionStatus::ToolError, error);
                return result;
            }
            if output.exit_code.is_none() {
                result.block(
                    ExecutionStatus::ToolError,
                    "Command terminated without an exit code",
                );
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
            let findings = match super::generated_reports::collect(
                check,
                workspace,
                artifacts,
                snapshot,
                &mut result,
            )
            .await
            {
                Ok(findings) => findings,
                Err(error) => {
                    result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                    return result;
                }
            };
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
                if let Err(error) = super::project_reports::collect(
                    check,
                    workspace,
                    artifacts,
                    snapshot,
                    &output.stdout,
                    &mut result,
                )
                .await
                {
                    result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                    return result;
                }
            }
            result.complete();
        }
        Err(error) => result.block(ExecutionStatus::ToolError, format!("{error:#}")),
    }
    result
}
