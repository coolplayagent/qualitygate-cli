//! Actual Cargo Clippy JSON Lines in paired snapshots, separate from parser fixtures.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const OLD: &str = "pub fn empty(values: &[u8]) -> bool { values.len() == 0 }\n";
const NEW: &str = "pub fn other(values: &[u8]) -> bool { values.len() == 0 }\n";

fn run(root: &Path, exit: i32) -> Value {
    report(&cli(root, &["check", "--format", "json"]), exit)
}

#[test]
fn stdout_report_configuration_requires_a_single_declared_source() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/clippy-ratchet.yaml"),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(0)
    );
    for reports in [
        "[{path: target/clippy.jsonl, format: cargo_clippy}]",
        "[{path: a.jsonl, format: cargo_clippy, from_stdout: true}, {path: b.jsonl, format: cargo_clippy, from_stdout: true}]",
    ] {
        fs::write(root.join("qualitygate.yaml"), format!("schema_version: 1\nchecks:\n  - id: clippy\n    argv: [cargo, clippy]\n    tools: [{{id: clippy, argv: [cargo, clippy, --version]}}]\n    reports: {reports}\n")).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
#[ignore = "requires Clippy; mandatory live Clippy CI job"]
fn real_clippy_stdout_ratchet_repair_and_incomplete_compilation() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), OLD).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"clippy-ratchet-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("rust-toolchain.toml"),
        include_str!("../rust-toolchain.toml"),
    )
    .unwrap();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let lock = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"clippy-ratchet",
        "argv":["cargo","clippy","--locked","--offline","--message-format=json","--lib","--","-W","clippy::len_zero"],
        "timeout_seconds":90,
        "tools":[{"id":"clippy","argv":["cargo","clippy","--version"]}],
        "reports":[{"path":"target/clippy.jsonl","baseline":"target/clippy.jsonl","from_stdout":true,"format":"cargo_clippy","mode":"ratchet"}]
    }]});
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical Clippy finding"]);

    let initial = run(root, 0);
    assert_eq!(
        initial["checks"][0]["metadata"]["target/clippy.jsonl:ratchet"][0]["baseline"],
        1
    );
    assert_eq!(
        initial["checks"][0]["metadata"]["target/clippy.jsonl:ratchet"][0]["current"],
        1
    );
    assert!(
        initial["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["path"]
                .as_str()
                .unwrap()
                .ends_with("baseline-report-0"))
    );

    policy["checks"][0]["argv"] = json!([
        "cargo",
        "clippy",
        "--locked",
        "--offline",
        "--message-format=json",
        "--lib",
        "--",
        "-D",
        "clippy::len_zero"
    ]);
    policy["checks"][0]["findings_exit_codes"] = json!([101]);
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    let denied = run(root, 0);
    assert_eq!(denied["checks"][0]["execution"]["exit_code"], 101);
    policy["checks"][0]["argv"] = json!([
        "cargo",
        "clippy",
        "--locked",
        "--offline",
        "--message-format=json",
        "--lib",
        "--",
        "-W",
        "clippy::len_zero"
    ]);
    policy["checks"][0]["findings_exit_codes"] = json!([]);
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();

    fs::write(root.join("src/lib.rs"), format!("{OLD}{NEW}")).unwrap();
    let growth = run(root, 1);
    assert_eq!(growth["gate"]["decision"], "fail");
    assert_eq!(
        growth["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        growth["checks"][0]["diagnostics"][0]["evidence"]["rule"],
        "clippy::len_zero"
    );

    fs::write(
        root.join("src/lib.rs"),
        "pub fn empty(values: &[u8]) -> bool { values.is_empty() }\n",
    )
    .unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");

    fs::write(root.join("src/lib.rs"), "pub fn broken(\n").unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
    assert_eq!(incomplete["checks"][0]["execution"]["exit_code"], 101);
}
