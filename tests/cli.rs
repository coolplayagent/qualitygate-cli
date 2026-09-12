mod common;
use common::*;

#[cfg(unix)]
#[test]
fn fresh_coverage_rejects_missing_sources_and_branches_before_a_successful_recheck() {
    let root = fixture();
    let root = root.path();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(root.join("src/b.rs"), "fn b() {}\n").unwrap();
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: coverage\n    argv: [sh, analyze.sh]\n    tools: [{id: coverage-fixture, argv: [sh, -c, 'printf fixture-v1'], inputs: [analyze.sh]}]\n    reports:\n      - path: coverage.info\n        format: lcov\n        coverage_paths: ['src/*.rs']\n        minimum_coverage: 90\n").unwrap();
    let a = "SF:src/a.rs\nDA:1,1\nBRDA:1,0,0,1\nBRDA:1,0,1,0\nLF:1\nLH:1\nBRF:2\nBRH:1\nend_of_record\n";
    let b = "SF:src/b.rs\nDA:1,1\nLF:1\nLH:1\nBRF:0\nBRH:0\nend_of_record\n";
    let run = |text: &str, code| {
        std::fs::write(
            root.join("analyze.sh"),
            format!("cat > coverage.info <<'COVERAGE'\n{text}COVERAGE\n"),
        )
        .unwrap();
        report(&cli(root, &["check", "--format", "json"]), code)
    };
    let omitted = run(a, 2);
    assert!(
        omitted["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("omitted sources")
    );
    let uncovered = run(&format!("{a}{b}"), 1);
    assert_eq!(
        uncovered["checks"][0]["metadata"]["coverage.info:coverage"]["branch_percent"],
        50.0
    );
    let covered = a
        .replace("BRDA:1,0,1,0", "BRDA:1,0,1,1")
        .replace("BRH:1", "BRH:2");
    let passed = run(&format!("{covered}{b}"), 0);
    assert_eq!(
        passed["checks"][0]["metadata"]["coverage.info:coverage"]["branch_percent"],
        100.0
    );
    assert_eq!(
        passed["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[cfg(unix)]
#[test]
fn command_preconditions_timeout_and_prerequisites_remain_incomplete_with_logs() {
    let root = fixture();
    let root = root.path();
    let run = |checks: &str, code| {
        std::fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nchecks:\n{checks}\n"),
        )
        .unwrap();
        report(&cli(root, &["check", "--format", "json"]), code)
    };
    let missing = run(
        "  - {id: compile, argv: [sh, -c, 'exit 0'], required_args: [-s], severity: warning}",
        2,
    );
    assert_eq!(missing["checks"][0]["execution"]["status"], "blocked");
    assert_eq!(
        missing["checks"][0]["execution"]["exit_code"],
        serde_json::Value::Null
    );
    let blocked = run(
        "  - {id: compile, argv: [sh, -c, 'exit 4']}\n  - {id: tests, argv: [sh, -c, 'exit 0'], depends_on: [compile]}",
        2,
    );
    assert_eq!(blocked["checks"][0]["verdict"], "fail");
    assert_eq!(blocked["checks"][1]["execution"]["status"], "blocked");
    let expected = run(
        "  - {id: expected, argv: [sh, -c, 'exit 7'], expected_exit_code: 7}",
        0,
    );
    assert_eq!(expected["checks"][0]["execution"]["exit_code"], 7);
    let timeout = run(
        "  - {id: slow, argv: [sh, -c, 'printf partial; sleep 30'], timeout_seconds: 1}",
        2,
    );
    assert_eq!(timeout["checks"][0]["execution"]["status"], "timed_out");
    let path = timeout["checks"][0]["execution"]["artifacts"][0]["path"]
        .as_str()
        .unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "partial");
    let manual = run(
        "  - {id: review, kind: manual, evidence_file: review.json}",
        2,
    );
    assert_eq!(manual["checks"][0]["execution"]["status"], "blocked");
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
fn trusted_policy_preserves_severity_task_items_and_verification_inputs() {
    let root = fixture();
    let policy = serde_json::json!({
        "schema_version": 1,
        "rules": {"line-ending": {"required": true, "severity": "error"}},
        "checks": [{"id": "verification-build", "argv": ["rustc", "verify.rs", "-o", "verify-output"]}],
        "verification_assets": ["verify.rs", "verification/**"]
    });
    let task = serde_json::json!({"schema_version": 1, "task_id": "preserve-contract", "acceptance": [
        {"id": "first", "description": "First required condition", "verification": {"check_id": "first", "argv": ["rustc", "--version"]}},
        {"id": "second", "description": "Second required condition", "verification": {"check_id": "second", "argv": ["rustc", "--version"]}}
    ]});
    let write_json = |name: &str, value: &serde_json::Value| {
        std::fs::write(
            root.path().join(name),
            serde_norway::to_string(value).unwrap(),
        )
        .unwrap();
    };
    write_json("qualitygate.yaml", &policy);
    write_json("task.yaml", &task);
    std::fs::write(
        root.path().join("verify.rs"),
        "fn main() { assert!(true); }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &["commit", "-qm", "trusted complete verification contract"],
    );
    let run = |code| {
        report(
            &cli(
                root.path(),
                &[
                    "check",
                    "--policy-ref",
                    "HEAD",
                    "--task",
                    "task.yaml",
                    "--format",
                    "json",
                ],
            ),
            code,
        )
    };
    run(0);
    let expect_change = |path: &str| {
        let output = run(2);
        assert!(
            output["policy"]["changes"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(path))
        );
        assert_eq!(output["gate"]["complete"], false);
    };
    let mut weakened = policy.clone();
    weakened["rules"]["line-ending"]["severity"] = serde_json::json!("warning");
    write_json("qualitygate.yaml", &weakened);
    expect_change("qualitygate.yaml");
    write_json("qualitygate.yaml", &policy);
    let mut shortened = task.clone();
    shortened["acceptance"].as_array_mut().unwrap().pop();
    write_json("task.yaml", &shortened);
    expect_change("task.yaml");
    write_json("task.yaml", &task);
    std::fs::write(root.path().join("verify.rs"), "fn main() {}\n").unwrap();
    expect_change("verify.rs");
    std::fs::remove_file(root.path().join("verify.rs")).unwrap();
    expect_change("verify.rs");
    git(root.path(), &["restore", "verify.rs"]);
    std::fs::create_dir(root.path().join("verification")).unwrap();
    std::fs::write(root.path().join("verification/new.rs"), "fn main() {}\n").unwrap();
    expect_change("verification/new.rs");
    std::fs::remove_file(root.path().join("verification/new.rs")).unwrap();
    run(0);
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
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: tests\n    argv: [sh, -c, 'printf \"<testsuite/>\" > junit.xml']\n    tools: [{id: junit-fixture, argv: [sh, -c, 'printf fixture-v1']}]\n    reports: [{path: junit.xml, format: junit}]\n").unwrap();
    let zero = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    assert_eq!(zero["checks"][0]["metadata"]["tests"]["executed"], 0);
    std::fs::write(
        root.path().join("junit.xml"),
        "<testsuite tests=\"1\"><testcase name=\"old\"/></testsuite>",
    )
    .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: tests\n    argv: [sh, -c, 'true']\n    tools: [{id: junit-fixture, argv: [sh, -c, 'printf fixture-v1']}]\n    reports: [{path: junit.xml, format: junit}]\n").unwrap();
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
    let config = "schema_version: 1\nchecks:\n  - id: static\n    argv: [sh, -c, 'cat findings.json > report.json']\n    tools: [{id: analyzer-fixture, argv: [sh, -c, 'printf fixture-v1']}]\n    reports: [{path: report.json, format: diagnostics, mode: new_diagnostics, baseline: report.json}]\n";
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
