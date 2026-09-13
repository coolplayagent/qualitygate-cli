mod common;
use common::*;
use serde_json::{Value, json};
use std::process::Command;

const DRIVER: &str = r#"
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args[1] == "--version" { println!("Coverage fixture 1"); return; }
    if args[1] == "timeout" { std::thread::sleep(std::time::Duration::from_secs(30)); }
    if args[1] == "skip" { return; }
    let root = std::env::current_dir().unwrap();
    let input = std::fs::read_to_string("input.report").unwrap().replace("$WORKSPACE", root.to_str().unwrap());
    std::fs::create_dir_all("target").unwrap();
    std::fs::write("target/coverage", input).unwrap();
    if args[1] == "mutate" { std::fs::write("src/a.py", "changed after measurement\n").unwrap(); }
}
"#;

fn python(covered: bool) -> Value {
    let count = usize::from(covered);
    let summary = json!({"covered_lines":1+count,"num_statements":2,"missing_lines":1-count,"excluded_lines":0,
        "num_branches":0,"num_partial_branches":0,"covered_branches":0,"missing_branches":0});
    json!({"meta":{"format":3,"version":"7.10.7","branch_coverage":true},"totals":summary,
        "files":{"src/a.py":{"executed_lines":if covered {vec![1,2]} else {vec![1]},"missing_lines":if covered {vec![]} else {vec![2]},"excluded_lines":[],
        "executed_branches":[],"missing_branches":[],"summary":summary}}})
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
            .arg(source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/a.py"), "def value():\n    return 1\n").unwrap();
        std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
        let result = Self {
            root,
            _external: external,
            policy: json!({"schema_version":1,"checks":[{
                "id":"coverage","argv":[binary,"normal"],"timeout_seconds":1,
                "tools":[{"id":"producer","argv":[binary,"--version"]}],
                "reports":[{"path":"target/coverage","format":"coverage_py","mode":"full","coverage_paths":["src/*.py"],"minimum_coverage":100}]
            }]}),
        };
        result.configure();
        result.input(&serde_json::to_string(&python(false)).unwrap());
        git(result.root.path(), &["add", "."]);
        git(
            result.root.path(),
            &["commit", "-qm", "baseline incomplete coverage"],
        );
        result
    }
    fn configure(&self) {
        std::fs::write(
            self.root.path().join("qualitygate.yaml"),
            serde_norway::to_string(&self.policy).unwrap(),
        )
        .unwrap();
    }
    fn input(&self, text: &str) {
        std::fs::write(self.root.path().join("input.report"), text).unwrap();
    }
    fn run(&self, code: i32, args: &[&str]) -> Value {
        let mut command = vec!["check", "--format", "json"];
        command.extend_from_slice(args);
        report(&cli(self.root.path(), &command), code)
    }
}

#[test]
fn native_coverage_repairs_preserve_raw_reports_and_reject_inconsistent_or_absent_evidence() {
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture.run(1, &[])["checks"][0]["metadata"]["target/coverage:coverage"]["line_percent"],
        50.0
    );
    fixture.input(&serde_json::to_string(&python(true)).unwrap());
    fixture.run(0, &[]);
    let mut invalid = python(true);
    invalid["totals"]["num_statements"] = json!(0);
    let text = serde_json::to_string(&invalid).unwrap();
    fixture.input(&text);
    let incomplete = fixture.run(2, &[]);
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
    let artifact = incomplete["checks"][0]["execution"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(artifact["path"].as_str().unwrap()).unwrap(),
        text
    );
    assert_eq!(
        artifact["digest"],
        qualitygate::snapshot::digest(text.as_bytes())
    );
    fixture.input(&serde_json::to_string(&python(true)).unwrap());
    std::fs::write(fixture.root.path().join("src/missing.py"), "missing = 1\n").unwrap();
    fixture.run(2, &[]);
    std::fs::remove_file(fixture.root.path().join("src/missing.py")).unwrap();
    for (mode, expected) in [
        ("skip", "tool_error"),
        ("timeout", "timed_out"),
        ("mutate", "blocked"),
    ] {
        fixture.policy["checks"][0]["argv"][1] = json!(mode);
        fixture.configure();
        let incomplete = fixture.run(2, &[]);
        assert_eq!(
            incomplete["checks"][0]["execution"]["status"], expected,
            "{mode}: {}",
            incomplete["checks"][0]["execution"]["reason"]
        );
        if mode == "mutate" {
            assert_eq!(
                incomplete["checks"][0]["metadata"]["input_integrity"]["after"],
                "invalid"
            );
            assert_eq!(
                std::fs::read_to_string(fixture.root.path().join("src/a.py")).unwrap(),
                "def value():\n    return 1\n"
            );
        }
    }
}

#[test]
fn cobertura_roots_and_staged_coverage_bind_to_the_selected_source_and_policy() {
    let mut fixture = Fixture::new();
    fixture.policy["checks"][0]["reports"][0]["format"] = json!("cobertura");
    fixture.policy["checks"][0]["reports"][0]["require_branch_coverage"] = json!(false);
    fixture.configure();
    let xml = "<coverage lines-valid='2' lines-covered='2' branches-valid='0' branches-covered='0'><sources><source>$WORKSPACE/src</source></sources><class filename='a.py'><lines><line number='1' hits='1'/><line number='2' hits='1'/></lines></class></coverage>";
    fixture.input(xml);
    std::fs::create_dir(fixture.root.path().join("tests")).unwrap();
    std::fs::write(fixture.root.path().join("tests/a.py"), "other = 1\n").unwrap();
    fixture.run(0, &[]);
    fixture.input(&xml.replace("</sources>", "<source>$WORKSPACE/tests</source></sources>"));
    fixture.run(2, &[]);
    fixture.input(xml);
    git(fixture.root.path(), &["add", "."]);
    git(
        fixture.root.path(),
        &["commit", "-qm", "measured source roots"],
    );
    fixture.policy["checks"][0]["reports"][0]["mode"] = json!("changed_lines");
    fixture.configure();
    std::fs::write(
        fixture.root.path().join("src/a.py"),
        "def value():\n    return 2\n",
    )
    .unwrap();
    git(
        fixture.root.path(),
        &["add", "src/a.py", "qualitygate.yaml"],
    );
    fixture.input(
        &xml.replace("number='2' hits='1'", "number='2' hits='0'")
            .replace("lines-covered='2'", "lines-covered='1'"),
    );
    let staged = fixture.run(0, &["--staged"]);
    assert_eq!(
        staged["checks"][0]["metadata"]["target/coverage:coverage"]["lines"],
        1
    );
    fixture.run(1, &[]);
    fixture.policy["checks"][0]["reports"][0]["minimum_coverage"] = json!(0);
    fixture.configure();
    fixture.run(0, &[]);
    fixture.run(2, &["--policy-ref", "HEAD"]);
}
