use super::*;
use crate::domain::Severity;
use serde_json::json;

fn check(argv: &[&str]) -> CommandCheck {
    serde_json::from_value(
        json!({"id": "analyzer", "kind": "command", "argv": ["git", "--version"],
        "tools": [{"id": "git", "argv": argv, "timeout_seconds": 2, "inputs": ["tool.lock"]}]}),
    )
    .unwrap()
}

fn snapshot(root: &Path) -> Snapshot {
    let mut snapshot = crate::application::test_support::snapshot(root);
    snapshot.files.insert(
        "tool.lock".into(),
        crate::snapshot::File {
            bytes: b"pinned".to_vec(),
            executable: false,
        },
    );
    snapshot
}

#[tokio::test]
async fn successful_probe_retains_exact_output_identity_and_snapshot_inputs_on_each_side() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = snapshot(workspace.path());
    let check = check(&["git", "--version"]);
    for baseline in [false, true] {
        let mut result = CheckResult::pending("analyzer", true, Severity::Error);
        let evidence = collect(
            &check,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &mut result,
            baseline,
        )
        .await
        .unwrap();
        assert_eq!(evidence.len(), 1);
        let tool = &evidence[0];
        assert!(tool.version.starts_with("git version "));
        assert_eq!(tool.inputs["tool.lock"], crate::snapshot::digest(b"pinned"));
        assert_eq!(tool.snapshot_digest, snapshot.identity.content_digest);
        assert_eq!(tool.exit_code, Some(0));
        assert!(!tool.timed_out);
        assert_eq!(tool.resolved_argv[0], tool.executable.path);
        assert!(Path::new(&tool.executable.path).is_absolute());
        let bytes = std::fs::read(&tool.stdout.path).unwrap();
        assert_eq!(tool.stdout.digest, crate::snapshot::digest(&bytes));
        assert_eq!(tool.stdout.bytes, bytes.len() as u64);
        assert_eq!(String::from_utf8(bytes).unwrap().trim(), tool.version);
        assert_eq!(
            result.metadata[if baseline { "baseline_tools" } else { "tools" }][0]["id"],
            "git"
        );
        verify(&check, workspace.path(), &evidence).await.unwrap();
        comparable(&serde_json::to_value(&evidence).unwrap(), &evidence).unwrap();
    }
}

#[tokio::test]
async fn missing_input_exhausted_budget_and_failed_probe_never_produce_successful_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut snapshot = snapshot(workspace.path());
    let mut result = CheckResult::pending("analyzer", true, Severity::Error);
    let check = check(&["git", "--version"]);
    snapshot.files.clear();
    let error = collect(
        &check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
        false,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("Tool input missing"));
    assert!(result.metadata.is_empty());
    snapshot.files.insert(
        "tool.lock".into(),
        crate::snapshot::File {
            bytes: vec![],
            executable: false,
        },
    );
    let error = collect_until(
        &check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
        false,
        Some(std::time::Instant::now()),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ProbeError>().unwrap().code,
        "probe.budget_exhausted"
    );
    let check = self::check(&["git", "--qualitygate-invalid-option"]);
    let error = collect(
        &check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ProbeError>().unwrap().code,
        "probe.exit_code"
    );
    let evidence: Vec<ToolEvidence> =
        serde_json::from_value(result.metadata["tools"].clone()).unwrap();
    assert_ne!(evidence[0].exit_code, Some(0));
    assert!(!std::fs::read(&evidence[0].stderr.path).unwrap().is_empty());
}

#[tokio::test]
async fn ratchet_comparison_rejects_analyzer_or_input_changes_but_allows_new_run_locations() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = snapshot(workspace.path());
    let check = check(&["git", "--version"]);
    let mut result = CheckResult::pending("analyzer", true, Severity::Error);
    let evidence = collect(
        &check,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
        false,
    )
    .await
    .unwrap();
    let value = serde_json::to_value(&evidence).unwrap();
    for (key, replacement) in [
        ("id", json!("other")),
        ("version", json!("other-version")),
        (
            "executable",
            json!({"path": "/other", "digest": "changed", "bytes": 1}),
        ),
        ("inputs", json!({"tool.lock": "changed"})),
    ] {
        let mut changed = value.clone();
        changed[0][key] = replacement;
        assert!(
            comparable(&changed, &evidence)
                .unwrap_err()
                .to_string()
                .contains("versions or tool inputs differ")
        );
    }
    assert!(comparable(&json!([]), &evidence).is_err());
    assert!(comparable(&json!({}), &evidence).is_err());
    let mut relocated = evidence.clone();
    relocated[0].cwd = "other-workspace".into();
    relocated[0].started_at_ms += 1;
    relocated[0].snapshot_digest = "new snapshot".into();
    comparable(&serde_json::to_value(relocated).unwrap(), &evidence).unwrap();
    let mut changed = evidence;
    changed[0].executable.digest = "changed".into();
    assert!(
        verify(&check, workspace.path(), &changed)
            .await
            .unwrap_err()
            .to_string()
            .contains("changed during command execution")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn empty_binary_oversized_and_timed_out_version_responses_keep_raw_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = snapshot(workspace.path());
    for (script, expected) in [
        ("exit 0", "probe.version_invalid"),
        ("printf '\\377'", "probe.version_invalid"),
        ("printf 'v\\0001'", "probe.version_invalid"),
        ("head -c 65537 /dev/zero", "probe.output_limit"),
        ("sleep 5", "probe.timeout"),
    ] {
        let mut check = check(&["sh", "-c", script]);
        check.tools[0].timeout_seconds = 1;
        let mut result = CheckResult::pending("analyzer", true, Severity::Error);
        let error = collect(
            &check,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &mut result,
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ProbeError>().unwrap().code,
            expected,
            "{script}"
        );
        assert!(
            Path::new(
                result.metadata["tools"][0]["stdout"]["path"]
                    .as_str()
                    .unwrap()
            )
            .is_file()
        );
    }
}
