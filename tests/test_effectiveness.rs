mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

// This producer executes Rust assertions over captured production/test inputs.
// It is a controlled runner fixture, not a claim about other JUnit producers.
const DRIVER: &str = r#"
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let value: i32 = std::fs::read_to_string("source.txt").unwrap().trim().parse().unwrap();
    if args[1] == "--version" {
        println!("assertion-fixture {}", if args.get(2).is_some() { value } else { 1 });
        return;
    }
    let base = value == 1;
    let mode = args[1].as_str();
    if mode == "mutate-current" || (base && mode == "mutate-base") {
        std::fs::write("source.txt", "99").unwrap();
    }
    if mode == "shared" { std::thread::sleep(std::time::Duration::from_millis(600)); }
    if base && mode == "timeout" {
        println!("baseline started");
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    if base && mode == "missing" { std::process::exit(9); }
    let offset: i32 = std::fs::read_to_string("support.txt").unwrap_or("0".into()).trim().parse().unwrap();
    let mut xml = String::from("<testsuite>");
    let mut failed = false;
    let mut paths: Vec<_> = std::fs::read_dir("tests").unwrap().map(|f| f.unwrap().path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|v| v.to_str()) != Some("case") { continue; }
        if mode == "zero" || (base && mode == "omit") { continue; }
        let expected = std::fs::read_to_string(&path).unwrap();
        let result = std::panic::catch_unwind(|| {
            if expected.trim().starts_with("positive") { assert!(value + offset > 0); }
            else { assert_eq!(value + offset, expected.trim().parse::<i32>().unwrap()); }
        });
        let name = if base && mode == "identity" { "different" } else { "behavior" };
        let file = if mode == "absolute" { std::env::current_dir().unwrap().join(&path).to_string_lossy().to_string() }
            else { path.to_string_lossy().replace('\\', "/") };
        let mut case = format!("<testcase file='{file}' classname='Suite' name='{name}'>");
        if mode == "skip" { case.push_str("<skipped/>"); }
        else if base && mode == "error" { case.push_str("<error type='MissingSymbol'/>"); failed = true; }
        else if result.is_err() {
            failed = true;
            let kind = if mode == "unknown" { "MissingSymbol" } else { "AssertionError" };
            case.push_str(&format!("<failure type='{kind}' message='actual assertion failed'/>"));
        }
        case.push_str("</testcase>");
        xml.push_str(&case);
        if mode == "duplicate" { xml.push_str(&case); }
    }
    xml.push_str("</testsuite>");
    std::fs::create_dir_all("target").unwrap();
    std::fs::write("target/tests.xml", if base && mode == "malformed" { "<bad" } else { &xml }).unwrap();
    if base && mode == "crash" { std::process::exit(9); }
    if failed && mode != "false-success" { std::process::exit(1); }
}
"#;

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
        let output = Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let fixture = Self {
            root,
            _external: external,
            policy: json!({"schema_version":1,"checks":[{
                "id":"behavior","argv":[binary,"normal"],"timeout_seconds":10,"findings_exit_codes":[1],
                "tools":[{"id":"producer","argv":[binary,"--version"]}],
                "reports":[{"path":"target/tests.xml","format":"junit"}],
                "test_effectiveness":{"source_paths":["source.txt"],"test_paths":["tests/*.case"],
                    "support_paths":["support.txt"],"assertion_failure_types":["AssertionError"]}
            }]}),
        };
        std::fs::create_dir(fixture.path().join("tests")).unwrap();
        fixture.write(".gitignore", "target/\n");
        fixture.write("source.txt", "1");
        fixture.write("tests/a.case", "positive");
        fixture.write("support.txt", "0");
        fixture.configure();
        fixture.commit();
        fixture.write("source.txt", "2");
        fixture.write("tests/a.case", "2");
        fixture
    }
    fn path(&self) -> &Path {
        self.root.path()
    }
    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.path().join(path), text).unwrap();
    }
    fn configure(&self) {
        self.write(
            "qualitygate.yaml",
            &serde_norway::to_string(&self.policy).unwrap(),
        );
    }
    fn commit(&self) {
        git(self.path(), &["add", "."]);
        git(self.path(), &["commit", "-qm", "record baseline"]);
    }
    fn run(&self, code: i32, args: &[&str]) -> Value {
        let mut argv = vec!["check", "--format", "json"];
        argv.extend(args);
        report(&cli(self.path(), &argv), code)
    }
    fn mode(&mut self, mode: &str) {
        self.policy["checks"][0]["argv"][1] = json!(mode);
        self.configure();
    }
}

#[test]
fn real_assertions_prove_each_changed_file_and_retain_both_executions() {
    let fixture = Fixture::new();
    let passed = fixture.run(0, &[]);
    let check = &passed["checks"][0];
    assert_eq!(
        check["metadata"]["test_effectiveness_files"][0]["counterexamples"],
        1
    );
    assert_eq!(
        check["metadata"]["current_test_execution"]["execution"]["exit_code"],
        0
    );
    assert_eq!(
        check["metadata"]["baseline_test_execution"]["execution"]["exit_code"],
        1
    );
    assert_eq!(
        check["metadata"]["test_effectiveness"]["overlay"]["tests/a.case"],
        qualitygate::snapshot::digest(b"2")
    );
    assert_ne!(
        check["metadata"]["test_effectiveness"]["baseline_snapshot"]["content_digest"],
        passed["snapshot"]["content_digest"]
    );
    for artifact in check["execution"]["artifacts"].as_array().unwrap() {
        let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
        assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
    }
    for format in ["table", "markdown"] {
        let rendered = cli(fixture.path(), &["check", "--format", format]);
        assert!(rendered.status.success());
        assert!(
            String::from_utf8(rendered.stdout)
                .unwrap()
                .contains("1 counterexamples / 1 executed tests")
        );
    }
    fixture.write("tests/b.case", "positive");
    let weak = fixture.run(1, &[]);
    assert_eq!(weak["checks"][0]["diagnostics"][0]["file"], "tests/b.case");
    fixture.write("tests/b.case", "2");
    fixture.run(0, &[]);
    fixture.write("tests/a.case", "3");
    let failed = fixture.run(1, &[]);
    assert!(failed["checks"][0]["metadata"]["baseline_test_execution"].is_null());
    fixture.write("tests/a.case", "positive changed");
    fixture.write("tests/b.case", "positive");
    let weak = fixture.run(1, &[]);
    assert_eq!(
        weak["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_ne!(
        weak["checks"][0]["diagnostics"][0]["id"],
        weak["checks"][0]["diagnostics"][1]["id"]
    );
}

#[test]
fn execution_gaps_and_non_assertion_failures_never_establish_effectiveness() {
    let mut fixture = Fixture::new();
    for (mode, reason) in [
        ("missing", "Required report"),
        ("malformed", "root node"),
        ("error", "execution error"),
        ("unknown", "assertion type"),
        ("duplicate", "duplicate"),
        ("zero", "insufficient"),
        ("skip", "insufficient"),
        ("omit", "insufficient"),
        ("identity", "identities differ"),
        ("crash", "unexpected analyzer"),
        ("mutate-base", "invalid"),
        ("mutate-current", "invalid"),
        ("false-success", "contradict"),
    ] {
        fixture.mode(mode);
        let report = fixture.run(2, &[]);
        let check = &report["checks"][0];
        assert!(check["verdict"].is_null(), "{mode}");
        let actual = check["execution"]["reason"].as_str().unwrap();
        assert!(actual.contains(reason), "{mode}: {actual}");
    }
    fixture.policy["checks"][0]["timeout_seconds"] = json!(1);
    fixture.mode("timeout");
    let report = fixture.run(2, &[]);
    assert_eq!(
        report["checks"][0]["metadata"]["baseline_test_execution"]["execution"]["status"],
        "timed_out"
    );
    assert!(
        report["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| {
                std::fs::read_to_string(artifact["path"].as_str().unwrap())
                    .unwrap()
                    .contains("baseline started")
            })
    );
    fixture.mode("shared");
    let shared = fixture.run(2, &[]);
    assert_eq!(
        shared["checks"][0]["metadata"]["baseline_test_execution"]["execution"]["status"],
        "timed_out"
    );
    fixture.policy["checks"][0]["timeout_seconds"] = json!(10);
    fixture.mode("normal");
    fixture.policy["checks"][0]["tools"][0]["argv"]
        .as_array_mut()
        .unwrap()
        .push(json!("variable"));
    fixture.configure();
    let report = fixture.run(2, &[]);
    assert!(
        report["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("versions or tool inputs differ")
    );
}

#[test]
fn snapshot_selection_support_overlay_and_policy_authority_are_preserved() {
    let mut fixture = Fixture::new();
    fixture.run(1, &["--path", "source.txt"]);
    fixture.write("tests/a.case", "positive");
    let missing = fixture.run(1, &[]);
    assert_eq!(
        missing["checks"][0]["diagnostics"][0]["evidence"]["assertion"],
        "missing-tests"
    );
    fixture.write("source.txt", "1");
    let skipped = fixture.run(2, &[]);
    assert_eq!(skipped["checks"][0]["applicability"], "not_applicable");
    fixture.write("source.txt", "2");
    fixture.write("tests/a.case", "2");
    git(fixture.path(), &["add", "source.txt"]);
    fixture.run(1, &["--staged"]);
    git(fixture.path(), &["add", "tests/a.case"]);
    fixture.write("tests/a.case", "3");
    fixture.run(0, &["--staged"]);
    fixture.run(1, &[]);
    fixture.write("support.txt", "1");
    fixture.run(0, &[]);
    fixture.policy["checks"][0]["test_effectiveness"]["assertion_failure_types"] = json!(["Other"]);
    fixture.configure();
    let stale = fixture.run(2, &["--policy-ref", "HEAD"]);
    assert!(!stale["policy"]["changes"].as_array().unwrap().is_empty());
    fixture.policy["checks"][0]["test_effectiveness"]["assertion_failure_types"] =
        json!(["AssertionError"]);
    fixture.mode("absolute");
    fixture.run(0, &[]);
    // A selected task uses the same nested command specification.
    let mut verification = fixture.policy["checks"][0].clone();
    verification.as_object_mut().unwrap().remove("id");
    verification["check_id"] = json!("task-behavior");
    fixture.write("task.yaml", &serde_norway::to_string(&json!({"schema_version":1,"task_id":"task","acceptance":[{
        "id":"behavior","description":"Changed behavior has a counterexample","verification":verification
    }]})).unwrap());
    fixture.policy["checks"] = json!([]);
    fixture.configure();
    let task = fixture.run(0, &["--task", "task.yaml"]);
    assert_eq!(task["plan"]["acceptance"]["behavior"], "task-behavior");
}

#[test]
fn missing_scopes_protected_inputs_and_report_collisions_fail_closed() {
    let mut fixture = Fixture::new();
    for (key, values, reason) in [
        ("source_paths", json!(["absent/**"]), "matched no"),
        ("source_paths", json!(["source.txt", "tests/**"]), "overlap"),
        ("support_paths", json!(["qualitygate.yaml"]), "protected"),
        ("support_paths", json!(["Cargo.toml"]), "build input"),
    ] {
        let original = fixture.policy["checks"][0]["test_effectiveness"][key].clone();
        fixture.policy["checks"][0]["test_effectiveness"][key] = values;
        fixture.configure();
        fixture.write("Cargo.toml", "[package]\nname='fixture'\nversion='0.1.0'\n");
        let report = fixture.run(2, &[]);
        assert!(
            report["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains(reason)
        );
        fixture.policy["checks"][0]["test_effectiveness"][key] = original;
    }
    fixture.configure();
    fixture.policy["verification_assets"] = json!(["tests/**"]);
    fixture.configure();
    fixture.run(2, &[]);
    fixture
        .policy
        .as_object_mut()
        .unwrap()
        .remove("verification_assets");
    fixture.configure();
    fixture.policy["checks"][0]["reports"][0]["path"] = json!("source.txt");
    fixture.configure();
    let report = fixture.run(2, &[]);
    assert!(
        report["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("overwrite a captured input")
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("source.txt")).unwrap(),
        "2"
    );
}
