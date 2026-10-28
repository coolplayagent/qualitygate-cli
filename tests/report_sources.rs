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

fn driver() -> (tempfile::TempDir, std::path::PathBuf) {
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
    (external, driver)
}

#[test]
fn diagnostics_and_coverage_share_unique_source_resolution_and_recover_from_ambiguity() {
    let root = fixture();
    let (_external, driver) = driver();
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

#[test]
fn renamed_baselines_validate_original_lines_preserve_evidence_and_recover() {
    let root = fixture();
    let (_external, driver) = driver();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/old.rs"), "old\nstable\nlast\n").unwrap();
    fs::write(root.path().join("src/untouched.rs"), "stable\n").unwrap();
    let policy = serde_json::to_vec(&json!({"schema_version":1,"checks":[{
        "id":"static","argv":[driver],"tools":[{"id":"scan","argv":[driver,"--version"]}],
        "reports":[{"path":"report.out","baseline":"report.out","format":"diagnostics","mode":"new_diagnostics"}]
    }]})).unwrap();
    fs::write(root.path().join("qualitygate.yaml"), &policy).unwrap();
    let finding =
        |file: &str, line| json!({"rule":"R","file":file,"line":line,"message":"finding"});
    let baseline = |line| {
        serde_json::to_vec(
            &json!({"issues":[finding("src/old.rs",line),finding("src/untouched.rs",1)]}),
        )
        .unwrap()
    };
    let invalid = baseline(4);
    fs::write(root.path().join("input.txt"), &invalid).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "baseline source range"]);
    git(root.path(), &["mv", "src/old.rs", "src/new.rs"]);
    // Exact renames add no lines; file-level findings still belong to delivery.
    let renamed = json!({"rule":"R","file":"src/new.rs","message":"finding"});
    let current =
        serde_json::to_vec(&json!({"issues":[renamed,renamed,finding("src/untouched.rs",1)]}))
            .unwrap();
    fs::write(root.path().join("input.txt"), &current).unwrap();
    let run = |code| {
        report(
            &cli(root.path(), &["check", "--worktree", "--format", "json"]),
            code,
        )
    };
    let incomplete = run(2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
    let check = &incomplete["checks"][0];
    assert!(
        check["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("outside the selected source")
    );
    let raw = check["execution"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["path"].as_str().unwrap().ends_with("baseline-report-0"))
        .unwrap();
    assert_eq!(fs::read(raw["path"].as_str().unwrap()).unwrap(), invalid);
    assert_eq!(raw["digest"], qualitygate::snapshot::digest(&invalid));
    assert_eq!(
        check["metadata"]["baseline_input_integrity"]["status"],
        "verified"
    );
    assert_eq!(fs::read(root.path().join("input.txt")).unwrap(), current);
    assert_eq!(
        fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        policy
    );

    // Commit only the repaired baseline report input, keeping the rename staged.
    fs::write(root.path().join("input.txt"), baseline(1)).unwrap();
    git(
        root.path(),
        &[
            "commit",
            "--only",
            "-qm",
            "repair baseline range",
            "--",
            "input.txt",
        ],
    );
    fs::write(root.path().join("input.txt"), &current).unwrap();
    let checked = run(1);
    let changed = checked["selection"]["changed_files"].as_array().unwrap();
    assert!(changed.contains(&json!("src/new.rs")));
    assert!(!changed.contains(&json!("src/old.rs")));
    let check = &checked["checks"][0];
    assert_eq!(check["metadata"]["report.out:delivery_filtered"], 1);
    assert_eq!(check["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(check["diagnostics"][0]["file"], "src/new.rs");
    assert_eq!(check["diagnostics"][0]["range"], Value::Null);
    fs::write(
        root.path().join("input.txt"),
        serde_json::to_vec(&json!({"issues":[renamed,finding("src/untouched.rs",1)]})).unwrap(),
    )
    .unwrap();
    assert_eq!(run(0)["gate"]["decision"], "pass");
}
