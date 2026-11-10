use super::*;
use crate::domain::{Severity, Verdict};
use serde_json::json;

fn check(stdout: bool) -> CommandCheck {
    serde_json::from_value(json!({"id":"analysis", "kind":"command", "argv":["git", "--version"],
        "reports":[{"path":"report.json", "format":"diagnostics", "mode":"full", "from_stdout":stdout}]})).unwrap()
}

#[tokio::test]
async fn preparation_checks_whole_inventory_before_deleting_any_stale_output() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("report.json"), b"old evidence").unwrap();
    let mut check = check(false);
    check.reports.push(
        serde_json::from_value(json!({"path":"./source.rs", "format":"diagnostics"})).unwrap(),
    );
    let files = [(
        "source.rs".into(),
        snapshot::File {
            bytes: b"source".to_vec(),
            executable: false,
        },
    )]
    .into();
    let error = prepare(&check, root.path(), &files, false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("overwrite a checked input"));
    assert_eq!(
        std::fs::read(root.path().join("report.json")).unwrap(),
        b"old evidence"
    );
    check.reports.pop();
    prepare(&check, root.path(), &files, false).await.unwrap();
    assert!(!root.path().join("report.json").exists());
    prepare(&check, root.path(), &files, false).await.unwrap();
    std::fs::create_dir(root.path().join("report.json")).unwrap();
    assert!(
        prepare(&check, root.path(), &files, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("Cannot clear stale report")
    );
}

#[tokio::test]
async fn report_reads_require_bounded_regular_confined_files_and_bound_stdout() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        read_report(root.path(), "missing.json")
            .await
            .unwrap_err()
            .to_string()
            .contains("not produced")
    );
    std::fs::create_dir(root.path().join("directory")).unwrap();
    assert!(
        read_report(root.path(), "directory")
            .await
            .unwrap_err()
            .to_string()
            .contains("regular file")
    );
    let file = std::fs::File::create(root.path().join("large.json")).unwrap();
    file.set_len(snapshot::MAX_FILE_BYTES as u64 + 1).unwrap();
    assert!(read_report(root.path(), "large.json").await.is_err());
    assert!(read_report(root.path(), "../outside.json").await.is_err());
    std::fs::write(root.path().join("report.json"), b"complete").unwrap();
    assert_eq!(
        read_report(root.path(), "report.json").await.unwrap(),
        b"complete"
    );
    let at_limit = vec![b'x'; snapshot::MAX_FILE_BYTES];
    assert_eq!(bounded_stdout(&at_limit).unwrap().len(), at_limit.len());
    assert!(bounded_stdout(&vec![b'x'; snapshot::MAX_FILE_BYTES + 1]).is_err());
}

#[tokio::test]
async fn collection_preserves_invalid_raw_bytes_and_does_not_substitute_an_empty_baseline() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(super::super::test_support::snapshot(workspace.path()));
    let mut result = CheckResult::pending("analysis", true, Severity::Error);
    let check = check(true);
    let malformed = b"not a report";
    assert!(
        collect(
            &check,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &mut result,
            malformed
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read(&result.execution.artifacts[0].path).unwrap(),
        malformed
    );
    assert_eq!(result.verdict, None);
    let mut baseline = Err(anyhow::anyhow!("baseline timed out"));
    let retained = collect_current(
        &check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
        b"{\"issues\":[]}",
        &mut baseline,
    )
    .await
    .unwrap();
    assert!(!retained);
    assert!(baseline.is_err());
    assert_eq!(result.verdict, None);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.execution.artifacts.len(), 2);
}

#[tokio::test]
async fn current_file_and_stdout_reports_produce_findings_without_overwriting_gate_authority() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(super::super::test_support::snapshot(workspace.path()));
    for stdout in [true, false] {
        for (body, expected) in [
            (r#"{"issues":[]}"#, false),
            (
                r#"{"issues":[{"rule":"lint","message":"bad behavior"}]}"#,
                true,
            ),
        ] {
            let check = check(stdout);
            std::fs::write(workspace.path().join("report.json"), body).unwrap();
            let mut result = CheckResult::pending("analysis", true, Severity::Error);
            let found = collect(
                &check,
                workspace.path(),
                artifacts.path(),
                &snapshot,
                &mut result,
                body.as_bytes(),
            )
            .await
            .unwrap();
            assert_eq!(found, expected);
            assert_eq!(result.diagnostics.len(), usize::from(expected));
            assert_eq!(result.execution.artifacts.len(), 1);
            assert_eq!(
                result.execution.artifacts[0].digest,
                snapshot::digest(body.as_bytes())
            );
            assert_eq!(result.verdict, None);
            result.complete();
            assert_eq!(
                result.verdict,
                Some(if expected {
                    Verdict::Fail
                } else {
                    Verdict::Pass
                })
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn independent_baseline_retains_version_identity_and_rejects_unexpected_exit_or_missing_findings()
 {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(super::super::test_support::snapshot(workspace.path()));
    for (script, expected_error) in [
        ("printf '{\"issues\":[]}'", None),
        ("printf '{\"issues\":[]}'; exit 3", Some("contradicted")),
        (
            "printf '{\"issues\":[]}'; exit 7",
            Some("unexpected exit code"),
        ),
        ("sleep 5", Some("did not complete")),
    ] {
        let mut check = check(true);
        check.argv = vec!["sh".into(), "-c".into(), script.into()];
        check.timeout_seconds = 1;
        check.findings_exit_codes = vec![3];
        check.reports[0].mode = crate::config::IncrementMode::Ratchet;
        let mut result = CheckResult::pending("analysis", true, Severity::Error);
        result.metadata.insert("tools".into(), json!([]));
        let executable = runner::identity::executable("sh", workspace.path())
            .await
            .unwrap();
        result
            .metadata
            .insert("command_executable".into(), json!(executable));
        let outcome = baseline(&check, artifacts.path(), &snapshot, &mut result).await;
        if let Some(message) = expected_error {
            assert!(
                outcome.unwrap_err().to_string().contains(message),
                "{script}"
            );
        } else {
            assert!(outcome.unwrap()[&0].issues.is_empty());
        }
        assert!(result.execution.artifacts.len() >= 2);
        assert_eq!(
            result.metadata["baseline_input_integrity"]["status"],
            "verified"
        );
    }
}

#[tokio::test]
async fn python_preparation_requires_fresh_installation_and_creates_report_parent() {
    let root = tempfile::tempdir().unwrap();
    let check:CommandCheck=serde_json::from_value(json!({"id":"python","argv":["python"],"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"installed","install_report":"reports/pip.json"}]})).unwrap();
    prepare(&check, root.path(), &BTreeMap::new(), false)
        .await
        .unwrap();
    assert!(root.path().join("reports").is_dir());
    std::fs::create_dir(root.path().join("installed")).unwrap();
    std::fs::write(root.path().join("installed/retained"), b"prior install").unwrap();
    assert!(
        prepare(&check, root.path(), &BTreeMap::new(), false)
            .await
            .unwrap_err()
            .to_string()
            .contains("fresh and absent")
    );
    assert_eq!(
        std::fs::read(root.path().join("installed/retained")).unwrap(),
        b"prior install"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn baseline_failures_preserve_current_collection_errors_and_checked_input_mutations() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut input = super::super::test_support::snapshot(root.path());
    input.base_files.insert(
        "source.rs".into(),
        snapshot::File {
            bytes: b"original".to_vec(),
            executable: false,
        },
    );
    let input = Arc::new(input);
    let mut check = check(false);
    check.reports[0].mode = crate::config::IncrementMode::Ratchet;
    check.argv = vec![
        "sh".into(),
        "-c".into(),
        "printf changed > source.rs".into(),
    ];
    let mut result = CheckResult::pending("analysis", true, Severity::Error);
    result.metadata.insert("tools".into(), json!([]));
    result.metadata.insert(
        "command_executable".into(),
        json!(
            runner::identity::executable("sh", root.path())
                .await
                .unwrap()
        ),
    );
    let error = collect(
        &check,
        root.path(),
        artifacts.path(),
        &input,
        &mut result,
        b"",
    )
    .await
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(
        message.contains("Current report collection also failed"),
        "{message}"
    );
    assert!(message.contains("changed its checked inputs"), "{message}");
    assert_eq!(
        result.metadata["baseline_input_integrity"]["status"],
        "invalid"
    );
    assert_eq!(result.verdict, None);
    assert!(result.execution.artifacts.len() >= 2);
}

#[cfg(unix)]
#[tokio::test]
async fn baseline_file_reports_normalize_workspace_locations_before_ratchet_comparison() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let input = Arc::new(super::super::test_support::snapshot(root.path()));
    let mut check = check(false);
    check.reports[0].mode = crate::config::IncrementMode::Ratchet;
    check.reports[0].baseline = Some("before.json".into());
    check.argv=vec!["sh".into(),"-c".into(),r#"printf '{"issues":[{"rule":"lint","message":"existing finding","file":"%s/source.rs","locations":[{"file":"%s/source.rs"},{"file":"%s/related.rs"}]}]}' "$PWD" "$PWD" "$PWD" > before.json"#.into()];
    let mut result = CheckResult::pending("analysis", true, Severity::Error);
    result.metadata.insert("tools".into(), json!([]));
    result.metadata.insert(
        "command_executable".into(),
        json!(
            runner::identity::executable("sh", root.path())
                .await
                .unwrap()
        ),
    );
    let reports = baseline(&check, artifacts.path(), &input, &mut result)
        .await
        .unwrap();
    assert_eq!(reports[&0].issues[0].file.as_deref(), Some("source.rs"));
    assert_eq!(
        reports[&0].issues[0].locations[1].file.as_deref(),
        Some("related.rs")
    );
    assert!(
        result
            .execution
            .artifacts
            .iter()
            .any(|a| a.path.ends_with("baseline-report-0"))
    );
}
