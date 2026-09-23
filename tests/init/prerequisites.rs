use super::*;
use serde_json::{Value, json};
use std::{fs, process::Command};

fn failure(root: &Path, arguments: &[&str], code: &str) -> Value {
    let value = report(&cli(root, arguments), 2);
    qualitygate::config::decision_schema::validate_command_error(&value).unwrap();
    assert_eq!(value["issues"][0]["code"], code, "{value}");
    assert!(value.get("run_id").is_none());
    value
}

fn policy(root: &Path, checks: Value) {
    fs::write(
        root.join("qualitygate.yaml"),
        serde_json::to_vec(&json!({
            "schema_version":1,"rules":{"line-ending":{}},"checks":checks
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn missing_local_configuration_returns_exact_recovery_arguments_without_writes() {
    let root = tempfile::Builder::new()
        .prefix("qualitygate spaces ")
        .tempdir()
        .unwrap();
    for arguments in [
        vec!["config", "--show"],
        vec!["rules", "enable", "line-ending"],
        vec!["rules", "disable", "line-ending"],
        vec!["rules", "configure", "line-ending", "--severity", "warning"],
        vec!["rules", "categories", "create", "team"],
        vec!["rules", "assign", "line-ending", "--category", "core"],
        vec!["rules", "list"],
    ] {
        let mut command = vec!["--config", "settings/team policy.yaml", "--format", "json"];
        command.extend(arguments);
        let value = failure(root.path(), &command, "repository_not_initialized");
        assert_eq!(
            value["issues"][0]["next_actions"][0]["argv"],
            json!([
                "qualitygate",
                "--root",
                root.path().display().to_string(),
                "--config",
                "settings/team policy.yaml",
                "init",
                "--format",
                "json"
            ])
        );
        assert!(!root.path().join("settings").exists());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn discovery_and_independent_inputs_remain_available_before_init() {
    let root = tempfile::tempdir().unwrap();
    for arguments in [
        vec!["rules", "list"],
        vec!["rules", "describe", "line-ending"],
        vec!["rules", "categories"],
        vec!["rules", "context"],
    ] {
        let mut command = arguments;
        command.extend(["--format", "json"]);
        report(&cli(root.path(), &command), 0);
    }
    fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nKeep lines bounded.\n",
    )
    .unwrap();
    report(
        &cli(
            root.path(),
            &[
                "rules",
                "source",
                "--document",
                "AGENTS.md",
                "--section",
                "Rules",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let validation = report(
        &cli(
            root.path(),
            &["rules", "validate", "missing.yaml", "--format", "json"],
        ),
        2,
    );
    assert_ne!(validation["kind"], "command_error");
    assert!(!root.path().join("qualitygate.yaml").exists());
    failure(
        root.path(),
        &[
            "judgment",
            "question-digest",
            "--input",
            "missing.yaml",
            "--format",
            "json",
        ],
        "input_missing",
    );
    failure(
        root.path(),
        &[
            "pilot",
            "seal",
            "--input",
            "missing.json",
            "--format",
            "json",
        ],
        "input_missing",
    );
    failure(
        root.path(),
        &["policy", "history", "--format", "json"],
        "archive_unavailable",
    );
}

#[test]
fn schema_export_ignores_root_and_error_schema_rejects_false_success() {
    let root = tempfile::tempdir().unwrap();
    let schema = report(
        &cli(&root.path().join("absent"), &["schema", "command-error"]),
        0,
    );
    assert_eq!(schema["$id"], "urn:qualitygate:command-error:1");
    let mut error = failure(
        root.path(),
        &["config", "--show", "--format", "json"],
        "repository_not_initialized",
    );
    error["gate"]["complete"] = json!(true);
    assert!(qualitygate::config::decision_schema::validate_command_error(&error).is_err());
    error["gate"]["complete"] = json!(false);
    error["issues"][0]["next_actions"][0]["argv"] = json!([]);
    assert!(qualitygate::config::decision_schema::validate_command_error(&error).is_err());
}

#[test]
fn invalid_config_and_root_do_not_recommend_initialization() {
    let root = fixture();
    for input in [
        "schema_version: 1\nunknown_field: true\n",
        "schema_version: [\n",
    ] {
        fs::write(root.path().join("qualitygate.yaml"), input).unwrap();
        let value = failure(
            root.path(),
            &["config", "--show", "--format", "json"],
            "config_invalid",
        );
        assert!(
            value["issues"][0]["next_actions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|action| action["kind"] == "instruction")
        );
        failure(
            root.path(),
            &["rules", "enable", "line-ending", "--format", "json"],
            "config_invalid",
        );
        assert!(!root.path().join("qualitygate.yaml.lock").exists());
    }
    fs::remove_file(root.path().join("qualitygate.yaml")).unwrap();
    fs::create_dir(root.path().join("qualitygate.yaml")).unwrap();
    failure(
        root.path(),
        &["config", "--show", "--format", "json"],
        "config_invalid",
    );
    failure(
        &root.path().join("hello.txt"),
        &["config", "--show", "--format", "json"],
        "root_unavailable",
    );
    failure(
        &root.path().join("absent"),
        &["config", "--show", "--format", "json"],
        "root_unavailable",
    );
}

#[test]
fn selected_snapshot_controls_missing_policy_recovery() {
    let root = fixture();
    failure(
        root.path(),
        &["check", "--worktree", "--format", "json"],
        "repository_not_initialized",
    );
    failure(
        root.path(),
        &["check", "--policy-ref", "HEAD", "--format", "json"],
        "policy_snapshot_missing",
    );
    policy(root.path(), json!([]));
    let staged = failure(
        root.path(),
        &["check", "--staged", "--format", "json"],
        "policy_snapshot_missing",
    );
    assert!(
        staged["issues"][0]["next_actions"][0]["message"]
            .as_str()
            .unwrap()
            .contains("stage")
    );
    git(root.path(), &["add", "qualitygate.yaml"]);
    report(
        &cli(root.path(), &["check", "--staged", "--format", "json"]),
        0,
    );
    failure(
        root.path(),
        &["check", "--task", "absent.yaml", "--format", "json"],
        "input_missing",
    );
    failure(
        root.path(),
        &[
            "rules",
            "context",
            "--policy-ref",
            "HEAD",
            "--format",
            "json",
        ],
        "policy_snapshot_missing",
    );
}

#[test]
fn git_dependencies_have_distinct_error_codes() {
    let root = tempfile::tempdir().unwrap();
    failure(
        root.path(),
        &["check", "--format", "json"],
        "git_repository_invalid",
    );
    git(root.path(), &["init", "-q"]);
    failure(
        root.path(),
        &["check", "--format", "json"],
        "git_ref_unavailable",
    );
    let root = fixture();
    policy(root.path(), json!([]));
    failure(
        root.path(),
        &["check", "--base", "no-such-reference", "--format", "json"],
        "git_ref_unavailable",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("PATH", "")
        .arg("--root")
        .arg(root.path())
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    let value = report(&output, 2);
    assert_eq!(value["issues"][0]["code"], "git_unavailable", "{value}");
}

#[test]
fn tooling_is_checked_per_selected_check_without_changing_optional_gate_semantics() {
    let root = fixture();
    for required in [true, false] {
        policy(
            root.path(),
            json!([{"id":"missing-tool","argv":["qualitygate-absent-tool"],"required":required}]),
        );
        let value = report(
            &cli(root.path(), &["check", "--format", "json"]),
            if required { 2 } else { 0 },
        );
        let check = value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == "missing-tool")
            .unwrap();
        assert_eq!(
            check["metadata"]["prerequisites"][0]["code"], "tool_unavailable",
            "{value}"
        );
        assert_eq!(
            check["metadata"]["prerequisites"][0]["check_id"],
            "missing-tool"
        );
    }
    let value = report(
        &cli(
            root.path(),
            &["check", "--profile", "quick", "--format", "json"],
        ),
        0,
    );
    assert!(
        value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["id"] != "missing-tool")
    );
}

#[test]
fn dependent_working_directories_are_checked_after_their_producer() {
    let root = fixture();
    policy(
        root.path(),
        json!([
            {"id":"create-directory","argv":["git","init","--bare","generated"]},
            {"id":"use-directory","argv":["git","--version"],"cwd":"generated","depends_on":["create-directory"]}
        ]),
    );
    report(&cli(root.path(), &["check", "--format", "json"]), 0);
    policy(
        root.path(),
        json!([{"id":"missing-cwd","argv":["git","--version"],"cwd":"absent"}]),
    );
    let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
    let check = value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "missing-cwd")
        .unwrap();
    assert_eq!(
        check["metadata"]["prerequisites"][0]["code"],
        "workspace_unavailable"
    );
}

#[test]
fn failed_and_timed_out_tool_probes_produce_actionable_incomplete_checks() {
    let root = fixture();
    let executable = env!("CARGO_BIN_EXE_qualitygate");
    for mode in ["failure", "timeout-long"] {
        policy(
            root.path(),
            json!([{"id":"probe","argv":["git","--version"],
            "tools":[{"id":"version","argv":[executable,"selfcheck-probe",mode],"timeout_seconds":1}]}]),
        );
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        let check = value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == "probe")
            .unwrap();
        assert_eq!(
            check["metadata"]["prerequisites"][0]["code"], "tool_probe_failed",
            "{value}"
        );
        assert!(check["execution"]["started_at_ms"].is_null());
    }
}

#[test]
fn storage_lock_and_packaged_assets_have_specific_remediation() {
    let root = fixture();
    policy(root.path(), json!([]));
    fs::write(root.path().join("qualitygate.yaml.lock"), "active writer").unwrap();
    failure(
        root.path(),
        &["rules", "enable", "line-ending", "--format", "json"],
        "lock_unavailable",
    );
    let blocked = root.path().join("hello.txt").display().to_string();
    failure(
        root.path(),
        &["check", "--output-dir", &blocked, "--format", "json"],
        "storage_unavailable",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            root.path().join("absent-assets"),
        )
        .arg("--root")
        .arg(root.path())
        .args(["rules", "list", "--format", "json"])
        .output()
        .unwrap();
    let value = report(&output, 2);
    assert_eq!(value["issues"][0]["code"], "rule_assets_unavailable");
    assert!(
        value["issues"][0]["next_actions"][0]["message"]
            .as_str()
            .unwrap()
            .contains("QUALITYGATE_BUILTIN_RULES_DIR")
    );
}

#[test]
fn argument_errors_and_tool_inputs_are_not_initialization_failures() {
    let root = fixture();
    for arguments in [
        vec!["check", "--diff", "HEAD", "--format", "json"],
        vec!["check", "--diff", "HEAD...HEAD", "--format", "json"],
        vec!["rules", "list", "--envelope", "--format", "json"],
        vec![
            "pilot",
            "seal",
            "--input",
            "absent",
            "--envelope",
            "--format",
            "json",
        ],
    ] {
        failure(root.path(), &arguments, "invalid_arguments");
    }
    policy(
        root.path(),
        json!([{"id":"tool-input","argv":["git","--version"],
            "tools":[{"id":"git","argv":["git","--version"],"inputs":["absent.lock"]}]}]),
    );
    let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
    let check = value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "tool-input")
        .unwrap();
    assert_eq!(
        check["metadata"]["prerequisites"][0]["code"],
        "input_missing"
    );
    assert!(check["execution"]["started_at_ms"].is_null());
}

#[test]
fn early_feedback_and_envelope_errors_use_the_command_error_contract() {
    let root = fixture();
    for arguments in [
        vec!["check", "--feedback", "--feedback-max-bytes", "4096"],
        vec!["check", "--envelope", "--format", "json"],
        vec!["check", "--feedback", "--envelope", "--format", "json"],
    ] {
        let output = cli(root.path(), &arguments);
        assert!(output.stdout.len() <= 4096);
        let value = report(&output, 2);
        qualitygate::config::decision_schema::validate_command_error(&value).unwrap();
        assert_eq!(value["issues"][0]["code"], "repository_not_initialized");
        assert!(value.get("full_report").is_none());
    }
}

#[test]
fn large_diagnostics_keep_whole_recovery_arguments_or_an_instruction() {
    use qualitygate::domain::prerequisites::{FailureCode, Phase, PrerequisiteIssue};
    use qualitygate::interfaces::{cli::Format, errors::ErrorContext};
    for length in [20, 16_000] {
        let context = ErrorContext {
            root: "目录 ".repeat(length),
            config: "policy.yaml".into(),
            format: Format::Json,
            max_bytes: 4096,
        };
        let error = PrerequisiteIssue::new(
            FailureCode::RepositoryNotInitialized,
            Phase::Policy,
            "Missing configuration",
        )
        .resource("资源".repeat(4000))
        .wrap(anyhow::anyhow!("{}", "cause".repeat(5000)));
        let output = context.render(&error);
        assert!(output.len() < 4096);
        let value: Value = serde_json::from_str(&output).unwrap();
        qualitygate::config::decision_schema::validate_command_error(&value).unwrap();
        let action = &value["issues"][0]["next_actions"][0];
        if length == 20 {
            assert_eq!(action["argv"][2], context.root);
        } else {
            assert_eq!(action["kind"], "instruction");
            assert!(action.get("argv").is_none());
        }
    }
}

#[test]
fn escaped_diagnostics_and_check_ids_respect_the_error_budget() {
    use qualitygate::domain::prerequisites::{FailureCode, Phase, PrerequisiteIssue};
    use qualitygate::interfaces::{cli::Format, errors::ErrorContext};
    let context = ErrorContext {
        root: ".".into(),
        config: "qualitygate.yaml".into(),
        format: Format::Json,
        max_bytes: 4096,
    };
    let mut issue = PrerequisiteIssue::new(
        FailureCode::EvidenceInvalid,
        Phase::Evidence,
        "\0".repeat(10_000),
    )
    .instruction("\0".repeat(10_000));
    issue.check_id = Some("check".repeat(10_000));
    let output = context.render(&issue.into());
    assert!(output.len() < 4096);
    let value = serde_json::from_str(&output).unwrap();
    qualitygate::config::decision_schema::validate_command_error(&value).unwrap();
}
