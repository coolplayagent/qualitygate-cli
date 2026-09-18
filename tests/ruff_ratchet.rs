//! Actual Ruff JSON across paired Python snapshots.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn run(root: &Path, exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env(
            "QUALITYGATE_HOME",
            root.join(".git/qualitygate-test-evidence"),
        )
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/skills/qualitygate-cli/references/rules"
            ),
        )
        .arg("--root")
        .arg(root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    report(&output, exit)
}

#[test]
fn ruff_reference_policy_is_valid_and_unknown_format_fails() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/ruff-ratchet.yaml"),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(0)
    );
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/ruff-ratchet.yaml")
            .replace("ruff_json", "unknown_ruff"),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
#[ignore = "requires pinned Ruff 0.16.8; mandatory live Python CI job"]
fn real_ruff_stdout_ratchet_growth_repair_and_invalid_syntax() {
    let binary = std::env::var("RUFF_BIN").expect("Set RUFF_BIN to pinned Ruff binary");
    let version = Command::new(&binary).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("ruff 0.16.8"));

    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("ruff.toml"), "[lint]\nselect = [\"F401\"]\n").unwrap();
    fs::write(root.join("app.py"), "import os\n").unwrap();
    let policy = json!({"schema_version":1,"checks":[{
        "id":"ruff-ratchet",
        "argv":[binary,"check","--output-format=json","--no-fix","--no-fix-only","--no-cache","."],
        "timeout_seconds":120,"findings_exit_codes":[1],
        "tools":[{"id":"ruff","argv":[binary,"--version"]}],
        "reports":[{"path":"target/ruff.json","baseline":"target/ruff.json",
            "from_stdout":true,"format":"ruff_json","mode":"ratchet"}]
    }]});
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical Ruff finding"]);

    let initial = run(root, 0);
    assert_eq!(initial["checks"][0]["execution"]["exit_code"], 1);
    assert_eq!(
        initial["checks"][0]["metadata"]["target/ruff.json:ratchet"][0]["baseline"],
        1
    );
    fs::write(root.join("app.py"), "import os\nimport sys\n").unwrap();
    let growth = run(root, 1);
    assert_eq!(
        growth["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        growth["checks"][0]["diagnostics"][0]["evidence"]["tool"],
        "ruff"
    );
    fs::write(root.join("app.py"), "value = 1\n").unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");
    fs::write(root.join("app.py"), "def broken(:\n").unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}
