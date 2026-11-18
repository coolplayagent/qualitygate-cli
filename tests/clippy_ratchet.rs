//! Actual Cargo Clippy JSON Lines in paired snapshots, separate from parser fixtures.
mod common;
#[path = "common/repository.rs"]
mod repository;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const OLD: &str = "pub fn empty(values: &[u8]) -> bool { values.len() == 0 }\n";
const NEW: &str = "pub fn other(values: &[u8]) -> bool { values.len() == 0 }\n";
const DEPRECATION_POLICY: &str =
    include_str!("../skills/qualitygate-cli/references/rust-deprecation.yaml");

// These cases measure historical repository debt; delivery intersection is tested separately.
fn run(root: &Path, exit: i32) -> Value {
    report(&repository::check(root, &[]), exit)
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

fn cargo_fixture(root: &Path, source: &str) {
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), source).unwrap();
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
}

#[test]
#[ignore = "requires Clippy; mandatory live Clippy CI job"]
fn real_clippy_stdout_ratchet_repair_and_incomplete_compilation() {
    let repository = fixture();
    let root = repository.path();
    cargo_fixture(root, OLD);
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

#[test]
fn packaged_rust_deprecation_policy_is_valid() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), DEPRECATION_POLICY).unwrap();
    report(&cli(root, &["config", "--show", "--format", "json"]), 0);
}

#[test]
#[ignore = "requires Clippy; mandatory live Clippy CI job"]
fn real_clippy_deprecation_blocks_unchanged_call_sites_despite_local_allow() {
    let repository = fixture();
    let root = repository.path();
    cargo_fixture(
        root,
        "mod api;\n#[allow(deprecated)]\npub fn use_api() { let unrelated_unused = 1; api::legacy(); }\n",
    );
    let api = root.join("src/api.rs");
    fs::write(&api, "pub fn legacy() {}\n").unwrap();
    fs::write(root.join("qualitygate.yaml"), DEPRECATION_POLICY).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "non-deprecated Rust declaration"]);
    let check = |status| {
        report(
            &cli(
                root,
                &[
                    "check",
                    "--worktree",
                    "--profile",
                    "full",
                    "--format",
                    "json",
                ],
            ),
            status,
        )
    };
    assert_eq!(check(0)["gate"]["decision"], "pass");

    fs::write(&api, "#[deprecated]\npub fn legacy() {}\n").unwrap();
    let blocked = check(1);
    assert_eq!(blocked["selection"]["changed_files"], json!(["src/api.rs"]));
    assert_eq!(blocked["gate"]["complete"], true);
    let result = &blocked["checks"][0];
    assert_eq!(result["execution"]["exit_code"], 0);
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert_eq!(diagnostic["file"], "src/lib.rs");
        assert_eq!(diagnostic["range"]["start_line"], 3);
        assert_eq!(diagnostic["evidence"]["rule"], "deprecated");
    }

    fs::write(&api, "pub fn broken(\n").unwrap();
    assert_eq!(check(2)["gate"]["decision"], "incomplete");
    fs::write(&api, "pub fn legacy() {}\n").unwrap();
    assert_eq!(check(0)["gate"]["decision"], "pass");
}
