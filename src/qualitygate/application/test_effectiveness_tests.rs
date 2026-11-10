use super::*;

fn check(script: &str) -> CommandCheck {
    serde_json::from_value(json!({"id":"effectiveness","kind":"command","argv":["sh","-c",script],
        "timeout_seconds":2,"findings_exit_codes":[1],"tools":[{"id":"fixture","argv":["sh","-c","printf fixture-v1"]}],
        "reports":[{"path":"results.xml","format":"junit","mode":"full"}],
        "test_effectiveness":{"source_paths":["src/**"],"test_paths":["tests/**"],"assertion_failure_types":["AssertionError"]}})).unwrap()
}

fn snapshot(root: &Path, changed_source: bool, changed_test: bool) -> Arc<Snapshot> {
    let mut snapshot = super::super::test_support::snapshot(root);
    for (path, old, current) in [
        (
            "src/version.txt",
            "old",
            if changed_source { "new" } else { "old" },
        ),
        (
            "tests/behavior.txt",
            "original test",
            if changed_test {
                "new scenario"
            } else {
                "original test"
            },
        ),
    ] {
        snapshot.base_files.insert(
            path.into(),
            snapshot::File {
                bytes: old.as_bytes().into(),
                executable: false,
            },
        );
        snapshot.files.insert(
            path.into(),
            snapshot::File {
                bytes: current.as_bytes().into(),
                executable: false,
            },
        );
        if old != current {
            snapshot.changes.insert(
                path.into(),
                snapshot::Change {
                    kind: "modified".into(),
                    old_path: None,
                    added_lines: [1].into(),
                    removed_lines: [1].into(),
                },
            );
        }
    }
    Arc::new(snapshot)
}

async fn observe(check: &CommandCheck, snapshot: &Arc<Snapshot>, artifacts: &Path) -> CheckResult {
    execute(check, artifacts, snapshot, &[], 1024 * 1024, 1024 * 1024).await
}

#[tokio::test]
async fn unchanged_source_skips_but_missing_changed_tests_is_a_violation() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let command = check("exit 99");
    let skipped = observe(
        &command,
        &snapshot(root.path(), false, true),
        artifacts.path(),
    )
    .await;
    assert_eq!(skipped.verdict, Some(Verdict::Skipped));
    let missing = observe(
        &command,
        &snapshot(root.path(), true, false),
        artifacts.path(),
    )
    .await;
    assert_eq!(missing.execution.status, ExecutionStatus::Completed);
    assert_eq!(missing.verdict, Some(Verdict::Fail));
    assert!(
        missing.diagnostics[0]
            .message
            .contains("no added or content-modified")
    );
    assert!(missing.execution.artifacts.is_empty());
    let mut collision = (*snapshot(root.path(), true, true)).clone();
    collision.files.insert(
        "results.xml".into(),
        snapshot::File {
            bytes: b"input".to_vec(),
            executable: false,
        },
    );
    let blocked = observe(&command, &Arc::new(collision), artifacts.path()).await;
    assert_eq!(blocked.execution.status, ExecutionStatus::Blocked);
    assert!(blocked.execution.reason.unwrap().contains("overwrite"));
    let mut missing = command;
    missing.test_effectiveness = None;
    assert_eq!(
        observe(
            &missing,
            &snapshot(root.path(), true, true),
            artifacts.path()
        )
        .await
        .execution
        .status,
        ExecutionStatus::Blocked
    );
}

#[cfg(unix)]
#[tokio::test]
async fn old_code_counterexample_is_required_and_tool_failure_is_not_a_counterexample() {
    // A bounded fake test tool isolates orchestration from a particular language runner.
    let pass = "<testsuite tests=\"1\"><testcase name=\"changed_behavior\" file=\"tests/behavior.txt\"/></testsuite>";
    let fail = "<testsuite tests=\"1\" failures=\"1\"><testcase name=\"changed_behavior\" file=\"tests/behavior.txt\"><failure type=\"AssertionError\">behavior differs</failure></testcase></testsuite>";
    for case in 0..5 {
        let root = tempfile::tempdir().unwrap();
        let artifacts = tempfile::tempdir().unwrap();
        let script = match case {
            0 => format!(
                "if [ \"$(cat src/version.txt)\" = new ]; then printf '%s' '{pass}' > results.xml; else printf '%s' '{fail}' > results.xml; exit 1; fi"
            ),
            1 => format!("printf '%s' '{pass}' > results.xml"),
            2 => format!("printf '%s' '{fail}' > results.xml; exit 1"),
            3 => "exit 7".into(),
            4 => "sleep 4".into(),
            _ => unreachable!(),
        };
        let mut command = check(&script);
        command.timeout_seconds = 1;
        let result = observe(
            &command,
            &snapshot(root.path(), true, true),
            artifacts.path(),
        )
        .await;
        match case {
            0 => {
                assert_eq!(
                    result.verdict,
                    Some(Verdict::Pass),
                    "{:?}",
                    result.execution
                );
                assert_eq!(result.matched_entities, 1);
                assert_eq!(
                    result.metadata["test_effectiveness_files"][0]["counterexamples"],
                    1
                );
                assert!(result.metadata.contains_key("baseline_test_execution"));
            }
            1 | 2 => {
                assert_eq!(result.execution.status, ExecutionStatus::Completed);
                assert_eq!(result.verdict, Some(Verdict::Fail));
                let expected = if case == 1 {
                    "no assertion counterexample"
                } else {
                    "Tests fail on the current code"
                };
                assert!(
                    result
                        .diagnostics
                        .iter()
                        .any(|d| d.message.contains(expected))
                );
            }
            _ => assert_eq!(result.execution.status, ExecutionStatus::Blocked),
        }
        assert!(result.metadata.contains_key("current_test_execution"));
        assert!(!result.execution.artifacts.is_empty());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn retained_execution_accumulates_time_and_preserves_current_command_identity() {
    let mut result = CheckResult::pending("effectiveness", true, Severity::Error);
    let mut current = CheckResult::pending("current", true, Severity::Error);
    assert!(cases(&current).is_err());
    current.complete();
    assert!(cases(&current).is_err());
    current.metadata.insert("test_cases".into(), json!([]));
    assert!(cases(&current).unwrap().is_empty());
    current.execution.argv = vec!["current-command".into()];
    current.execution.duration_ms = Some(10);
    current.execution.started_at_ms = Some(100);
    retain(&mut result, "current", &Arc::new(current.clone()))
        .await
        .unwrap();
    current.execution.argv = vec!["baseline-command".into()];
    current.execution.duration_ms = Some(20);
    current.execution.ended_at_ms = Some(130);
    retain(&mut result, "baseline", &Arc::new(current))
        .await
        .unwrap();
    assert_eq!(result.execution.argv, ["current-command"]);
    assert_eq!(result.execution.duration_ms, Some(30));
    assert_eq!(result.execution.started_at_ms, Some(100));
    assert_eq!(result.execution.ended_at_ms, Some(130));
}
