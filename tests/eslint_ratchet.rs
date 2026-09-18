//! Actual ESLint JSON formatter in paired snapshots, separate from parser fixtures.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn run(root: &Path, exit: i32) -> Value {
    report(&cli(root, &["check", "--format", "json"]), exit)
}

#[test]
fn eslint_reference_policy_is_valid_and_rejects_unrecognized_formats() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/eslint-ratchet.yaml"),
    )
    .unwrap();
    let output = cli(root, &["config", "--show", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(
        root.join("qualitygate.yaml"),
        include_str!("../skills/qualitygate-cli/references/eslint-ratchet.yaml")
            .replace("eslint_json", "unsupported_eslint"),
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
#[ignore = "requires pinned ESLint and npm; mandatory live ESLint CI job"]
fn real_eslint_stdout_ratchet_growth_repair_and_fatal_parse() {
    let repository = fixture();
    let root = repository.path();
    let external = tempfile::tempdir().unwrap();
    let installation = Command::new("npm")
        .args(["install", "--prefix"])
        .arg(external.path())
        .args([
            "--no-audit",
            "--no-fund",
            "--ignore-scripts",
            "--package-lock=false",
            "eslint@10.10.0",
        ])
        .env("npm_config_cache", external.path().join("npm-cache"))
        .output()
        .unwrap();
    assert!(
        installation.status.success(),
        "{}",
        String::from_utf8_lossy(&installation.stderr)
    );
    let eslint = external.path().join("node_modules/.bin/eslint");
    let version = Command::new(&eslint).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), "v10.10.0");

    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/app.js"), "eval('one');\n").unwrap();
    fs::write(
        root.join("eslint.config.mjs"),
        "export default [{ files: ['src/**/*.js'], rules: { 'no-eval': 'error' } }];\n",
    )
    .unwrap();
    let policy = json!({"schema_version":1,"checks":[{
        "id":"eslint-ratchet","argv":[eslint,"--format=json","--exit-on-fatal-error","src"],
        "timeout_seconds":90,"findings_exit_codes":[1],
        "tools":[{"id":"eslint","argv":[eslint,"--version"]}],
        "reports":[{"path":"target/eslint.json","baseline":"target/eslint.json","from_stdout":true,"format":"eslint_json","mode":"ratchet"}]
    }]});
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical ESLint finding"]);

    let initial = run(root, 0);
    assert_eq!(
        initial["checks"][0]["metadata"]["target/eslint.json:ratchet"][0]["baseline"],
        1
    );
    assert_eq!(initial["checks"][0]["execution"]["exit_code"], 1);
    fs::write(root.join("src/app.js"), "eval('one');\neval('two');\n").unwrap();
    let growth = run(root, 1);
    assert_eq!(
        growth["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        growth["checks"][0]["diagnostics"][0]["evidence"]["rule"],
        "no-eval"
    );
    fs::write(root.join("src/app.js"), "export const value = 1;\n").unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");
    fs::write(root.join("src/app.js"), "const = ;\n").unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}
