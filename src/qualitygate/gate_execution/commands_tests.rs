use super::*;
use serde_json::json;

fn check(argv: &[&str]) -> CommandCheck {
    serde_json::from_value(
        json!({"id":"unit-command", "kind":"command", "argv":argv, "timeout_seconds":1}),
    )
    .unwrap()
}

async fn run(check: &CommandCheck, deadline: Option<std::time::Instant>) -> CheckResult {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(crate::application::test_support::snapshot(workspace.path()));
    let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
        .await
        .unwrap();
    let result = execute_until(
        check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &guard,
        deadline,
    )
    .await;
    for artifact in &result.execution.artifacts {
        let bytes = std::fs::read(&artifact.path).unwrap();
        assert_eq!(artifact.bytes, bytes.len() as u64);
        assert_eq!(artifact.digest, snapshot::digest(&bytes));
    }
    result
}

#[tokio::test]
async fn successful_process_and_nonzero_exit_have_distinct_verdicts_with_retained_logs() {
    for (argv, verdict) in [
        (vec!["git", "--version"], Verdict::Pass),
        (vec!["git", "--qualitygate-invalid-option"], Verdict::Fail),
    ] {
        let result = run(&check(&argv), None).await;
        assert_eq!(result.execution.status, ExecutionStatus::Completed);
        assert_eq!(result.verdict, Some(verdict));
        assert_eq!(result.execution.artifacts.len(), 2);
        assert_eq!(result.metadata["input_integrity"]["after"], "verified");
        assert_eq!(result.diagnostics.is_empty(), verdict == Verdict::Pass);
        assert!(
            result.metadata["command_executable"]["digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }
}

#[tokio::test]
async fn missing_prerequisites_block_execution_instead_of_becoming_violations() {
    let mut missing_argument = check(&["git", "--version"]);
    missing_argument.required_args = vec!["--required".into()];
    let result = run(&missing_argument, None).await;
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert!(result.execution.started_at_ms.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert!(
        result
            .execution
            .reason
            .unwrap()
            .contains("required arguments")
    );

    let mut missing_env = check(&["git", "--version"]);
    // A process environment key cannot contain '=' on any supported platform.
    missing_env.required_env = vec!["QUALITYGATE_UNIT=ABSENT".into()];
    let result = run(&missing_env, None).await;
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert_eq!(
        result.metadata["required_env_missing"],
        json!(["QUALITYGATE_UNIT=ABSENT"])
    );

    let mut escaped = check(&["git", "--version"]);
    escaped.cwd = "../outside".into();
    assert_eq!(
        run(&escaped, None).await.execution.status,
        ExecutionStatus::Blocked
    );
    let result = run(&check(&["qualitygate-unit-missing-tool-493bf"]), None).await;
    assert_eq!(result.execution.status, ExecutionStatus::ToolError);
    assert!(result.execution.started_at_ms.is_none());

    let mut report = check(&["git", "--version"]);
    report.reports = vec![
        serde_json::from_value(json!({"path":"report.json", "format":"diagnostics"})).unwrap(),
    ];
    let result = run(&report, None).await;
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert!(result.execution.reason.unwrap().contains("version probes"));
}

#[tokio::test]
async fn expired_shared_deadline_and_modified_inputs_prevent_command_start() {
    let result = run(
        &check(&["git", "--version"]),
        Some(std::time::Instant::now()),
    )
    .await;
    assert_eq!(result.execution.status, ExecutionStatus::TimedOut);
    assert!(result.execution.started_at_ms.is_none());
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(workspace.path());
    snapshot.files.insert(
        "source.rs".into(),
        snapshot::File {
            bytes: b"original".to_vec(),
            executable: false,
        },
    );
    std::fs::write(workspace.path().join("source.rs"), b"original").unwrap();
    let snapshot = Arc::new(snapshot);
    let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
        .await
        .unwrap();
    let path = workspace.path().join("source.rs");
    let write = std::fs::write(&path, b"changed");
    #[cfg(windows)]
    {
        assert_eq!(write.unwrap_err().raw_os_error(), Some(32));
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        guard.verify().await.unwrap();
    }
    #[cfg(not(windows))]
    write.unwrap();
    #[cfg(windows)]
    let permissions = {
        // Windows guards deny data writes but still detect input attribute changes.
        let permissions = std::fs::metadata(&path).unwrap().permissions();
        let mut changed = permissions.clone();
        changed.set_readonly(true);
        std::fs::set_permissions(&path, changed).unwrap();
        permissions
    };
    let result = execute(
        &check(&["git", "--version"]),
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &guard,
    )
    .await;
    #[cfg(windows)]
    std::fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert_eq!(result.metadata["input_integrity"]["before"], "invalid");
    assert!(result.execution.started_at_ms.is_none());
    assert!(guard.verify().await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_and_signal_termination_are_execution_gaps_not_successful_empty_runs() {
    for (script, status) in [
        ("sleep 5", ExecutionStatus::TimedOut),
        ("kill -TERM $$", ExecutionStatus::ToolError),
    ] {
        let result = run(&check(&["sh", "-c", script]), None).await;
        assert_eq!(result.execution.status, status);
        assert_eq!(result.verdict, None);
        assert_eq!(result.execution.artifacts.len(), 2);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn report_exit_codes_require_matching_findings_and_parseable_evidence() {
    for (body, code, expected_status, expected_verdict) in [
        (
            r#"{"issues":[]}"#,
            0,
            ExecutionStatus::Completed,
            Some(Verdict::Pass),
        ),
        (r#"{"issues":[]}"#, 3, ExecutionStatus::ToolError, None),
        (r#"{"issues":[]}"#, 7, ExecutionStatus::ToolError, None),
        ("not-json", 0, ExecutionStatus::ToolError, None),
        (
            r#"{"issues":[{"rule":"lint","message":"violation"}]}"#,
            3,
            ExecutionStatus::Completed,
            Some(Verdict::Fail),
        ),
    ] {
        let script = format!("printf '%s' '{body}'; exit {code}");
        let mut check = check(&["sh", "-c", &script]);
        check.findings_exit_codes = vec![3];
        check.tools =
            serde_json::from_value(json!([{"id":"git", "argv":["git", "--version"]}])).unwrap();
        check.reports = serde_json::from_value(json!([{"path":"report.json", "format":"diagnostics", "from_stdout":true, "mode":"full"}])).unwrap();
        let result = run(&check, None).await;
        assert_eq!(
            result.execution.status, expected_status,
            "{body}, {code}: {:?}",
            result.execution.reason
        );
        assert_eq!(result.verdict, expected_verdict, "{body}, {code}");
        assert!(result.execution.artifacts.len() >= 3);
    }
}

#[tokio::test]
async fn failed_probe_and_missing_artifact_directory_preserve_execution_gap_reason() {
    let mut failed = check(&["git", "--version"]);
    failed.tools = serde_json::from_value(
        json!([{"id":"missing", "argv":["qualitygate-unit-missing-tool-493bf"]}]),
    )
    .unwrap();
    assert_eq!(
        run(&failed, None).await.execution.status,
        ExecutionStatus::ToolError
    );
    let workspace = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(crate::application::test_support::snapshot(workspace.path()));
    let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
        .await
        .unwrap();
    let result = execute(
        &check(&["git", "--version"]),
        workspace.path(),
        &workspace.path().join("absent"),
        &snapshot,
        &guard,
    )
    .await;
    assert_eq!(result.execution.status, ExecutionStatus::ToolError);
    assert!(
        result
            .execution
            .reason
            .unwrap()
            .contains("Cannot persist process evidence")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_discovery_is_required_even_when_runner_exits_successfully() {
    use std::os::unix::fs::PermissionsExt;
    let tools = tempfile::tempdir().unwrap();
    for (name, output, expected) in [
        (
            "cargo",
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s",
            Some(Verdict::Pass),
        ),
        (
            "cargo",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s",
            Some(Verdict::Fail),
        ),
        ("cargo", "no discovery evidence", None),
        ("pytest", "2 tests passed", None),
    ] {
        let path = tools.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{output}'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let result = run(&check(&[path.to_str().unwrap(), "test"]), None).await;
        assert_eq!(result.verdict, expected, "{output}");
        if expected.is_none() {
            assert_eq!(result.execution.status, ExecutionStatus::ToolError);
        } else {
            assert_eq!(result.execution.status, ExecutionStatus::Completed);
            assert!(result.metadata["tests"]["executed"].is_number());
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn mutating_probe_or_command_invalidates_evidence_and_preserves_prior_outcome() {
    for during_probe in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let artifacts = tempfile::tempdir().unwrap();
        let mut snapshot = crate::application::test_support::snapshot(workspace.path());
        snapshot.files.insert(
            "Cargo.toml".into(),
            snapshot::File {
                bytes: b"original".to_vec(),
                executable: false,
            },
        );
        std::fs::write(workspace.path().join("Cargo.toml"), "original").unwrap();
        let snapshot = Arc::new(snapshot);
        let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
            .await
            .unwrap();
        let mutate = "printf changed > Cargo.toml; printf version; exit 0";
        let mut selected = check(&["sh", "-c", if during_probe { "exit 0" } else { mutate }]);
        if during_probe {
            selected.tools = vec![
                serde_json::from_value(json!({"id":"mutating-tool","argv":["sh","-c",mutate]}))
                    .unwrap(),
            ];
        }
        let result = execute(
            &selected,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &guard,
        )
        .await;
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
        assert_eq!(result.metadata["input_integrity"]["after"], "invalid");
        assert_eq!(
            result.metadata["environment"]["dependency_inputs"]["Cargo.toml"],
            snapshot::digest(b"original")
        );
        if during_probe {
            assert!(result.execution.started_at_ms.is_none());
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn executable_changes_during_probe_or_execution_invalidate_even_successful_exit() {
    use std::os::unix::fs::PermissionsExt;
    for during_probe in [false, true] {
        let tools = tempfile::tempdir().unwrap();
        let command = tools.path().join("producer");
        let script = if during_probe {
            "#!/bin/sh\nprintf output\n"
        } else {
            "#!/bin/sh\nprintf '\\n# changed executable\\n' >> \"$0\"; printf output\n"
        };
        std::fs::write(&command, script).unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut selected = check(&[command.to_str().unwrap()]);
        if during_probe {
            selected.tools=vec![serde_json::from_value(json!({"id":"version","argv":["sh","-c","printf '\\n# changed executable\\n' >> \"$1\"; printf version","probe",command]})).unwrap()];
        }
        let result = run(&selected, None).await;
        assert_eq!(result.execution.status, ExecutionStatus::ToolError);
        assert!(result.execution.reason.unwrap().contains(if during_probe {
            "changed during version probing"
        } else {
            "changed during execution"
        }));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn oversized_output_and_failed_project_producers_cannot_supply_successful_evidence() {
    let argv = format!("head -c {} /dev/zero", crate::runner::MAX_OUTPUT_BYTES + 1);
    let result = run(&check(&["sh", "-c", &argv]), None).await;
    assert_eq!(result.execution.status, ExecutionStatus::ToolError);
    assert!(!result.execution.artifacts.is_empty());
    for script in ["exit 1", "exit 0"] {
        let mut selected = check(&["sh", "-c", script]);
        selected.tools = vec![
            serde_json::from_value(json!({"id":"tool","argv":["sh","-c","printf version"]}))
                .unwrap(),
        ];
        selected.projects = vec![
            serde_json::from_value(
                json!({"root":".","effective_pom":"effective.xml","dependency_tree":"tree.json"}),
            )
            .unwrap(),
        ];
        let result = run(&selected, None).await;
        assert_eq!(result.execution.status, ExecutionStatus::ToolError);
        assert!(!result.execution.artifacts.is_empty());
    }
}
