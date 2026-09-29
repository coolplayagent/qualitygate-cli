#[path = "doctor/budgets.rs"]
mod budgets;
mod common;
#[path = "doctor/probes.rs"]
mod probes;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn producer() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    if let Some(binary) = option_env!("QUALITYGATE_TEST_PROBE") {
        std::fs::copy(binary, directory.path().join("probe.exe")).unwrap();
        return directory;
    }
    let source = directory.path().join("probe.rs");
    std::fs::write(&source, include_str!("doctor/native_probe.rs")).unwrap();
    let output = Command::new("rustc")
        .args(["--edition=2024", "--crate-name", "doctor_probe"])
        .arg(&source)
        .arg("-o")
        .arg(directory.path().join("probe.exe"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    directory
}

fn policy(root: &Path, value: Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

fn simple() -> Value {
    json!({"schema_version":1,"checks":[{"id":"verify","argv":["git","--version"],"tools":[{"id":"git","argv":["git","--version"]}]}]})
}

fn doctor(root: &Path, args: &[&str], code: i32) -> Value {
    let mut all = vec!["doctor", "--format", "json"];
    all.extend(args);
    let value = report(&cli(root, &all), code);
    qualitygate::config::preflight_schema::validate_doctor(&value).unwrap();
    assert_eq!(value["scope"], "preflight");
    assert!(value.get("gate").is_none());
    value
}

fn has_code(report: &Value, code: &str) -> bool {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == code)
}

#[test]
fn offline_capabilities_and_schemas_ignore_repository_and_invalid_policy() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), "invalid: [").unwrap();
    for path in [root.path().to_path_buf(), root.path().join("absent")] {
        let value = report(&cli(&path, &["capabilities", "--format", "json"]), 0);
        qualitygate::config::preflight_schema::validate_capabilities(&value).unwrap();
        assert_eq!(value["cli_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            value["executable"]["digest"],
            qualitygate::snapshot::digest(
                &std::fs::read(env!("CARGO_BIN_EXE_qualitygate")).unwrap()
            )
        );
        assert!(
            value["capabilities"]
                .as_array()
                .unwrap()
                .contains(&json!("doctor.static.v1"))
        );
        for kind in ["doctor", "capabilities"] {
            let schema = report(&cli(&path, &["schema", kind, "--format", "json"]), 0);
            let bundled: Value = serde_json::from_str(
                &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "skills/qualitygate-cli/references/schemas/{kind}.schema.json"
                )))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(schema, bundled);
        }
    }
    let mut value = report(&cli(root.path(), &["capabilities", "--format", "json"]), 0);
    value["executable"]["digest"] = "forged".into();
    assert!(qualitygate::config::preflight_schema::validate_capabilities(&value).is_err());
}

#[test]
fn explicit_requirements_precede_strict_fields_but_do_not_mask_bad_yaml() {
    let root = fixture();
    for (text, code) in [
        (
            "schema_version: 1\nrequires: {min_cli_version: '999.0.0'}\nfuture_field: true\n",
            "runtime.version_too_old",
        ),
        (
            "schema_version: 1\nrequires: {capabilities: [future.v9]}\nfuture_field: true\n",
            "runtime.capability_missing",
        ),
        ("schema_version: 1\ncheckz: []\n", "config.unknown_field"),
        ("schema_version: []\n", "config.type"),
        (
            "schema_version: 1\nrequires: {min_cli_version: '999.0.0'}\ninvalid: [\n",
            "config.syntax",
        ),
        ("schema_version: 1\nschema_version: 1\n", "config.syntax"),
        (
            "schema_version: 1\nrequires: {capabilities: wrong}\n",
            "config.type",
        ),
        (
            "schema_version: 1\nrequires: {min_cli_version: not-a-version}\n",
            "config.type",
        ),
    ] {
        std::fs::write(root.path().join("qualitygate.yaml"), text).unwrap();
        let result = doctor(root.path(), &[], 2);
        assert!(has_code(&result, code), "{result}");
        let check = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert_eq!(check["diagnostics"][0]["code"], code);
        if code == "config.unknown_field" {
            assert_eq!(result["diagnostics"][0]["field"], "checkz");
            assert!(result["diagnostics"][0]["line"].as_u64().is_some());
            assert!(
                result["diagnostics"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("unknown field")
            );
        }
    }
    let legacy =
        qualitygate::config::parse(b"schema_version: 1\nchecks: [{id: test, argv: [git]}]\n")
            .unwrap();
    let serialized = serde_json::to_value(legacy).unwrap();
    assert!(serialized.get("requires").is_none());
    assert!(serialized["checks"][0].get("required_env").is_none());
    let mut value = simple();
    value["requires"] =
        json!({"min_cli_version":"0.1.0-alpha.1","capabilities":["doctor.static.v1"]});
    policy(root.path(), value);
    doctor(root.path(), &[], 0);
}

#[test]
fn static_inspection_never_launches_probes_or_formal_commands() {
    let root = fixture();
    let tool = producer();
    let binary = tool.path().join("probe.exe");
    let command_marker = tool.path().join("formal-command");
    let probe_marker = tool.path().join("probe-command");
    policy(
        root.path(),
        json!({"schema_version":1,"checks":[{"id":"verify","argv":[binary,"record-failure",command_marker],"tools":[{"id":"tool","argv":[binary,"record-failure",probe_marker]}]}]}),
    );
    let value = doctor(root.path(), &[], 0);
    assert_eq!(value["checks"][0]["probes"][0]["attempted"], false);
    assert_eq!(value["not_executed"].as_array().unwrap().len(), 2);
    assert!(!command_marker.exists());
    assert!(!probe_marker.exists());
    let value = doctor(root.path(), &["--probe-tools"], 2);
    assert!(has_code(&value, "probe.exit_code"));
    assert_eq!(value["not_executed"].as_array().unwrap().len(), 1);
    assert!(probe_marker.exists());
    assert!(!command_marker.exists());
    policy(
        root.path(),
        json!({"schema_version":1,"checks":[{"id":"verify","argv":[binary,"record-failure",command_marker],"tools":[{"id":"tool","argv":["git","--version"]}]}]}),
    );
    let value = doctor(root.path(), &["--probe-tools"], 0);
    assert_eq!(value["checks"][0]["probes"][0]["complete"], true);
    assert_eq!(value["not_executed"][0]["operation"], "check");
    assert!(!command_marker.exists());
    assert!(!root.path().join(".git/qualitygate-test-evidence").exists());
}

#[test]
fn environment_requirements_block_check_and_task_commands_without_leaking_values() {
    let root = fixture();
    let name = "QUALITYGATE_DOCTOR_TEST_REQUIRED";
    let mut value = simple();
    value["checks"][0]["required_env"] = json!([name]);
    policy(root.path(), value);
    let run = |subcommand: &str, value: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_qualitygate"));
        command
            .env_remove(name)
            .env("QUALITYGATE_HOME", root.path().join(".git/evidence"))
            .args([
                "--root",
                root.path().to_str().unwrap(),
                subcommand,
                "--format",
                "json",
            ]);
        if let Some(value) = value {
            command.env(name, value);
        }
        command.output().unwrap()
    };
    for value in [None, Some("")] {
        assert!(has_code(
            &report(&run("doctor", value), 2),
            "environment.missing"
        ));
        let check = report(&run("check", value), 2);
        assert_eq!(
            check["checks"][0]["metadata"]["required_env_missing"],
            json!([name])
        );
        assert!(check["checks"][0]["execution"]["started_at_ms"].is_null());
    }
    let output = run("doctor", Some("sensitive-test-value"));
    report(&output, 0);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("sensitive-test-value"));
    policy(root.path(), json!({"schema_version":1}));
    let task = json!({"schema_version":1,"task_id":"task","acceptance":[{"id":"requirement","description":"needs environment","verification":{"check_id":"task-check","argv":["git","--version"],"required_env":["QUALITYGATE_ABSENT_TASK_39"]}}]});
    std::fs::write(
        root.path().join("task.yaml"),
        serde_json::to_vec(&task).unwrap(),
    )
    .unwrap();
    assert!(has_code(
        &doctor(root.path(), &["--task", "task.yaml"], 2),
        "environment.missing"
    ));
    report(
        &cli(
            root.path(),
            &["check", "--task", "task.yaml", "--format", "json"],
        ),
        2,
    );
}

#[test]
fn selected_profile_expands_dependencies_and_does_not_inspect_unselected_commands() {
    let root = fixture();
    let value = json!({"schema_version":1,"checks":[
        {"id":"dependency","argv":["git","--version"],"required":false},
        {"id":"selected","argv":["git","--version"],"depends_on":["dependency"]},
        {"id":"other","argv":["qualitygate-absent-tool-39"],"required_env":["QUALITYGATE_ABSENT_39"]}
    ],"profiles":{"quick":{"include":["selected"]},"full":{"include":["selected","other"]}}});
    policy(root.path(), value);
    let result = doctor(root.path(), &["--profile", "quick"], 0);
    assert_eq!(result["planned_checks"], json!(["dependency", "selected"]));
    assert_eq!(result["pending_delivery_checks"], json!(["other"]));
    let full = doctor(root.path(), &[], 2);
    assert!(has_code(&full, "command.executable_missing"));
    assert!(has_code(&full, "environment.missing"));
}

#[test]
fn snapshot_and_policy_selection_never_fall_back_to_worktree_configuration() {
    let root = fixture();
    policy(root.path(), simple());
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "policy"]);
    let mut broken = simple();
    broken["checks"][0]["argv"] = json!(["qualitygate-absent-tool-39"]);
    policy(root.path(), broken);
    let staged = doctor(root.path(), &["--staged"], 0);
    assert_eq!(staged["snapshot"]["mode"], "staged");
    doctor(root.path(), &["--diff", "HEAD~1..HEAD"], 0);
    assert!(has_code(
        &doctor(root.path(), &["--worktree"], 2),
        "command.executable_missing"
    ));
    let trusted = doctor(root.path(), &["--policy-ref", "HEAD"], 2);
    assert!(has_code(&trusted, "policy.snapshot_mismatch"));
    assert_eq!(trusted["policy"]["trust"], "caller_supplied_ref");
    git(root.path(), &["add", "."]);
    assert!(has_code(
        &doctor(root.path(), &["--staged"], 2),
        "command.executable_missing"
    ));
    git(root.path(), &["commit", "-qm", "candidate"]);
    doctor(
        root.path(),
        &["--diff", "HEAD~2..HEAD~1", "--policy-ref", "HEAD~1"],
        0,
    );
    assert!(has_code(
        &doctor(
            root.path(),
            &["--diff", "HEAD~1..HEAD", "--policy-ref", "HEAD~1"],
            2
        ),
        "policy.snapshot_mismatch"
    ));
}

#[test]
fn preflight_reports_missing_context_and_rejects_unsupported_scope() {
    let root = fixture();
    let absent = doctor(root.path(), &[], 2);
    assert!(has_code(&absent, "config.missing"));
    assert!(!absent["next_steps"].as_array().unwrap().is_empty());
    let mut value = simple();
    value["checks"][0]["cwd"] = "absent".into();
    value["checks"][0]["required_args"] = json!(["--required"]);
    value["checks"][0]["tools"][0]["inputs"] = json!(["absent.input"]);
    policy(root.path(), value);
    let result = doctor(root.path(), &[], 2);
    for code in [
        "command.cwd_missing",
        "command.argument_missing",
        "probe.input_missing",
    ] {
        assert!(has_code(&result, code), "{result}");
    }
    for args in [
        vec!["doctor", "--mr", "https://example.invalid"],
        vec!["doctor", "--diff", "a...b"],
        vec!["doctor", "--staged", "--worktree"],
        vec!["doctor", "--snapshot-max-file-mib", "9"],
        vec!["doctor", "--envelope", "--format", "json"],
    ] {
        assert_eq!(cli(root.path(), &args).status.code(), Some(2));
    }
    let empty = tempfile::tempdir().unwrap();
    doctor(&empty.path().join("absent"), &[], 2);
}
