pub mod common;
// Exercise the same optional harness source without adding a second CLI binary.
#[path = "../examples/agent_loop.rs"]
mod harness;
use clap::Parser;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

async fn run(root: &Path, output: &Path, agent: Value, attempts: u8) -> Value {
    run_budget(root, output, agent, attempts, 30).await
}

async fn run_budget(root: &Path, output: &Path, agent: Value, attempts: u8, seconds: u64) -> Value {
    std::fs::write(output.join("agent.json"), agent.to_string()).unwrap();
    std::fs::write(
        output.join("prompt.txt"),
        "Repair the line ending violation.",
    )
    .unwrap();
    let options = harness::Options::try_parse_from([
        "agent_loop",
        "--root",
        root.to_str().unwrap(),
        "--qualitygate",
        env!("CARGO_BIN_EXE_qualitygate"),
        "--base",
        "HEAD",
        "--policy-ref",
        "HEAD",
        "--task",
        "task.yaml",
        "--prompt-file",
        output.join("prompt.txt").to_str().unwrap(),
        "--agent-command",
        output.join("agent.json").to_str().unwrap(),
        "--output-dir",
        output.to_str().unwrap(),
        "--max-attempts",
        &attempts.to_string(),
        "--timeout-seconds",
        &seconds.to_string(),
    ])
    .unwrap();
    let summary = harness::run(options).await.unwrap();
    serde_json::from_slice(
        &std::fs::read(Path::new(summary["evidence"].as_str().unwrap()).join("index.json"))
            .unwrap(),
    )
    .unwrap()
}

fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules: {line-ending: {enabled: true}}\n",
    )
    .unwrap();
    std::fs::write(root.path().join("task.yaml"), "schema_version: 1\ntask_id: lines\nacceptance:\n  - id: lines\n    description: The external command producer runs alongside the line-ending gate.\n    verification: {kind: command, check_id: producer, argv: [git, --version]}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "fixed policy"]);
    std::fs::write(root.path().join("hello.txt"), "wrong\r\n").unwrap();
    root
}

#[tokio::test]
async fn harness_accepts_only_changed_full_snapshot_and_preserves_untracked_edits() {
    let root = setup();
    let evidence = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("new.txt"), "untracked evidence\n").unwrap();
    let result = run(
        root.path(),
        evidence.path(),
        json!(["git", "restore", "hello.txt"]),
        3,
    )
    .await;
    assert_eq!(result["termination"], "accepted", "{result}");
    assert_eq!(result["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(result["initial"]["gate"]["decision"], "fail");
    assert_eq!(result["attempts"][0]["recheck"]["gate"]["decision"], "pass");
    assert_eq!(result["initial_changes"]["untracked"][0]["path"], "new.txt");
    assert_eq!(
        result["attempts"][0]["changes"]["untracked"][0]["path"],
        "new.txt"
    );
    assert_eq!(result["complete"], true);
}

#[tokio::test]
async fn harness_retains_failed_attempts_and_stops_at_progress_or_attempt_limits() {
    for (attempts, agent, stop, count) in [
        (3, json!(["git", "--version"]), "no_progress", 2),
        (1, json!(["git", "--version"]), "attempt_budget", 1),
        (3, json!(["git", "not-a-real-command"]), "agent_failed", 1),
    ] {
        let root = setup();
        let evidence = tempfile::tempdir().unwrap();
        let result = run(root.path(), evidence.path(), agent, attempts).await;
        assert_eq!(result["termination"], stop, "{result}");
        assert_eq!(result["attempts"].as_array().unwrap().len(), count);
        assert_eq!(result["complete"], false);
        assert!(!result["attempts"][0]["agent"]["stdout"]["digest"].is_null());
    }
}

#[tokio::test]
async fn harness_does_not_retry_incomplete_or_changed_policy_inputs() {
    let root = setup();
    let evidence = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("task.yaml"), "invalid\n").unwrap();
    let result = run(root.path(), evidence.path(), json!(["git", "--version"]), 3).await;
    assert_eq!(result["termination"], "incomplete_check", "{result}");
    assert_eq!(result["attempts"].as_array().unwrap().len(), 0);
    assert_eq!(result["complete"], false);
}

#[tokio::test]
async fn harness_timeout_keeps_the_attempt_and_unfinished_status() {
    let root = setup();
    let evidence = tempfile::tempdir().unwrap();
    let result = run_budget(
        root.path(),
        evidence.path(),
        json!([
            env!("CARGO_BIN_EXE_qualitygate"),
            "selfcheck-probe",
            "timeout"
        ]),
        3,
        2,
    )
    .await;
    assert_eq!(result["termination"], "time_budget", "{result}");
    assert_eq!(result["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(result["attempts"][0]["agent"]["timed_out"], true);
    assert!(result["attempts"][0]["recheck"].is_null());
    assert_eq!(result["complete"], false);
}
