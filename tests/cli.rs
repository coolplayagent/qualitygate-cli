use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.name", "Fixture"]);
    git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(root.path().join("hello.txt"), "initial\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "initial"]);
    root
}

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env(
            "QUALITYGATE_HOME",
            root.join(".git/qualitygate-test-evidence"),
        )
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn report(output: &Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn init_check_repair_and_staged_bytes_form_a_real_cli_loop() {
    let root = fixture();
    report(&cli(root.path(), &["init", "--format", "json"]), 0);
    std::fs::write(root.path().join("hello.txt"), "bad\r\n").unwrap();
    let failed = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    assert_eq!(failed["gate"]["decision"], "fail");
    assert!(failed["checks"][0]["diagnostics"][0]["recheck"]["argv"].is_array());
    git(root.path(), &["add", "."]);
    std::fs::write(root.path().join("hello.txt"), "fixed\n").unwrap();
    report(
        &cli(root.path(), &["check", "--staged", "--format", "json"]),
        1,
    );
    let passed = report(
        &cli(root.path(), &["check", "--worktree", "--format", "json"]),
        0,
    );
    assert_eq!(passed["gate"]["decision"], "pass");
    assert_ne!(
        failed["snapshot"]["content_digest"],
        passed["snapshot"]["content_digest"]
    );
}

#[test]
fn missing_configuration_and_invalid_selection_return_incomplete() {
    let root = fixture();
    report(&cli(root.path(), &["check", "--format", "json"]), 2);
    assert_eq!(
        cli(root.path(), &["check", "--staged", "--worktree"])
            .status
            .code(),
        Some(2)
    );
    assert!(cli(root.path(), &["--help"]).status.success());
}

#[test]
fn trusted_policy_detects_disabled_checks_and_formats_preserve_exit_codes() {
    let root = fixture();
    cli(root.path(), &["init"]);
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "policy"]);
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules: {line-ending: {enabled: false}}\n",
    )
    .unwrap();
    let blocked = report(
        &cli(
            root.path(),
            &["check", "--policy-ref", "HEAD", "--format", "json"],
        ),
        2,
    );
    assert_eq!(blocked["policy"]["changes"][0], "qualitygate.yaml");
    assert_eq!(
        cli(
            root.path(),
            &["check", "--policy-ref", "HEAD", "--format", "markdown"]
        )
        .status
        .code(),
        Some(2)
    );
}

#[test]
fn required_tool_absence_blocks_even_when_violation_severity_is_warning() {
    let root = fixture();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nrules: {line-ending: {}}\nchecks: [{id: missing, argv: [qualitygate-no-such-executable], severity: warning}]\n").unwrap();
    let value = report(
        &cli(
            root.path(),
            &["check", "--format", "json", "--severity", "error"],
        ),
        2,
    );
    assert_eq!(value["checks"][1]["execution"]["status"], "tool_error");
}

#[cfg(unix)]
#[test]
fn zero_test_success_exit_and_stale_reports_cannot_pass() {
    let root = fixture();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: tests\n    argv: [sh, -c, 'printf \"<testsuite/>\" > junit.xml']\n    reports: [{path: junit.xml, format: junit}]\n").unwrap();
    let zero = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    assert_eq!(zero["checks"][0]["metadata"]["tests"]["executed"], 0);
    std::fs::write(
        root.path().join("junit.xml"),
        "<testsuite tests=\"1\"><testcase name=\"old\"/></testsuite>",
    )
    .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: tests\n    argv: [sh, -c, 'true']\n    reports: [{path: junit.xml, format: junit}]\n").unwrap();
    let missing = report(&cli(root.path(), &["check", "--format", "json"]), 2);
    assert!(
        missing["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("not produced")
    );
}

#[cfg(unix)]
#[test]
fn baseline_analysis_filters_old_diagnostics_even_when_their_line_moves() {
    let root = fixture();
    let config = "schema_version: 1\nchecks:\n  - id: static\n    argv: [sh, -c, 'cat findings.json > report.json']\n    reports: [{path: report.json, format: diagnostics, mode: new_diagnostics, baseline: report.json}]\n";
    std::fs::write(root.path().join("qualitygate.yaml"), config).unwrap();
    std::fs::write(
        root.path().join("findings.json"),
        r#"{"issues":[{"rule":"old","file":"hello.txt","line":1,"message":"old warning"}]}"#,
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "baseline"]);
    std::fs::write(root.path().join("hello.txt"), "first\ninitial\nnew\n").unwrap();
    std::fs::write(root.path().join("findings.json"), r#"{"issues":[{"rule":"old","file":"hello.txt","line":2,"message":"old warning"},{"rule":"new","file":"hello.txt","line":3,"message":"new violation"}]}"#).unwrap();
    let result = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    assert_eq!(
        result["checks"][0]["diagnostics"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        result["checks"][0]["diagnostics"][0]["message"],
        "new violation"
    );
    assert_eq!(result["checks"][0]["metadata"]["report.json:filtered"], 1);
    assert!(result["checks"][0]["metadata"]["baseline_execution"]["argv"].is_array());
}

#[cfg(unix)]
#[test]
fn task_acceptance_is_merged_and_quick_reports_pending_delivery() {
    let root = fixture();
    cli(root.path(), &["init"]);
    std::fs::write(root.path().join("task.yaml"), "schema_version: 1\ntask_id: example\nacceptance:\n  - id: behavior\n    description: The requested behavior works\n    verification:\n      check_id: behavior-test\n      kind: command\n      argv: [sh, -c, 'exit 4']\n").unwrap();
    let quick = report(
        &cli(
            root.path(),
            &[
                "check",
                "--task",
                "task.yaml",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(quick["plan"]["pending_delivery_checks"][0], "behavior-test");
    let full = report(
        &cli(
            root.path(),
            &["check", "--task", "task.yaml", "--format", "json"],
        ),
        1,
    );
    assert_eq!(full["scope"], "task");
    assert_eq!(full["plan"]["acceptance"]["behavior"], "behavior-test");
    assert_eq!(full["checks"][1]["execution"]["exit_code"], 4);
}
