use crate::{
    adapters::rules::diagnostic,
    config::{CheckKind, CommandCheck},
    domain::*,
    paths, runner, snapshot,
};
use std::{path::Path, time::Duration};

pub(super) async fn execute(
    check: &CommandCheck,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &snapshot::Snapshot,
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
    if let Err(error) = super::generated_reports::prepare(check, workspace, false).await {
        result.block(ExecutionStatus::ToolError, format!("{error:#}"));
        return result;
    }
    match runner::capture(
        &check.argv,
        &cwd,
        None,
        Duration::from_secs(check.timeout_seconds),
    )
    .await
    {
        Ok(output) => {
            result.execution.started_at_ms = Some(output.started_at_ms);
            result.execution.duration_ms = Some(output.duration_ms);
            result.execution.exit_code = output.exit_code;
            let test_counts = super::test_counts::from_output(check, &output.stdout);
            for (suffix, bytes) in [("stdout", output.stdout), ("stderr", output.stderr)] {
                let path = artifacts.join(format!("{}.{suffix}.log", check.id));
                if let Err(error) = tokio::fs::write(&path, &bytes).await {
                    result.block(
                        ExecutionStatus::ToolError,
                        format!("Cannot persist process evidence: {error}"),
                    );
                    return result;
                }
                result.execution.artifacts.push(Artifact {
                    path: path.display().to_string(),
                    digest: snapshot::digest(&bytes),
                    bytes: bytes.len() as u64,
                });
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
            if output.exit_code != Some(check.expected_exit_code) {
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
            if let Err(error) = super::generated_reports::collect(
                check,
                workspace,
                artifacts,
                snapshot,
                &mut result,
            )
            .await
            {
                result.block(ExecutionStatus::ToolError, format!("{error:#}"));
                return result;
            }
            result.complete();
        }
        Err(error) => result.block(ExecutionStatus::ToolError, format!("{error:#}")),
    }
    result
}
