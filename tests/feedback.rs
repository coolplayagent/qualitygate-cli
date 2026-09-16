mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nrules: {line-ending: {enabled: true}}\nprofiles:\n  quick: {include: [line-ending]}\n").unwrap();
    std::fs::write(root.path().join("task.yaml"), "schema_version: 1\ntask_id: behavior\nacceptance:\n  - id: behavior\n    description: The configured behavior producer succeeds.\n    verification: {kind: command, check_id: behavior, argv: [git, --version]}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "task policy"]);
    git(root.path(), &["branch", "selected-policy"]);
    root
}

fn full(feedback: &Value) -> Value {
    let artifact = &feedback["full_report"];
    let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
    assert_eq!(bytes.len() as u64, artifact["bytes"]);
    assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["gate"]["decision"], feedback["gate"]["decision"]);
    assert_eq!(report["gate"]["complete"], feedback["gate"]["complete"]);
    assert_eq!(
        report["snapshot"]["content_digest"],
        feedback["snapshot"]["content_digest"]
    );
    report
}

fn recheck(root: &Path, argv: &Value, code: i32) -> Value {
    let argv: Vec<String> = serde_json::from_value(argv.clone()).unwrap();
    assert_eq!(argv[1], "--root");
    assert_eq!(Path::new(&argv[2]), root);
    report(
        &cli(
            root,
            &argv[3..].iter().map(String::as_str).collect::<Vec<_>>(),
        ),
        code,
    )
}

#[test]
fn compact_feedback_retains_full_unfiltered_evidence_and_pinned_delivery_commands() {
    let root = setup();
    let path = root.path();
    std::fs::write(path.join("hello.txt"), "wrong\r\n").unwrap();
    let args = [
        "check",
        "--task",
        "task.yaml",
        "--policy-ref",
        "selected-policy",
        "--profile",
        "quick",
        "--feedback",
    ];
    let initial = report(&cli(path, &args), 1);
    let original = full(&initial);
    assert_eq!(initial["pending_delivery_checks"]["total"], 1);
    assert_eq!(initial["delivery_ready"], false);
    assert_eq!(initial["findings"]["total"], 1);
    let mut filtered = args.to_vec();
    filtered.extend(["--severity", "warning"]);
    let warning = report(&cli(path, &filtered), 1);
    assert_eq!(warning["findings"]["filtered"], 1);
    assert_eq!(full(&warning)["summary"]["diagnostics_total"], 1);
    git(path, &["add", "hello.txt"]);
    git(path, &["commit", "-qm", "bad input"]);
    git(path, &["branch", "-f", "selected-policy", "HEAD"]);
    assert_eq!(
        recheck(path, &initial["recheck"]["argv"], 1)["policy"]["resolved_commit"],
        original["policy"]["resolved_commit"]
    );
    std::fs::write(path.join("hello.txt"), "repaired\n").unwrap();
    let repaired = recheck(path, &initial["delivery_recheck"]["argv"], 0);
    assert_eq!(repaired["profile"], "full");
    assert_eq!(repaired["plan"]["task_id"], "behavior");
    assert!(
        repaired["plan"]["pending_delivery_checks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_ne!(
        repaired["snapshot"]["content_digest"],
        original["snapshot"]["content_digest"]
    );
    std::fs::write(path.join("hello.txt"), "regression\r\n").unwrap();
    recheck(path, &initial["delivery_recheck"]["argv"], 1);
    // Deleting old evidence never changes the archived snapshot or grants acceptance.
    std::fs::remove_file(initial["full_report"]["path"].as_str().unwrap()).unwrap();
    assert!(
        initial["evidence_availability"]
            .as_str()
            .unwrap()
            .starts_with("not_rechecked")
    );
}

#[test]
fn path_and_policy_changes_preserve_scope_and_cannot_remove_acceptance() {
    let root = setup();
    let path = root.path();
    for selection in [
        vec!["--path", "hello.txt"],
        vec!["--worktree", "--path", "hello.txt"],
    ] {
        let mut args = vec![
            "check",
            "--task",
            "task.yaml",
            "--policy-ref",
            "selected-policy",
            "--feedback",
        ];
        args.extend(selection);
        let output = report(&cli(path, &args), 0);
        full(&output);
        assert_eq!(output["scope"], "path");
        assert_eq!(output["delivery_ready"], false);
        let complete = recheck(path, &output["delivery_recheck"]["argv"], 0);
        assert_eq!(complete["scope"], "task");
        assert_eq!(
            complete["context"]["delivery_recheck"],
            complete["context"]["recheck"]
        );
    }
    std::fs::write(path.join("task.yaml"), "invalid task").unwrap();
    let output = report(
        &cli(
            path,
            &[
                "check",
                "--task",
                "task.yaml",
                "--policy-ref",
                "selected-policy",
                "--feedback",
            ],
        ),
        2,
    );
    assert_eq!(output["policy"]["changes"], 1);
    assert_eq!(output["execution_gaps"]["total"], 1);
    assert_eq!(full(&output)["plan"]["acceptance"]["behavior"], "behavior");
    assert_eq!(output["gate"]["decision"], "incomplete");
}

#[cfg(unix)]
#[test]
fn simultaneous_findings_and_timeout_remain_visible_with_a_bounded_response() {
    let root = fixture();
    let path = root.path();
    std::fs::write(path.join("qualitygate.yaml"),"schema_version: 1\nrules: {line-ending: {enabled: true}}\nchecks:\n  - id: slow\n    argv: [git, -c, 'alias.slow=!sleep 5', slow]\n    timeout_seconds: 1\n").unwrap();
    std::fs::write(path.join("hello.txt"), "wrong\r\n").unwrap();
    let result = cli(
        path,
        &["check", "--feedback", "--feedback-max-bytes", "8192"],
    );
    assert!(result.stdout.len() <= 8192);
    let output = report(&result, 2);
    full(&output);
    assert_eq!(output["findings"]["total"], 1);
    assert_eq!(output["execution_gaps"]["items"][0]["status"], "timed_out");
    assert_eq!(
        output["execution_gaps"]["items"][0]["artifacts"]["count"],
        2
    );
}

#[test]
fn malformed_inputs_and_budget_errors_never_synthesize_feedback_success() {
    let root = fixture();
    let missing = report(&cli(root.path(), &["check", "--feedback"]), 2);
    assert_eq!(missing["gate"]["decision"], "incomplete");
    assert!(missing["run_id"].is_null());
    for args in [
        vec!["check", "--feedback-max-bytes", "4096"],
        vec!["check", "--feedback", "--feedback-max-bytes", "1"],
    ] {
        assert_eq!(cli(root.path(), &args).status.code(), Some(2));
    }
}

#[test]
fn staged_replays_reject_a_changed_implicit_base_before_running_checks() {
    let root = setup();
    let path = root.path();
    let original = report(
        &cli(
            path,
            &[
                "check",
                "--staged",
                "--task",
                "task.yaml",
                "--policy-ref",
                "selected-policy",
                "--feedback",
            ],
        ),
        0,
    );
    let replay = recheck(path, &original["delivery_recheck"]["argv"], 0);
    assert_eq!(replay["snapshot"]["base"], original["snapshot"]["base"]);
    git(path, &["commit", "--allow-empty", "-qm", "base moved"]);
    let refused = recheck(path, &original["delivery_recheck"]["argv"], 2);
    assert_eq!(refused["gate"]["decision"], "incomplete");
    assert!(refused.to_string().contains("expected base"));
    assert!(refused["run_id"].is_null());
}
