//! Actual golangci-lint v2 JSON across paired Go snapshots.
//! Each live fixture isolates the runner lock in its own temporary directory.
mod common;
#[path = "common/repository.rs"]
mod repository;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const DEPRECATION_POLICY: &str =
    include_str!("../skills/qualitygate-cli/references/go-deprecation.yaml");
const DEPRECATION_LINTER: &str =
    include_str!("../skills/qualitygate-cli/references/golangci-deprecation.yaml");

// These cases measure historical repository debt; delivery intersection is tested separately.
fn run(root: &Path, exit: i32) -> Value {
    let output = repository::check(root, &[]);
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

fn pinned_golangci_binary() -> String {
    let binary =
        std::env::var("GOLANGCI_LINT_BIN").expect("Set GOLANGCI_LINT_BIN to pinned v2 binary");
    let version = Command::new(&binary).arg("version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("version 2.13.2"));
    binary
}

#[test]
#[ignore = "requires pinned Go and golangci-lint v2; mandatory live Go CI job"]
fn real_golangci_stdout_ratchet_growth_repair_and_typecheck_failure() {
    let binary = pinned_golangci_binary();

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
        "argv":["env",format!("TMPDIR={}",cache.path().display()),
            format!("GOLANGCI_LINT_CACHE={}",cache.path().join("lint").display()),
            format!("GOCACHE={}",cache.path().join("go-build").display()),
            format!("GOPATH={}",cache.path().join("go-path").display()),"GOTELEMETRY=off",
            binary,"run","--output.json.path=stdout","--output.text.path=stderr",
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

    let initial = run(root, 0);
    assert_eq!(initial["checks"][0]["execution"]["exit_code"], 1);
    assert_eq!(
        initial["checks"][0]["metadata"]["target/golangci-lint.json:ratchet"][0]["baseline"],
        1
    );
    fs::write(root.join("main.go"), "package main\nimport \"fmt\"\nfunc main() { fmt.Printf(\"%d\", \"wrong\"); fmt.Printf(\"%d\", \"also wrong\") }\n").unwrap();
    let growth = run(root, 1);
    assert_eq!(
        growth["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        growth["checks"][0]["diagnostics"][0]["evidence"]["tool"],
        "govet"
    );
    fs::write(root.join("main.go"), "package main\nfunc main() {}\n").unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");
    fs::write(
        root.join("main.go"),
        "package main\nfunc main() { missing }\n",
    )
    .unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}

#[test]
fn packaged_go_deprecation_policy_is_valid() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), DEPRECATION_POLICY).unwrap();
    report(&cli(root, &["config", "--show", "--format", "json"]), 0);
}

#[test]
#[ignore = "requires pinned Go and golangci-lint v2; mandatory live Go CI job"]
fn real_golangci_deprecation_blocks_unchanged_cross_package_callers() {
    let binary = pinned_golangci_binary();
    let repository = fixture();
    let root = repository.path();
    let cache = tempfile::tempdir().unwrap();
    fs::create_dir(root.join("config")).unwrap();
    fs::create_dir(root.join("api")).unwrap();
    fs::write(
        root.join("config/golangci-deprecation.yaml"),
        DEPRECATION_LINTER,
    )
    .unwrap();
    fs::write(
        root.join("go.mod"),
        "module example.com/qualitygate\n\ngo 1.26\n",
    )
    .unwrap();
    let api = root.join("api/legacy.go");
    let use_site = root.join("main.go");
    let source = r#"package main

import (
    "fmt"

    "example.com/qualitygate/api"
)

func main() {
    api.Legacy()
    fmt.Printf("%d", "unrelated")
}
"#;
    fs::write(&use_site, source).unwrap();
    fs::write(
        &api,
        "package api\n\n// Legacy is the original API.\nfunc Legacy() {}\n\nfunc Current() {}\n",
    )
    .unwrap();
    let mut policy: Value = serde_norway::from_str(DEPRECATION_POLICY).unwrap();
    let check = &mut policy["checks"][0];
    let argv = check["argv"].as_array_mut().unwrap();
    argv[0] = json!(binary);
    argv.splice(
        0..0,
        [
            json!("env"),
            json!(format!("TMPDIR={}", cache.path().display())),
            json!(format!(
                "GOLANGCI_LINT_CACHE={}",
                cache.path().join("lint").display()
            )),
            json!(format!(
                "GOCACHE={}",
                cache.path().join("go-build").display()
            )),
            json!(format!("GOPATH={}", cache.path().join("go-path").display())),
            json!("GOTELEMETRY=off"),
            json!("GOTOOLCHAIN=local"),
        ],
    );
    check["tools"][1]["argv"][0] = json!(binary);
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "non-deprecated Go API"]);

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

    fs::write(
        &api,
        "package api\n\n// Legacy is the original API.\n//\n// Deprecated: Use Current instead.\nfunc Legacy() {}\n\nfunc Current() {}\n",
    )
    .unwrap();
    let blocked = check(1);
    assert_eq!(
        blocked["selection"]["changed_files"],
        json!(["api/legacy.go"])
    );
    assert_eq!(blocked["gate"]["complete"], true);
    let result = &blocked["checks"][0];
    assert_eq!(result["execution"]["exit_code"], 1);
    let tools = result["metadata"]["tools"].as_array().unwrap();
    for (id, input) in [
        ("go", "go.mod"),
        ("golangci-lint", "config/golangci-deprecation.yaml"),
    ] {
        let evidence = tools.iter().find(|tool| tool["id"] == id).unwrap();
        assert!(evidence["inputs"][input].is_string());
        assert_eq!(
            evidence["snapshot_digest"],
            blocked["snapshot"]["content_digest"]
        );
    }
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    let finding = &diagnostics[0];
    assert_eq!(finding["file"], "main.go");
    assert_eq!(finding["range"]["start_line"], 10);
    assert_eq!(finding["evidence"]["rule"], "SA1019");
    assert_eq!(finding["evidence"]["tool"], "staticcheck");

    fs::write(&use_site, source.replace("api.Legacy()", "api.Current()")).unwrap();
    assert_eq!(check(0)["gate"]["decision"], "pass");
    fs::write(&use_site, source.replace("api.Legacy()", "api.Missing()")).unwrap();
    let incomplete = check(2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert!(incomplete["checks"][0]["verdict"].is_null());
}
