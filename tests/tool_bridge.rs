//! Independent executable contract: conversion never runs tools or claims a gate pass.

use std::{path::Path, process::Command};

fn bridge(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(
        Path::new(env!("CARGO_BIN_EXE_qualitygate-tool-bridge"))
            .canonicalize()
            .unwrap(),
    )
    .current_dir(root)
    .args(args)
    .output()
    .unwrap()
}

#[test]
fn help_and_version_need_no_repository_and_unsupported_tools_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    for option in ["--help", "--version"] {
        let output = bridge(directory.path(), &[option]);
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("qualitygate-tool-bridge")
        );
    }
    let output = bridge(directory.path(), &["--format", "mutator"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn conversion_errors_are_machine_readable_and_leave_original_inputs_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("raw.edn"), b"{}").unwrap();
    let invalid = br#"{"argv":["unexpected-command"],"complete":true}"#;
    std::fs::write(root.join("context.json"), invalid).unwrap();
    let output = bridge(
        root,
        &[
            "--format",
            "dryer",
            "--input",
            "raw.edn",
            "--context",
            "context.json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "incomplete");
    assert!(value.get("gate").is_none());
    assert_eq!(std::fs::read(root.join("context.json")).unwrap(), invalid);
    assert_eq!(std::fs::read(root.join("raw.edn")).unwrap(), b"{}");
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 2);
}

#[test]
fn offline_conversion_produces_unbound_facts_even_with_a_claimed_success_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("report.edn"), b"{:candidates []}").unwrap();
    let context = serde_json::json!({
        "spec":{"adapter":"dryer","schema":"clone_pair","path":"report.edn","tool_id":"dryer",
            "tool_version":"66ff6d21a42c04afcad89c78a80066176d1294b0","scope":{"project":"fixture"},
            "paths":["src/lib.rs"],"crate_name":"fixture"},
        "producer":"clones","snapshot_digest":format!("sha256:{}", "0".repeat(64)),
        "artifact_digest":"caller claim","inventory":{"files":["src/lib.rs"],"entities":[]},"coverage":null,
        "execution":{"argv":["never-run"],"exit_code":0,"stderr":"","tool_id":"dryer",
            "tool_version":"66ff6d21a42c04afcad89c78a80066176d1294b0","scope_files":["src/lib.rs"]}
    });
    std::fs::write(
        root.join("context.json"),
        serde_json::to_vec(&context).unwrap(),
    )
    .unwrap();
    let output = bridge(
        root,
        &[
            "--format",
            "dryer",
            "--input",
            "report.edn",
            "--context",
            "context.json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["complete"], false);
    assert_eq!(value["protocol_version"], 1);
    assert_eq!(value["records"], serde_json::json!([]));
    assert_ne!(value["artifact_digest"], "caller claim");
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 2);
}
