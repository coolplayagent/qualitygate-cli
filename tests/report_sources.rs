mod common;
use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn unmapped(root: &Path, input: &[u8], policy: &[u8]) {
    let checked = report(&cli(root, &["check", "--worktree", "--format", "json"]), 2);
    assert_eq!(checked["gate"]["decision"], "incomplete");
    assert_eq!(checked["checks"][0]["verdict"], Value::Null);
    let execution = &checked["checks"][0]["execution"];
    assert!(
        execution["reason"]
            .as_str()
            .unwrap()
            .contains("Report path cannot be mapped uniquely")
    );
    let raw = execution["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
        .unwrap();
    assert_eq!(fs::read(raw["path"].as_str().unwrap()).unwrap(), input);
    assert_eq!(raw["digest"], qualitygate::snapshot::digest(input));
    assert_eq!(fs::read(root.join("input.txt")).unwrap(), input);
    assert_eq!(fs::read(root.join("qualitygate.yaml")).unwrap(), policy);
}

#[test]
fn diagnostics_and_coverage_share_unique_source_resolution_and_recover_from_ambiguity() {
    let root = fixture();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("driver.rs");
    let driver = external.path().join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    fs::write(
        &source,
        r#"
fn main() {
    if std::env::args().any(|arg| arg == "--version") { println!("Source fixture 1"); return; }
    std::fs::copy("input.txt", "report.out").unwrap();
}
"#,
    )
    .unwrap();
    let built = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&driver)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    fs::create_dir(root.path().join("src")).unwrap();
    fs::create_dir(root.path().join("tests")).unwrap();
    for name in ["one.rs", "two.rs"] {
        fs::write(root.path().join("src").join(name), "old\nstable\n").unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "source mapping fixture"]);
    fs::write(root.path().join("src/one.rs"), "changed\nstable\n").unwrap();
    let configure = |spec: Value| {
        let bytes = serde_json::to_vec(&json!({"schema_version":1,"checks":[{
            "id":"static","argv":[driver],"tools":[{"id":"scan","argv":[driver,"--version"]}],"reports":[spec]
        }]})).unwrap();
        fs::write(root.path().join("qualitygate.yaml"), &bytes).unwrap();
        bytes
    };
    let run = |code| {
        report(
            &cli(root.path(), &["check", "--worktree", "--format", "json"]),
            code,
        )
    };
    let policy = configure(json!({"path":"report.out","format":"diagnostics","mode":"full"}));
    let finding = |file: &str| json!({"rule":"R","file":file,"line":1,"message":"finding"});
    let mut findings = vec![finding("two.rs"); 200];
    findings.push(finding("one.rs"));
    let input = serde_json::to_vec(&json!({"issues":findings})).unwrap();
    fs::write(root.path().join("input.txt"), &input).unwrap();
    let selected = run(1);
    assert_eq!(
        selected["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        selected["checks"][0]["diagnostics"][0]["file"],
        "src/one.rs"
    );
    assert_eq!(
        selected["checks"][0]["metadata"]["report.out:delivery_filtered"],
        200
    );
    fs::write(root.path().join("tests/one.rs"), "other\n").unwrap();
    unmapped(root.path(), &input, &policy);
    fs::write(
        root.path().join("input.txt"),
        serde_json::to_vec(&json!({"issues":[finding("src/one.rs")]})).unwrap(),
    )
    .unwrap();
    assert_eq!(run(1)["checks"][0]["diagnostics"][0]["file"], "src/one.rs");
    fs::remove_file(root.path().join("tests/one.rs")).unwrap();
    fs::write(root.path().join("input.txt"), b"{\"issues\":[]}").unwrap();
    assert_eq!(run(0)["gate"]["decision"], "pass");

    let policy = configure(
        json!({"path":"report.out","format":"lcov","mode":"full", "coverage_paths":["src/one.rs"],"require_branch_coverage":false,"minimum_coverage":100}),
    );
    let input = b"SF:one.rs\nDA:1,1\nLF:1\nLH:1\nend_of_record\n";
    fs::write(root.path().join("input.txt"), input).unwrap();
    let covered = run(0);
    assert_eq!(
        covered["checks"][0]["metadata"]["report.out:coverage"]["source_files"],
        json!(["src/one.rs"])
    );
    fs::write(root.path().join("tests/one.rs"), "other\n").unwrap();
    unmapped(root.path(), input, &policy);
    fs::write(
        root.path().join("input.txt"),
        b"SF:src/one.rs\nDA:1,1\nLF:1\nLH:1\nend_of_record\n",
    )
    .unwrap();
    assert_eq!(run(0)["gate"]["decision"], "pass");
}
