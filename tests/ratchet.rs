mod common;
#[path = "common/repository.rs"]
mod repository;
use common::*;
use serde_json::{Value, json};
use std::process::Command;

const DRIVER: &str = r#"
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let base = std::fs::read_to_string("state.txt").unwrap() == "base";
    if args[1] == "--version" {
        println!("diagnostic-fixture {}", if args.get(2).is_some() { if base { "old" } else { "new" } } else { "1" });
        return;
    }
    if base {
        match args[1].as_str() {
            "skip-base" => return,
            "timeout-base" => std::thread::sleep(std::time::Duration::from_secs(30)),
            "mutate-base" => std::fs::write("a.rs", "changed after capture\n").unwrap(),
            _ => {}
        }
    }
    std::fs::create_dir_all("target").unwrap();
    std::fs::copy("input.report", "target/report.json").unwrap();
    if base && args[1] == "crash-base" { std::process::exit(9); }
}
"#;

fn issue(rule: &str, line: usize) -> Value {
    json!({"rule":rule,"file":"a.rs","line":line,"message":format!("{rule} finding"),"symbol":null})
}

struct Fixture {
    root: tempfile::TempDir,
    _external: tempfile::TempDir,
    policy: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture();
        let external = tempfile::tempdir().unwrap();
        let source = external.path().join("driver.rs");
        let binary = external.path().join(if cfg!(windows) {
            "driver.exe"
        } else {
            "driver"
        });
        std::fs::write(&source, DRIVER).unwrap();
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
        let value = Self {
            root,
            _external: external,
            policy: json!({"schema_version":1,"checks":[{"id":"debt","argv":[binary,"normal"],
                "timeout_seconds":1,"tools":[{"id":"producer","argv":[binary,"--version"]}],
                "reports":[{"path":"target/report.json","baseline":"target/report.json","format":"diagnostics","mode":"ratchet"}]}]}),
        };
        std::fs::write(value.root.path().join(".gitignore"), "target/\n").unwrap();
        std::fs::write(value.root.path().join("a.rs"), "old\nnew\nlast\n").unwrap();
        std::fs::write(value.root.path().join("state.txt"), "base").unwrap();
        value.configure();
        value.input(vec![issue("a", 1), issue("a", 2)]);
        value.commit();
        std::fs::write(value.root.path().join("state.txt"), "current").unwrap();
        value
    }
    fn input(&self, issues: Vec<Value>) {
        std::fs::write(
            self.root.path().join("input.report"),
            json!({"issues":issues}).to_string(),
        )
        .unwrap();
    }
    fn configure(&self) {
        std::fs::write(
            self.root.path().join("qualitygate.yaml"),
            serde_norway::to_string(&self.policy).unwrap(),
        )
        .unwrap();
    }
    fn commit(&self) {
        git(self.root.path(), &["add", "."]);
        git(
            self.root.path(),
            &["commit", "-qm", "record diagnostic baseline"],
        );
    }
    fn run(&self, code: i32, args: &[&str]) -> Value {
        report(&repository::check(self.root.path(), args), code)
    }
}

#[test]
fn ratchet_reexecutes_both_snapshots_and_does_not_trade_between_rule_counts() {
    let fixture = Fixture::new();
    fixture.input(vec![issue("a", 3), issue("b", 2)]);
    let failed = fixture.run(1, &[]);
    let result = &failed["checks"][0];
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(result["diagnostics"][0]["evidence"]["rule"], "b");
    assert_eq!(
        result["metadata"]["target/report.json:ratchet"],
        json!([
            {"key":{"tool":null,"rule":"a"},"baseline":2,"current":1},
            {"key":{"tool":null,"rule":"b"},"baseline":0,"current":1}
        ])
    );
    assert_eq!(
        result["metadata"]["baseline_input_integrity"]["status"],
        "verified"
    );
    assert_ne!(
        result["metadata"]["baseline_snapshot"]["content_digest"],
        failed["snapshot"]["content_digest"]
    );
    let artifacts = result["execution"]["artifacts"].as_array().unwrap();
    assert!(artifacts.iter().any(|artifact| {
        artifact["path"]
            .as_str()
            .unwrap()
            .contains("baseline-report")
    }));
    for artifact in artifacts {
        let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
        assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
    }
    fixture.input(vec![issue("a", 2), issue("a", 3)]);
    fixture.run(0, &[]);
    fixture.input(vec![issue("a", 1), issue("a", 2), issue("a", 3)]);
    fixture.run(1, &[]);
    git(fixture.root.path(), &["add", "input.report"]);
    fixture.input(vec![]);
    fixture.run(1, &["--staged"]);
    fixture.run(0, &[]);
    fixture.commit();
    fixture.input(vec![issue("a", 1)]);
    let regressed = fixture.run(1, &[]);
    assert_eq!(
        regressed["checks"][0]["metadata"]["target/report.json:ratchet"][0]["baseline"],
        0
    );
}

#[test]
fn ratchet_baseline_failures_timeouts_mutation_and_tool_mismatch_are_incomplete() {
    let mut fixture = Fixture::new();
    fixture.input(vec![]);
    for (mode, reason) in [
        ("skip-base", "Required report was not produced"),
        ("timeout-base", "did not complete"),
        ("mutate-base", "changed its checked inputs"),
        ("crash-base", "unexpected exit code"),
    ] {
        fixture.policy["checks"][0]["argv"][1] = json!(mode);
        fixture.configure();
        let incomplete = fixture.run(2, &[]);
        assert!(incomplete["checks"][0]["verdict"].is_null());
        assert!(
            incomplete["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains(reason),
            "{incomplete}"
        );
    }
    fixture.policy["checks"][0]["argv"][1] = json!("normal");
    fixture.policy["checks"][0]["tools"][0]["argv"]
        .as_array_mut()
        .unwrap()
        .push(json!("different"));
    fixture.configure();
    let mismatch = fixture.run(2, &[]);
    assert!(
        mismatch["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("differ")
    );
    // Selected policy protects the mode and command just like other report checks.
    fixture.run(2, &["--policy-ref", "HEAD"]);
}

#[test]
fn ratchet_malformed_and_non_diagnostic_reports_cannot_become_zero_debt() {
    let fixture = Fixture::new();
    for value in [
        json!({}),
        json!({"issues":[],"tests":{"executed":1,"failures":0,"skipped":0}}),
        json!({"issues":[issue("a",100)]}),
    ] {
        std::fs::write(fixture.root.path().join("input.report"), value.to_string()).unwrap();
        fixture.run(2, &[]);
    }
    fixture.commit();
    fixture.input(vec![]);
    fixture.run(2, &[]);
    for report_spec in [
        json!({"path":"out","format":"diagnostics","mode":"ratchet"}),
        json!({"path":"out","baseline":"out","format":"junit","mode":"ratchet"}),
    ] {
        let mut policy = fixture.policy.clone();
        policy["checks"][0]["reports"][0] = report_spec;
        assert!(
            qualitygate::config::parse(serde_norway::to_string(&policy).unwrap().as_bytes())
                .is_err()
        );
    }
}
