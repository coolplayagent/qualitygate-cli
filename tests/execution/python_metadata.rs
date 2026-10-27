//! A bounded child CLI exercises malformed producer outputs without invoking pip.
use super::*;
use std::{process::Command, time::Duration};

#[tokio::test]
async fn special_python_metadata_is_incomplete_and_blocks_consumers_without_hanging() {
    let tools = tempfile::tempdir().unwrap();
    let source = tools.path().join("producer.rs");
    let producer = tools.path().join("producer");
    std::fs::write(
        &source,
        include_str!("../fixtures/python_metadata_producer.rs"),
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .args(["--edition=2024", "-o"])
        .arg(&producer)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let root = fixture();
    std::fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname='fixture'\nversion='1.0'\n",
    )
    .unwrap();
    std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
    policy(
        root.path(),
        json!([
            {"id":"independent","argv":["git","--version"]},
            {"id":"python-facts","argv":[producer,"-E","-P","-m","pip","--isolated","install","--ignore-installed","--target","target/python","--report","target/pip.json","."],"tools":[{"id":"python","argv":[producer,"-E","-P","--version"]},{"id":"pip","argv":[producer,"-E","-P","-m","pip","--version"]}],"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"target/python","install_report":"target/pip.json"}]},
            {"id":"consumer","argv":["git","--version"],"depends_on":["python-facts"]}
        ]),
    );
    let scratch = tempfile::tempdir().unwrap();
    for mode in [
        "regular",
        "fifo",
        "socket",
        "directory",
        "oversized",
        "regular",
    ] {
        std::fs::write(root.path().join("producer-mode"), mode).unwrap();
        // Isolate all child temporary paths so an old-code timeout leaves no
        // abandoned materialization outside this test's owned directory.
        let argv = vec![
            "env".into(),
            format!("TMPDIR={}", scratch.path().display()),
            env!("CARGO_BIN_EXE_qualitygate").into(),
            "check".into(),
            "--worktree".into(),
            "--output-dir".into(),
            tools.path().join("evidence").display().to_string(),
            "--format".into(),
            "json".into(),
        ];
        let output = qualitygate::runner::capture(&argv, root.path(), None, Duration::from_secs(8))
            .await
            .unwrap();
        assert!(
            !output.timed_out,
            "Python {mode} metadata stalled collection"
        );
        assert!(output.capture_error.is_none());
        assert_eq!(
            output.exit_code,
            Some(if mode == "regular" { 0 } else { 2 })
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let check = |id| {
            report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|check| check["id"] == id)
                .unwrap()
        };
        assert_eq!(check("independent")["verdict"], "pass");
        if mode == "regular" {
            assert_eq!(report["gate"]["complete"], true);
            assert_eq!(check("python-facts")["verdict"], "pass");
            assert_eq!(check("consumer")["verdict"], "pass");
        } else {
            assert_eq!(report["gate"]["complete"], false);
            assert_eq!(report["gate"]["decision"], "incomplete");
            let failed = check("python-facts");
            assert_eq!(failed["execution"]["status"], "tool_error");
            assert!(failed["verdict"].is_null());
            assert_eq!(failed["execution"]["exit_code"], 0);
            let reason = failed["execution"]["reason"].as_str().unwrap();
            assert!(
                reason.contains(if mode == "oversized" {
                    "size budget"
                } else {
                    "regular file"
                }),
                "{mode}: {reason}"
            );
            assert_eq!(check("consumer")["execution"]["status"], "blocked");
            let artifacts = failed["execution"]["artifacts"].as_array().unwrap();
            assert!(artifacts.iter().any(|artifact| {
                artifact["path"]
                    .as_str()
                    .unwrap()
                    .contains("project-0-install")
            }));
            for artifact in artifacts {
                let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
                assert_eq!(artifact["bytes"], bytes.len());
                assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
            }
        }
    }
}
