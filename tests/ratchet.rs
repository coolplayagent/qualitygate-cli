mod common;
#[path = "common/repository.rs"]
mod repository;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

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
            "mutate-base-attributes" => {
                let mut permissions = std::fs::metadata("a.rs").unwrap().permissions();
                permissions.set_readonly(true);
                std::fs::set_permissions("a.rs", permissions).unwrap();
            }
            "restore-base" => {
                let modified = std::fs::metadata("a.rs").unwrap().modified().unwrap();
                std::fs::rename("a.rs", "a-held").unwrap();
                std::fs::copy("a-held", "a.rs").unwrap();
                std::fs::File::options().write(true).open("a.rs").unwrap()
                    .set_times(std::fs::FileTimes::new().set_modified(modified)).unwrap();
            }
            _ => {}
        }
    }
    std::fs::create_dir_all("target").unwrap();
    std::fs::copy("input.report", "target/report.json").unwrap();
    print!("{}", std::fs::read_to_string("input.report").unwrap());
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
    fn assert_current_reports(&self, value: &Value, indexes: &[usize]) {
        let bytes = std::fs::read(self.root.path().join("input.report")).unwrap();
        let identity = qualitygate::snapshot::digest(b"debt");
        let artifacts = value["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap();
        for index in indexes {
            let name = format!(
                "check-{}-report-{index}",
                identity.trim_start_matches("sha256:")
            );
            let artifact = artifacts
                .iter()
                .find(|artifact| {
                    Path::new(artifact["path"].as_str().unwrap())
                        .file_name()
                        .unwrap()
                        == name.as_str()
                })
                .unwrap_or_else(|| panic!("Missing current report {index}: {value}"));
            assert_eq!(
                std::fs::read(artifact["path"].as_str().unwrap()).unwrap(),
                bytes
            );
            assert_eq!(artifact["bytes"], bytes.len());
            assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
        }
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
    for digest in [
        &result["metadata"]["baseline_tools"][0]["snapshot_digest"],
        &result["metadata"]["baseline_input_integrity"]["snapshot_digest"],
    ] {
        assert_eq!(
            digest,
            &result["metadata"]["baseline_snapshot"]["content_digest"]
        );
    }
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
    let mutation_reason = if cfg!(windows) {
        "unexpected exit code"
    } else {
        "changed its checked inputs"
    };
    for (mode, reason) in [
        ("skip-base", "Required report was not produced"),
        ("timeout-base", "did not complete"),
        ("mutate-base", mutation_reason),
        ("restore-base", mutation_reason),
        ("mutate-base-attributes", "changed its checked inputs"),
        ("crash-base", "unexpected exit code"),
    ] {
        fixture.policy["checks"][0]["argv"][1] = json!(mode);
        fixture.configure();
        let incomplete = fixture.run(2, &[]);
        fixture.assert_current_reports(&incomplete, &[0]);
        assert!(incomplete["checks"][0]["verdict"].is_null());
        assert!(
            incomplete["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains(reason),
            "{incomplete}"
        );
        if matches!(
            mode,
            "mutate-base" | "restore-base" | "mutate-base-attributes"
        ) {
            let prevented = cfg!(windows) && mode != "mutate-base-attributes";
            assert_eq!(
                incomplete["checks"][0]["metadata"]["baseline_input_integrity"]["status"],
                if prevented { "verified" } else { "invalid" }
            );
            if prevented {
                assert!(
                    incomplete["checks"][0]["metadata"]["baseline_execution"]["exit_code"]
                        .as_i64()
                        .is_some_and(|code| code != 0)
                );
                assert_artifact_contains(
                    &incomplete["checks"][0]["execution"],
                    "baseline-stderr",
                    "code: 32",
                );
            }
            assert_eq!(
                std::fs::read_to_string(fixture.root.path().join("a.rs")).unwrap(),
                "old\nnew\nlast\n"
            );
        }
    }
    fixture.policy["checks"][0]["argv"][1] = json!("normal");
    fixture.policy["checks"][0]["tools"][0]["argv"]
        .as_array_mut()
        .unwrap()
        .push(json!("different"));
    fixture.configure();
    let mismatch = fixture.run(2, &[]);
    fixture.assert_current_reports(&mismatch, &[0]);
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
fn ratchet_retains_current_file_and_stdout_reports_when_baseline_fails() {
    for mode in ["ratchet", "new_diagnostics"] {
        let mut fixture = Fixture::new();
        fixture.input(vec![]);
        fixture.policy["checks"][0]["argv"][1] = json!("crash-base");
        fixture.policy["checks"][0]["reports"][0]["mode"] = json!(mode);
        fixture.policy["checks"][0]["reports"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"stdout.json","baseline":"stdout.json",
            "format":"diagnostics","mode":mode,"from_stdout":true}));
        fixture.policy["checks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"after","argv":["git","--version"],"depends_on":["debt"]}));
        for missing_current in [false, true] {
            if missing_current {
                fixture.policy["checks"][0]["reports"]
                    .as_array_mut()
                    .unwrap()
                    .push(
                        json!({"path":"target/missing.json","format":"diagnostics","mode":"full"}),
                    );
            }
            fixture.configure();
            let incomplete = fixture.run(2, &[]);
            assert_eq!(incomplete["gate"]["complete"], false);
            let check = &incomplete["checks"][0];
            assert!(check["verdict"].is_null());
            assert_eq!(check["execution"]["status"], "tool_error");
            assert_eq!(check["execution"]["exit_code"], 0);
            assert_eq!(check["metadata"]["baseline_execution"]["exit_code"], 9);
            let reason = check["execution"]["reason"].as_str().unwrap();
            assert!(reason.contains("Baseline analyzer returned unexpected exit code"));
            if missing_current {
                assert!(reason.contains("Current report collection also failed"));
                assert!(reason.contains("Required report was not produced: target/missing.json"));
            }
            assert!(check["metadata"].as_object().unwrap().keys().all(|key| {
                !key.starts_with("target/report.json:") && !key.starts_with("stdout.json:")
            }));
            fixture.assert_current_reports(&incomplete, &[0, 1]);
            assert_eq!(incomplete["checks"][1]["execution"]["status"], "blocked");
            assert!(incomplete["checks"][1]["execution"]["started_at_ms"].is_null());
        }
        fixture.policy["checks"][0]["reports"]
            .as_array_mut()
            .unwrap()
            .pop();
        fixture.policy["checks"][0]["argv"][1] = json!("normal");
        fixture.configure();
        let repaired = fixture.run(0, &[]);
        fixture.assert_current_reports(&repaired, &[0, 1]);
        assert_eq!(repaired["checks"][0]["verdict"], "pass");
        assert_eq!(repaired["checks"][1]["verdict"], "pass");
    }
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
