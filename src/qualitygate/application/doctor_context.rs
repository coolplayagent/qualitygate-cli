//! Static command readiness and optional version probes over private checked bytes.

use super::{doctor::diagnostic, tool_evidence};
use crate::{
    config::{CheckKind, CommandCheck, Plan},
    domain::{CheckResult, doctor::*},
    paths, runner, snapshot,
};
use anyhow::{Context, Result, ensure};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn inspect(
    plan: &Plan,
    workspace: &Path,
    snapshot: &snapshot::Snapshot,
    report: &mut Report,
) {
    for command in plan
        .order
        .iter()
        .filter_map(|id| plan.commands.iter().find(|command| &command.id == id))
    {
        if command.kind != CheckKind::Command {
            continue;
        }
        let mut check = Check {
            id: command.id.clone(),
            executable: None,
            probes: Vec::new(),
            required_env: command
                .required_env
                .iter()
                .map(|name| Environment {
                    name: name.clone(),
                    present: crate::env::required_present(name),
                })
                .collect(),
        };
        let missing = crate::env::missing_required(&command.required_env);
        if !missing.is_empty() {
            report.diagnostics.push(Diagnostic::new(
                "environment.missing",
                format!(
                    "Required environment variables are missing or empty: {}",
                    missing.join(", ")
                ),
                vec![command.id.clone()],
            ));
        }
        let root = workspace.to_owned();
        let path = command.cwd.clone();
        let cwd = tokio::task::spawn_blocking(move || -> Result<_> {
            let path = paths::confined(&root, Path::new(&path))?;
            ensure!(
                std::fs::metadata(&path)
                    .context("Command working directory is unavailable")?
                    .is_dir(),
                "Command working directory is not a directory"
            );
            Ok(path)
        })
        .await;
        let cwd = match cwd {
            Ok(Ok(cwd)) => Some(cwd),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.into(),
                    _ => unreachable!(),
                };
                report.diagnostics.push(diagnostic(
                    "command.cwd_missing",
                    &error,
                    vec![command.id.clone()],
                ));
                None
            }
        };
        if let Some(cwd) = &cwd {
            match runner::identity::executable(&command.argv[0], cwd).await {
                Ok(identity) => check.executable = Some(identity),
                Err(error) => report.diagnostics.push(diagnostic(
                    "command.executable_missing",
                    &error,
                    vec![command.id.clone()],
                )),
            }
        }
        for arg in &command.required_args {
            if !command.argv.contains(arg) {
                report.diagnostics.push(Diagnostic::new(
                    "command.argument_missing",
                    format!("Required argument is absent: {arg}"),
                    vec![command.id.clone()],
                ));
            }
        }
        if (!command.reports.is_empty() || !command.projects.is_empty()) && command.tools.is_empty()
        {
            report.diagnostics.push(Diagnostic::new(
                "probe.declaration_missing",
                "Report-producing commands require declared tool version probes",
                vec![command.id.clone()],
            ));
        }
        for tool in &command.tools {
            let mut probe = Probe {
                id: tool.id.clone(),
                executable: None,
                attempted: false,
                complete: false,
                evidence: None,
            };
            for path in &tool.inputs {
                if !snapshot.files.contains_key(path) {
                    report.diagnostics.push(Diagnostic::new(
                        "probe.input_missing",
                        format!("Tool input missing from checked snapshot: {path}"),
                        vec![command.id.clone()],
                    ));
                }
            }
            if let Some(cwd) = &cwd {
                match runner::identity::executable(&tool.argv[0], cwd).await {
                    Ok(identity) => probe.executable = Some(identity),
                    Err(error) => report.diagnostics.push(diagnostic(
                        "probe.executable_missing",
                        &error,
                        vec![command.id.clone()],
                    )),
                }
            }
            check.probes.push(probe);
        }
        report.checks.push(check);
    }
}

pub(super) async fn probe(
    plan: &Plan,
    workspace: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    guard: &snapshot::InputGuard,
    report: &mut Report,
) -> Result<()> {
    probe_until(
        plan,
        workspace,
        snapshot,
        guard,
        report,
        Instant::now() + Duration::from_secs(PROBE_SECONDS),
    )
    .await
}

async fn probe_until(
    plan: &Plan,
    workspace: &Path,
    snapshot: &Arc<snapshot::Snapshot>,
    guard: &snapshot::InputGuard,
    report: &mut Report,
    deadline: Instant,
) -> Result<()> {
    let directory = tokio::task::spawn_blocking(tempfile::tempdir).await??;
    for check in &mut report.checks {
        let command = plan
            .commands
            .iter()
            .find(|command| command.id == check.id)
            .expect("selected command");
        for probe in &mut check.probes {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                report.diagnostics.push(Diagnostic::new(
                    "probe.budget_exhausted",
                    "Tool probes exhausted the shared 120-second budget",
                    vec![check.id.clone()],
                ));
                continue;
            }
            if let Err(error) = guard.verify().await {
                report.diagnostics.push(diagnostic(
                    "snapshot.inputs_changed",
                    &error,
                    vec![check.id.clone()],
                ));
                continue;
            }
            let tool = command
                .tools
                .iter()
                .find(|tool| tool.id == probe.id)
                .expect("selected tool");
            if let Err(error) = verify_executables(
                command,
                check.executable.as_ref(),
                &probe.executable,
                tool,
                workspace,
            )
            .await
            {
                report.diagnostics.push(diagnostic(
                    "probe.executable_changed",
                    &error,
                    vec![check.id.clone()],
                ));
                continue;
            }
            let mut single = command.clone();
            single.tools = vec![tool.clone()];
            let mut result = CheckResult::pending(&check.id, command.required, command.severity);
            probe.attempted = true;
            let collected = tokio::time::timeout(
                remaining,
                tool_evidence::collect_until(
                    &single,
                    workspace,
                    directory.path(),
                    snapshot,
                    &mut result,
                    false,
                    Some(deadline),
                ),
            )
            .await;
            let mut complete = match collected {
                Ok(Ok(_)) => true,
                Ok(Err(error)) => {
                    let code = error
                        .downcast_ref::<tool_evidence::ProbeError>()
                        .map(|error| error.code)
                        .unwrap_or("probe.incomplete");
                    report
                        .diagnostics
                        .push(diagnostic(code, &error, vec![check.id.clone()]));
                    false
                }
                Err(_) => {
                    report.diagnostics.push(Diagnostic::new(
                        "probe.budget_exhausted",
                        "Tool probes exhausted the shared 120-second budget",
                        vec![check.id.clone()],
                    ));
                    false
                }
            };
            // Do not publish producer stdout, stderr or version strings: a probe can
            // print inherited credentials. Retain bounded output identities instead.
            if let Some(evidence) = result.metadata.get("tools").and_then(|tools| tools.get(0)) {
                let mut sanitized = serde_json::json!({});
                for name in [
                    "exit_code",
                    "timed_out",
                    "duration_ms",
                    "started_at_ms",
                    "ended_at_ms",
                    "inputs",
                    "snapshot_digest",
                ] {
                    sanitized[name] = evidence[name].clone();
                }
                sanitized["version_digest"] =
                    snapshot::digest(evidence["version"].as_str().unwrap_or_default().as_bytes())
                        .into();
                for name in ["stdout", "stderr"] {
                    sanitized[name] = serde_json::json!({"digest": evidence[name]["digest"], "bytes": evidence[name]["bytes"]});
                }
                probe.evidence = Some(sanitized);
            }
            if let Err(error) = guard.verify().await {
                complete = false;
                report.diagnostics.push(diagnostic(
                    "snapshot.inputs_changed",
                    &error,
                    vec![check.id.clone()],
                ));
            }
            if let Err(error) = verify_executables(
                command,
                check.executable.as_ref(),
                &probe.executable,
                tool,
                workspace,
            )
            .await
            {
                complete = false;
                report.diagnostics.push(diagnostic(
                    "probe.executable_changed",
                    &error,
                    vec![check.id.clone()],
                ));
            }
            probe.complete = complete;
        }
    }
    for check in &mut report.checks {
        let command = plan
            .commands
            .iter()
            .find(|command| command.id == check.id)
            .expect("selected command");
        for probe in &mut check.probes {
            let tool = command
                .tools
                .iter()
                .find(|tool| tool.id == probe.id)
                .expect("selected tool");
            if let Err(error) = verify_executables(
                command,
                check.executable.as_ref(),
                &probe.executable,
                tool,
                workspace,
            )
            .await
            {
                probe.complete = false;
                report.diagnostics.push(diagnostic(
                    "probe.executable_changed",
                    &error,
                    vec![check.id.clone()],
                ));
            }
        }
    }
    Ok(())
}

async fn verify_executables(
    command: &CommandCheck,
    original: Option<&crate::domain::Artifact>,
    tool_identity: &Option<crate::domain::Artifact>,
    tool: &crate::config::ToolVersion,
    workspace: &Path,
) -> Result<()> {
    let root = workspace.to_owned();
    let relative = command.cwd.clone();
    let cwd =
        tokio::task::spawn_blocking(move || paths::confined(&root, Path::new(&relative))).await??;
    for (argv, original) in [
        (&command.argv, original),
        (&tool.argv, tool_identity.as_ref()),
    ] {
        let original = original.context("Executable identity is missing")?;
        let current = runner::identity::executable(&argv[0], &cwd).await?;
        ensure!(
            current.path == original.path && current.digest == original.digest,
            "Executable changed during tool probing: {}",
            argv[0]
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exhausted_shared_budget_and_invalid_guard_never_start_another_probe() {
        let snapshot = Arc::new(snapshot::Snapshot {
            scope_evidence: Default::default(),
            root: "unused".into(),
            identity: snapshot::Identity {
                mode: "worktree".into(),
                base: "base".into(),
                head: "head".into(),
                content_digest: "digest".into(),
                verification_digest: None,
                merge_request: None,
            },
            files: [(
                "input".into(),
                snapshot::File {
                    bytes: b"original".to_vec(),
                    executable: false,
                },
            )]
            .into(),
            base_files: Default::default(),
            changes: Default::default(),
            path_filter: None,
            commits: Vec::new(),
        });
        let config = crate::config::parse(b"schema_version: 1\nchecks: [{id: test, argv: [git, --version], tools: [{id: git, argv: [git, --version]}]}]\n").unwrap();
        let plan = Plan::build(&config, None, "full").unwrap();
        let workspace = snapshot::materialize_shared(snapshot.clone())
            .await
            .unwrap();
        let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
            .await
            .unwrap();
        let mut report = Report::new("full".into(), true);
        inspect(&plan, workspace.path(), &snapshot, &mut report).await;
        assert!(report.diagnostics.is_empty());
        probe_until(
            &plan,
            workspace.path(),
            &snapshot,
            &guard,
            &mut report,
            Instant::now(),
        )
        .await
        .unwrap();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|error| error.code == "probe.budget_exhausted")
        );
        assert!(!report.checks[0].probes[0].attempted);
        report.diagnostics.clear();
        std::fs::write(workspace.path().join("input"), "changed").unwrap();
        probe_until(
            &plan,
            workspace.path(),
            &snapshot,
            &guard,
            &mut report,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|error| error.code == "snapshot.inputs_changed")
        );
        assert!(!report.checks[0].probes[0].attempted);
        let command = &plan.commands[0];
        assert!(
            verify_executables(command, None, &None, &command.tools[0], workspace.path())
                .await
                .is_err()
        );
    }
}
