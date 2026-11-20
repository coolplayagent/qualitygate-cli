mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn policy(root: &Path, policy: Value) {
    write(root, "qualitygate.yaml", policy.to_string());
}

fn check(root: &Path, selector: &[&str], code: i32) -> Value {
    let mut args = vec!["check", "--profile", "full", "--format", "json"];
    args.extend_from_slice(selector);
    report(&cli(root, &args), code)
}

fn outcome<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("Missing check {id} in {report}"))
}

fn finding(diagnostic: &Value, path: &str, kind: &str, extension: &str, bytes: usize) {
    assert_eq!(diagnostic["file"], path);
    assert_eq!(diagnostic["range"], Value::Null);
    assert_eq!(diagnostic["evidence"]["kind"], kind);
    assert_eq!(diagnostic["evidence"]["extension"], extension);
    assert_eq!(diagnostic["evidence"]["bytes"], bytes);
}

#[test]
fn artifact_rules_are_discoverable_and_opt_in() {
    let repo = fixture();
    let root = repo.path();
    let listed = report(
        &cli(
            root,
            &["rules", "list", "--source", "builtin", "--format", "json"],
        ),
        0,
    );
    for (id, severity) in [
        ("no-committed-binaries", "error"),
        ("oversized-added-file", "warning"),
    ] {
        let rule = listed["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|rule| rule["id"] == id)
            .unwrap();
        let builtin = &rule["definition"]["builtin"];
        assert_eq!(builtin["implementation"], "artifact-hygiene");
        assert_eq!(builtin["requires_capabilities"], json!(["files"]));
        assert!(rule["language"].as_array().unwrap().is_empty());
        assert_eq!(rule["enabled"], false);
        assert_eq!(builtin["defaults"]["enabled"], false);
        assert_eq!(builtin["defaults"]["required"], false);
        assert_eq!(builtin["defaults"]["severity"], severity);
    }

    policy(root, json!({"schema_version":1,"rules":{"line-ending":{}}}));
    write(root, "lib/default-off.so", vec![0; 1024 * 1024 + 1]);
    let unchecked = check(root, &["--worktree"], 0);
    assert_eq!(unchecked["checks"].as_array().unwrap().len(), 1);
    assert_eq!(unchecked["checks"][0]["id"], "line-ending");
}

#[test]
fn binary_extension_and_content_findings_block_added_files() {
    let repo = fixture();
    let root = repo.path();
    policy(
        root,
        json!({"schema_version":1,"rules":{"no-committed-binaries":{"enabled":true}}}),
    );
    write(root, "src/a.rs", "fn main() {}\n");
    let clean = check(root, &["--worktree"], 0);
    assert_eq!(
        outcome(&clean, "no-committed-binaries")["diagnostics"],
        json!([])
    );

    write(root, "lib/x.SO", [0, 1]);
    write(root, "src/data.bin", b"abc\0def");
    let blocked = check(root, &["--worktree"], 1);
    assert_eq!(blocked["gate"]["decision"], "fail");
    assert_eq!(blocked["gate"]["complete"], true);
    let rule = outcome(&blocked, "no-committed-binaries");
    assert_eq!(rule["required"], false);
    let diagnostics = rule["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    finding(&diagnostics[0], "lib/x.SO", "binary_extension", ".so", 2);
    finding(&diagnostics[1], "src/data.bin", "binary_content", ".bin", 7);
}

#[test]
fn retained_media_obeys_rule_local_allow_paths() {
    let repo = fixture();
    let root = repo.path();
    let mut settings =
        json!({"schema_version":1,"rules":{"no-committed-binaries":{"enabled":true}}});
    let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    write(root, "fixtures/sample.png", png);
    policy(root, settings.clone());
    let excluded = check(root, &["--worktree"], 0);
    assert_eq!(
        excluded["selection"]["resource_exclusions"]["default_excluded_paths"],
        json!(["fixtures/sample.png"])
    );

    settings["unstructured_documents"] = json!({"retain":["fixtures/**"]});
    policy(root, settings.clone());
    let visible = check(root, &["--worktree"], 1);
    assert_eq!(
        visible["selection"]["resource_exclusions"]["default_excluded_count"],
        0
    );
    finding(
        &outcome(&visible, "no-committed-binaries")["diagnostics"][0],
        "fixtures/sample.png",
        "binary_content",
        ".png",
        png.len(),
    );

    settings["rules"]["no-committed-binaries"]["parameters"] =
        json!({"allow_paths":["fixtures/**"]});
    settings["rules"]["oversized-added-file"] =
        json!({"enabled":true,"parameters":{"max_added_bytes":4}});
    // Put the policy in the base so only the retained PNG can trigger the size rule.
    policy(root, settings);
    git(root, &["add", "qualitygate.yaml"]);
    git(root, &["commit", "-qm", "configured artifact checks"]);
    let allowed = check(root, &["--worktree"], 0);
    assert_eq!(
        outcome(&allowed, "no-committed-binaries")["diagnostics"],
        json!([])
    );
    let size_findings = outcome(&allowed, "oversized-added-file")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(size_findings.len(), 1);
    finding(
        &size_findings[0],
        "fixtures/sample.png",
        "oversized_added_file",
        ".png",
        png.len(),
    );
}

#[test]
fn size_warnings_are_distinct_from_snapshot_limits() {
    let repo = fixture();
    let root = repo.path();
    policy(
        root,
        json!({"schema_version":1,"rules":{"oversized-added-file":{"enabled":true}}}),
    );
    let mib = 1024 * 1024;
    write(root, "exact.txt", vec![b'a'; mib]);
    write(root, "big.jar", vec![0; mib + 1]);
    let warning = check(root, &["--worktree"], 0);
    assert_eq!(warning["gate"]["decision"], "pass");
    let rule = outcome(&warning, "oversized-added-file");
    assert_eq!(rule["severity"], "warning");
    let diagnostics = rule["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    finding(
        &diagnostics[0],
        "big.jar",
        "oversized_added_file",
        ".jar",
        mib + 1,
    );

    write(root, "big.jar", vec![0; 2 * mib + 1]);
    let incomplete = check(root, &["--worktree"], 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["gate"]["complete"], false);
    assert!(!incomplete["checks"].as_array().is_some_and(|checks| {
        checks
            .iter()
            .any(|check| check["id"] == "oversized-added-file")
    }));
    let doctor = report(
        &cli(
            root,
            &[
                "doctor",
                "--profile",
                "full",
                "--worktree",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        doctor["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "snapshot.file_limit")
    );
}

#[test]
fn only_added_snapshot_paths_are_classified() {
    let repo = fixture();
    let root = repo.path();
    policy(
        root,
        json!({"schema_version":1,"rules":{"no-committed-binaries":{"enabled":true,"required":true}}}),
    );
    let artifact = b"\0compiled";
    write(root, "lib/original.so", artifact);
    write(root, "lib/modified.so", b"\0old");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical binaries"]);
    fs::rename(root.join("lib/original.so"), root.join("lib/a-renamed.so")).unwrap();
    write(root, "lib/modified.so", b"\0new");
    assert_eq!(
        outcome(&check(root, &["--worktree"], 0), "no-committed-binaries")["diagnostics"],
        json!([])
    );

    write(root, "lib/b-copy.bin", artifact);
    git(root, &["add", "."]);
    write(root, "lib/b-copy.bin", "a text worktree replacement\n");
    check(root, &["--worktree"], 0);
    let staged = check(root, &["--staged"], 1);
    git(root, &["commit", "-qm", "staged artifact copy"]);
    let diff = check(root, &["--diff", "HEAD~1..HEAD"], 1);
    for result in [&staged, &diff] {
        let diagnostics = outcome(result, "no-committed-binaries")["diagnostics"]
            .as_array()
            .unwrap();
        assert_eq!(diagnostics.len(), 1);
        finding(
            &diagnostics[0],
            "lib/b-copy.bin",
            "binary_content",
            ".bin",
            artifact.len(),
        );
    }
}
