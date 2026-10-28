mod common;
use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, process::Command};

fn document(format: &str, path: &str, count: usize) -> Vec<u8> {
    let text = match format {
        "eslint_json" => serde_json::to_string(&json!([{
            "filePath":path,"errorCount":count,"warningCount":0,"fatalErrorCount":0,
            "messages":vec![json!({"ruleId":"R","severity":2,"line":1,"message":"finding"});count]
        }]))
        .unwrap(),
        "checkstyle" => format!(
            "<checkstyle><file name='{path}'>{}</file></checkstyle>",
            "<error source='R' line='1' message='finding'/>".repeat(count)
        ),
        "pmd" => format!(
            "<pmd><file name='{path}'>{}</file></pmd>",
            "<violation rule='R' beginline='1'>finding</violation>".repeat(count)
        ),
        _ => unreachable!(),
    };
    assert!(text.len() < qualitygate::snapshot::MAX_FILE_BYTES);
    text.into_bytes()
}

#[test]
fn excessive_diagnostics_are_incomplete_preserve_raw_evidence_and_recover() {
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
    if std::env::args().any(|a| a == "--version") { println!("Diagnostic fixture 1"); return; }
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
    fs::write(root.path().join("src/app.js"), "old\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "diagnostic fixture source"]);
    fs::write(root.path().join("src/app.js"), "changed\n").unwrap();
    let run = |code| {
        report(
            &cli(root.path(), &["check", "--worktree", "--format", "json"]),
            code,
        )
    };
    for format in ["eslint_json", "checkstyle", "pmd"] {
        let config = serde_json::to_vec(&json!({"schema_version":1,"checks":[{
            "id":"static","argv":[driver],"tools":[{"id":"scan","argv":[driver,"--version"]}],
            "reports":[{"path":"report.out","format":format,"mode":"full"}]
        }]}))
        .unwrap();
        fs::write(root.path().join("qualitygate.yaml"), &config).unwrap();
        for (path, count) in [("x".repeat(1_000_000), 1024), ("\\".repeat(65_536), 128)] {
            let input = document(format, &path, count);
            fs::write(root.path().join("input.txt"), &input).unwrap();
            let checked = run(2);
            assert_eq!(checked["gate"]["complete"], false);
            assert_eq!(checked["gate"]["decision"], "incomplete");
            assert_eq!(checked["checks"][0]["verdict"], Value::Null);
            let execution = &checked["checks"][0]["execution"];
            assert!(
                execution["reason"]
                    .as_str()
                    .unwrap()
                    .contains("Normalized diagnostics exceed 16 MiB")
            );
            let artifact = execution["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
                .unwrap();
            assert_eq!(fs::read(artifact["path"].as_str().unwrap()).unwrap(), input);
            assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&input));
            assert_eq!(fs::read(root.path().join("input.txt")).unwrap(), input);
            assert_eq!(
                fs::read(root.path().join("qualitygate.yaml")).unwrap(),
                config
            );
            assert_eq!(
                fs::read(root.path().join("src/app.js")).unwrap(),
                b"changed\n"
            );
        }
        fs::write(
            root.path().join("input.txt"),
            document(format, "src/app.js", 1),
        )
        .unwrap();
        let finding = run(1);
        assert_eq!(finding["gate"]["complete"], true);
        assert_eq!(finding["gate"]["decision"], "fail");
        fs::write(
            root.path().join("input.txt"),
            document(format, "src/app.js", 0),
        )
        .unwrap();
        let repaired = run(0);
        assert_eq!(repaired["gate"]["complete"], true);
        assert_eq!(repaired["gate"]["decision"], "pass");
    }
}
