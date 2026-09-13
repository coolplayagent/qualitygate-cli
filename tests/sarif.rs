mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const DRIVER: &str = r#"
fn main() {
    if std::env::args().any(|arg| arg == "--version") { println!("SARIF fixture 1"); return; }
    let path = std::env::current_dir().unwrap().to_str().unwrap().replace('\\',"/");
    let mut uri = if path.starts_with('/') { "file://".to_owned() } else { "file:///".to_owned() };
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/:-._~".contains(&byte) { uri.push(byte as char); }
        else { uri.push_str(&format!("%{byte:02X}")); }
    }
    let input = std::fs::read_to_string("input.json").unwrap();
    std::fs::write("report.sarif", input.replace("$WORKSPACE", &uri)).unwrap();
}
"#;

fn location(index: usize, start: usize, end: usize) -> Value {
    json!({"physicalLocation":{"artifactLocation":{"index":index},"region":{"startLine":start,"endLine":end}},
        "logicalLocations":[{"fullyQualifiedName":"module::symbol"}]})
}

fn finding(message: &str, locations: Vec<Value>) -> Value {
    json!({"ruleId":"R","message":{"text":message},"locations":locations})
}

fn document(results: Vec<Value>) -> Value {
    json!({"version":"2.1.0","runs":[{
        "tool":{"driver":{"name":"scan","version":"1.0","rules":[{"id":"R"}]}},
        "invocations":[{"executionSuccessful":true}],
        "artifacts":[{"location":{"uri":"src/a%20one.rs","uriBaseId":"ROOT"}},{"location":{"uri":"src/b.rs","uriBaseId":"ROOT"}}],
        "originalUriBaseIds":{"ROOT":{"uri":"$WORKSPACE/"}},
        "results":results
    }]})
}

struct Fixture {
    root: tempfile::TempDir,
    _external: tempfile::TempDir,
    driver: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture();
        let external = tempfile::tempdir().unwrap();
        let source = external.path().join("driver.rs");
        let driver = external.path().join(if cfg!(windows) {
            "driver.exe"
        } else {
            "driver"
        });
        std::fs::write(&source, DRIVER).unwrap();
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
        std::fs::create_dir(root.path().join("src")).unwrap();
        for name in ["a one.rs", "b.rs"] {
            std::fs::write(root.path().join("src").join(name), "old\nchanged\nstable\n").unwrap();
        }
        let fixture = Self {
            root,
            _external: external,
            driver,
        };
        fixture.config("full");
        fixture.input(&document(vec![finding(
            "Historical issue",
            vec![location(0, 1, 1)],
        )]));
        git(fixture.root.path(), &["add", "."]);
        git(
            fixture.root.path(),
            &["commit", "-qm", "baseline SARIF analysis"],
        );
        fixture
    }
    fn config(&self, mode: &str) {
        let mut spec = json!({"path":"report.sarif","format":"sarif","mode":mode});
        if mode == "new_diagnostics" {
            spec["baseline"] = json!("report.sarif");
        }
        let policy = json!({"schema_version":1,"checks":[{
            "id":"static","argv":[self.driver],
            "tools":[{"id":"scan","argv":[self.driver,"--version"]}],
            "reports":[spec]
        }]});
        std::fs::write(
            self.root.path().join("qualitygate.yaml"),
            serde_norway::to_string(&policy).unwrap(),
        )
        .unwrap();
    }
    fn input(&self, value: &Value) {
        std::fs::write(
            self.root.path().join("input.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }
    fn run(&self, code: i32) -> Value {
        report(&cli(self.root.path(), &["check", "--format", "json"]), code)
    }
}

#[test]
fn indexed_absolute_paths_compare_fresh_baselines_and_multi_location_ranges() {
    let fixture = Fixture::new();
    fixture.config("new_diagnostics");
    fixture.run(0);
    std::fs::write(
        fixture.root.path().join("src/a one.rs"),
        "prefix\nold\nchanged\nstable\n",
    )
    .unwrap();
    let new = finding("New violation", vec![location(1, 1, 1), location(0, 1, 2)]);
    fixture.input(&document(vec![
        finding("Historical issue", vec![location(0, 2, 2)]),
        new.clone(),
    ]));
    let report = fixture.run(1);
    let check = &report["checks"][0];
    assert_eq!(check["metadata"]["report.sarif:filtered"], 1);
    assert_eq!(check["diagnostics"].as_array().unwrap().len(), 1);
    let identity = check["diagnostics"][0]["fingerprint"].clone();
    assert_eq!(
        check["diagnostics"][0]["evidence"]["locations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        check["metadata"]["report.sarif:sarif_runs"][0]["tool"],
        "scan"
    );
    for artifact in check["execution"]["artifacts"].as_array().unwrap() {
        assert!(Path::new(artifact["path"].as_str().unwrap()).is_file());
    }
    fixture.config("changed_lines");
    fixture.input(&document(vec![new.clone()]));
    let report = fixture.run(1);
    let diagnostic = &report["checks"][0]["diagnostics"][0];
    assert_eq!(diagnostic["file"], "src/a one.rs");
    assert_eq!(diagnostic["range"], json!({"start_line":1,"end_line":2}));
    assert_eq!(diagnostic["fingerprint"], identity);
    let mut reversed = new.clone();
    reversed["locations"].as_array_mut().unwrap().reverse();
    fixture.input(&document(vec![reversed]));
    assert_eq!(
        fixture.run(1)["checks"][0]["diagnostics"][0]["fingerprint"],
        identity
    );
    fixture.config("new_diagnostics");
    fixture.input(&document(vec![finding(
        "Historical issue",
        vec![location(0, 2, 2)],
    )]));
    fixture.run(0);
}

#[test]
fn malformed_partial_or_out_of_snapshot_reports_never_pass_and_keep_raw_evidence() {
    let fixture = Fixture::new();
    for (key, value) in [
        ("invocations", json!([{"executionSuccessful":"true"}])),
        (
            "invocations",
            json!([{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"error"}]}]),
        ),
        (
            "externalPropertyFileReferences",
            json!({"results":[{"location":{"uri":"missing.sarif"}}]}),
        ),
        (
            "results",
            json!([{"ruleId":"R","message":{"text":"Unresolved"},"kind":"open"}]),
        ),
        (
            "results",
            json!([{"ruleId":"R","message":{"text":"Old result"},"baselineState":"absent"}]),
        ),
    ] {
        let mut data = document(vec![]);
        data["runs"][0][key] = value;
        fixture.input(&data);
        let output = fixture.run(2);
        assert_eq!(output["checks"][0]["verdict"], Value::Null);
        assert!(
            output["checks"][0]["execution"]["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
        );
    }
    fixture.config("changed_lines");
    fixture.input(&document(vec![finding(
        "Outside source",
        vec![location(0, 100, 100)],
    )]));
    fixture.run(2);
    fixture.config("new_diagnostics");
    fixture.input(&document(vec![]));
    fixture.run(0);
    let mut invalid = document(vec![]);
    invalid["runs"][0]["invocations"] = json!([{"executionSuccessful":false}]);
    fixture.input(&invalid);
    git(fixture.root.path(), &["add", "."]);
    git(
        fixture.root.path(),
        &["commit", "-qm", "invalid baseline execution evidence"],
    );
    fixture.input(&document(vec![]));
    let output = fixture.run(2);
    assert_eq!(
        output["checks"][0]["metadata"]["baseline_execution"]["exit_code"],
        0
    );
}

#[test]
fn result_kinds_and_producer_identity_do_not_hide_or_invent_violations() {
    let fixture = Fixture::new();
    let mut passed = finding("Evaluated successfully", vec![]);
    passed["kind"] = json!("pass");
    fixture.input(&document(vec![passed]));
    let output = fixture.run(0);
    assert_eq!(
        output["checks"][0]["metadata"]["report.sarif:sarif_runs"][0]["non_violations"],
        1
    );
    let mut suppressed = finding("Needs repair", vec![location(0, 1, 1)]);
    suppressed["suppressions"] = json!([{"kind":"inSource","status":"accepted"}]);
    fixture.input(&document(vec![suppressed]));
    fixture.run(1);
    fixture.config("new_diagnostics");
    let mut changed_tool = document(vec![finding("Historical issue", vec![location(0, 1, 1)])]);
    changed_tool["runs"][0]["tool"]["driver"]["name"] = json!("different-scan");
    fixture.input(&changed_tool);
    fixture.run(1);
}
