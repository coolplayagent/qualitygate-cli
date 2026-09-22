mod common;
#[path = "common/repository.rs"]
mod repository;
use common::*;
use serde_json::json;
use std::{path::Path, process::Command};

fn write(root: &Path, name: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
}

fn policy(root: &Path, exclude: &[&str]) {
    write(
        root,
        "qualitygate.yaml",
        json!({"schema_version":1,"exclude":exclude,"rules":{"line-ending":{}}}).to_string(),
    );
}

#[test]
fn cli_infers_delivery_from_selectors_and_rechecks_without_a_scope_option() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    commit(root);
    write(root, "hello.txt", "changed\n");
    git(root, &["add", "hello.txt"]);
    for selector in [
        vec![],
        vec!["--worktree"],
        vec!["--staged"],
        vec!["--path", "hello.txt"],
    ] {
        let mut args = vec!["check", "--format", "json"];
        args.extend(selector);
        let checked = report(&cli(root, &args), 0);
        assert_eq!(checked["selection"]["mode"], "delivery");
        assert_eq!(checked["selection"]["changed_files"], json!(["hello.txt"]));
        let argv = checked["context"]["recheck"]["argv"].as_array().unwrap();
        assert!(!argv.contains(&json!("--scope")));
        let replay: Vec<_> = argv[3..]
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        let rechecked = report(&cli(root, &replay), 0);
        assert_eq!(
            checked["snapshot"]["verification_digest"],
            rechecked["snapshot"]["verification_digest"]
        );
    }
    commit(root);
    let diff = report(
        &cli(
            root,
            &["check", "--diff", "HEAD~1..HEAD", "--format", "json"],
        ),
        0,
    );
    assert_eq!(diff["selection"]["mode"], "delivery");
    assert_eq!(diff["selection"]["changed_files"], json!(["hello.txt"]));
    for removed in ["delivery", "repository"] {
        let rejected = cli(root, &["check", "--scope", removed]);
        assert_eq!(rejected.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("unexpected argument '--scope'")
        );
    }
    let help = cli(root, &["check", "--help"]);
    assert!(help.status.success());
    assert!(!String::from_utf8_lossy(&help.stdout).contains("--scope"));
}

#[test]
fn exclusion_precedes_content_limits_and_binds_delivery_and_repository_inputs() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &["legacy/**"]);
    write(root, "legacy/demo.bin", vec![0; 9 * 1024 * 1024]);
    commit(root);
    write(root, "hello.txt", "changed\n");
    let preflight = report(&cli(root, &["init", "--format", "json"]), 0);
    assert_eq!(preflight["snapshot_preflight"]["excluded_file_count"], 1);
    assert_eq!(preflight["snapshot_preflight"]["oversized_file_count"], 0);
    let delivered = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(delivered["scope"], "delivery");
    assert_eq!(
        delivered["selection"]["excluded_paths"],
        json!(["legacy/demo.bin"])
    );
    assert_eq!(
        delivered["selection"]["changed_files"],
        json!(["hello.txt"])
    );
    assert_eq!(delivered["selection"]["changed_lines"], 1);
    let whole = report(&repository::check(root, &[]), 0);
    assert_ne!(
        delivered["snapshot"]["verification_digest"],
        whole["snapshot"]["verification_digest"]
    );
    assert_eq!(
        delivered["selection"]["execution_context_digest"],
        whole["selection"]["execution_context_digest"]
    );
    assert!(
        !delivered["context"]["recheck"]["argv"]
            .as_array()
            .unwrap()
            .contains(&json!("--scope"))
    );
    assert_eq!(whole["context"]["recheck"]["argv"], json!([]));
    assert!(
        !whole["context"]["delivery_recheck"]["argv"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    policy(root, &[]);
    report(&cli(root, &["check", "--format", "json"]), 2);
}

#[test]
fn staged_and_diff_use_selected_policy_not_dirty_exclusions() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    write(root, "legacy/demo.bin", vec![0; 3 * 1024 * 1024]);
    commit(root);
    policy(root, &["legacy/**"]);
    report(&cli(root, &["check", "--format", "json"]), 0);
    report(&cli(root, &["check", "--staged", "--format", "json"]), 2);
    report(
        &cli(
            root,
            &["check", "--diff", "HEAD~1..HEAD", "--format", "json"],
        ),
        2,
    );
    git(root, &["add", "qualitygate.yaml"]);
    report(&cli(root, &["check", "--staged", "--format", "json"]), 0);
    commit(root);
    policy(root, &[]);
    report(
        &cli(
            root,
            &["check", "--diff", "HEAD~1..HEAD", "--format", "json"],
        ),
        0,
    );
    report(
        &cli(
            root,
            &["check", "--policy-ref", "HEAD~1", "--format", "json"],
        ),
        2,
    );
}

#[test]
fn excluded_policy_and_verification_assets_are_rejected() {
    let temp = fixture();
    let root = temp.path();
    for pattern in [
        "qualitygate.yaml",
        "../legacy/**",
        "!legacy/**",
        "/legacy/**",
        "C:/legacy/**",
    ] {
        policy(root, &[pattern]);
        report(&cli(root, &["check", "--format", "json"]), 2);
    }
    write(root, "protected.txt", "retain me\n");
    write(
        root,
        "qualitygate.yaml",
        "schema_version: 1\nexclude: ['protected.txt']\nverification_assets: ['protected.txt']\n",
    );
    let rejected = report(&cli(root, &["check", "--format", "json"]), 2);
    assert!(
        rejected
            .to_string()
            .contains("protected verification input")
    );
    let preflight = report(&cli(root, &["init", "--format", "json"]), 0);
    assert_eq!(preflight["snapshot_preflight"]["complete"], false);
    assert!(
        preflight["snapshot_preflight"]["reason"]
            .to_string()
            .contains("protected verification input")
    );
}

#[test]
fn delivery_filters_historical_lines_but_retains_context_and_command_failures() {
    let temp = fixture();
    let root = temp.path();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("driver.rs");
    let binary = external.path().join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    std::fs::write(
        &source,
        r#"fn main() {
        if std::env::args().any(|a| a == "--version") { println!("delivery-fixture 1"); return; }
        assert_eq!(std::fs::read_to_string("context.txt").unwrap(), "dependency\n");
        assert!(!std::path::Path::new("legacy/demo.bin").exists());
        println!("{}", std::fs::read_to_string("input.report").unwrap());
        if std::env::args().any(|a| a == "fail") { std::process::exit(9); }
    }"#,
    )
    .unwrap();
    let built = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let mut config = json!({"schema_version":1,"exclude":["legacy/**"],"checks":[{"id":"analyzer","argv":[binary],"tools":[{"id":"producer","argv":[binary,"--version"]}],"reports":[{"path":"stdout.json","format":"diagnostics","from_stdout":true}]}]});
    write(root, "qualitygate.yaml", config.to_string());
    write(root, "src.rs", "historical\nold\n");
    write(root, "context.txt", "dependency\n");
    write(root, "legacy/demo.bin", vec![0; 3 * 1024 * 1024]);
    let finding = |line| json!({"rule":"demo","file":"src.rs","line":line,"message":"finding"});
    write(
        root,
        "input.report",
        json!({"issues":[finding(1)]}).to_string(),
    );
    commit(root);
    write(root, "src.rs", "historical\nnew\n");
    let selected = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(
        selected["checks"][0]["metadata"]["stdout.json:delivery_filtered"],
        1
    );
    report(&repository::check(root, &[]), 1);
    write(
        root,
        "input.report",
        json!({"issues":[finding(1),finding(2)]}).to_string(),
    );
    let failed = report(&cli(root, &["check", "--format", "json"]), 1);
    assert_eq!(
        failed["checks"][0]["diagnostics"].as_array().unwrap().len(),
        1
    );
    write(root, "input.report", json!({"issues":[]}).to_string());
    config["checks"][0]["argv"]
        .as_array_mut()
        .unwrap()
        .push(json!("fail"));
    write(root, "qualitygate.yaml", config.to_string());
    let incomplete = report(&cli(root, &["check", "--format", "json"]), 2);
    assert!(
        incomplete
            .to_string()
            .contains("unexpected analyzer exit code")
    );
    let reports = config["checks"][0]
        .as_object_mut()
        .unwrap()
        .remove("reports")
        .unwrap();
    write(root, "qualitygate.yaml", config.to_string());
    let failed = report(&cli(root, &["check", "--format", "json"]), 1);
    assert!(failed.to_string().contains("Command exited"));
    config["checks"][0]["reports"] = reports;
    config["exclude"]
        .as_array_mut()
        .unwrap()
        .push(json!("context.txt"));
    write(root, "qualitygate.yaml", config.to_string());
    let incomplete = report(&cli(root, &["check", "--format", "json"]), 2);
    assert_eq!(incomplete["gate"]["complete"], false);
}

#[test]
fn deleted_renamed_and_binary_changes_are_reported_without_invented_lines() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    write(root, "old.txt", "moved\n");
    write(root, "binary.bin", [0, 1]);
    commit(root);
    std::fs::rename(root.join("old.txt"), root.join("new.txt")).unwrap();
    std::fs::remove_file(root.join("hello.txt")).unwrap();
    write(root, "binary.bin", [0, 2]);
    let checked = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(
        checked["selection"]["changed_files"],
        json!(["binary.bin", "hello.txt", "new.txt"])
    );
    assert_eq!(checked["selection"]["empty_delivery"], false);
    commit(root);
    let empty = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(empty["selection"]["empty_delivery"], true);
    assert!(
        empty["verification"]["known_limits"]
            .to_string()
            .contains("no unexcluded changed files")
    );
}
