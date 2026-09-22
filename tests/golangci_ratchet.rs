//! Actual golangci-lint v2 JSON across paired Go snapshots.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

// These cases measure historical repository debt; delivery intersection is tested separately.
fn run(root: &Path, cache: &Path, exit: i32) -> Value {
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
        .env("GOLANGCI_LINT_CACHE", cache.join("lint"))
        .env("GOCACHE", cache.join("go-build"))
        .env("GOPATH", cache.join("go-path"))
        .env("GOTELEMETRY", "off")
        .arg("--root")
        .arg(root)
        .args(["check", "--scope", "repository", "--format", "json"])
        .output()
        .unwrap();
    report(&output, exit)
}

#[test]
fn golangci_reference_policy_is_valid_and_unknown_format_fails() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/golangci-lint-ratchet.yaml"),
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
        include_str!("../skills/qualitygate-cli/references/golangci-lint-ratchet.yaml")
            .replace("golangci_json", "unknown_golangci"),
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
#[ignore = "requires pinned Go and golangci-lint v2; mandatory live Go CI job"]
fn real_golangci_stdout_ratchet_growth_repair_and_typecheck_failure() {
    let binary =
        std::env::var("GOLANGCI_LINT_BIN").expect("Set GOLANGCI_LINT_BIN to pinned v2 binary");
    let version = Command::new(&binary).arg("version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("version 2.13.2"));

    let repository = fixture();
    let root = repository.path();
    let cache = tempfile::tempdir().unwrap();
    fs::write(
        root.join("go.mod"),
        "module example.com/qualitygate\n\ngo 1.26\n",
    )
    .unwrap();
    fs::write(
        root.join(".golangci.yml"),
        "version: '2'\nlinters:\n  default: none\n  enable: [govet]\n",
    )
    .unwrap();
    fs::write(
        root.join("main.go"),
        "package main\nimport \"fmt\"\nfunc main() { fmt.Printf(\"%d\", \"wrong\") }\n",
    )
    .unwrap();
    let policy = json!({"schema_version":1,"checks":[{
        "id":"golangci-lint-ratchet",
        "argv":[binary,"run","--output.json.path=stdout","--output.text.path=stderr",
            "--show-stats=false","--max-issues-per-linter=0","--max-same-issues=0",
            "--uniq-by-line=false","--modules-download-mode=readonly","--timeout=90s","./..."],
        "timeout_seconds":120,"findings_exit_codes":[1],
        "tools":[{"id":"golangci-lint","argv":[binary,"version"]}],
        "reports":[{"path":"target/golangci-lint.json","baseline":"target/golangci-lint.json",
            "from_stdout":true,"format":"golangci_json","mode":"ratchet"}]
    }]});
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical Go lint finding"]);

    let initial = run(root, cache.path(), 0);
    assert_eq!(initial["checks"][0]["execution"]["exit_code"], 1);
    assert_eq!(
        initial["checks"][0]["metadata"]["target/golangci-lint.json:ratchet"][0]["baseline"],
        1
    );
    fs::write(root.join("main.go"), "package main\nimport \"fmt\"\nfunc main() { fmt.Printf(\"%d\", \"wrong\"); fmt.Printf(\"%d\", \"also wrong\") }\n").unwrap();
    let growth = run(root, cache.path(), 1);
    assert_eq!(
        growth["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        growth["checks"][0]["diagnostics"][0]["evidence"]["tool"],
        "govet"
    );
    fs::write(root.join("main.go"), "package main\nfunc main() {}\n").unwrap();
    assert_eq!(run(root, cache.path(), 0)["gate"]["decision"], "pass");
    fs::write(
        root.join("main.go"),
        "package main\nfunc main() { missing }\n",
    )
    .unwrap();
    let incomplete = run(root, cache.path(), 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}
