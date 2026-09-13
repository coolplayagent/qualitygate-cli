//! Real coverage producers; fixture orchestration and assertions are Rust.
mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const JACOCO_URL: &str =
    "https://repo.maven.apache.org/maven2/org/jacoco/jacoco/0.8.15/jacoco-0.8.15.zip";
const JACOCO_SHA: &str = "sha256:5b3f6ddb724e761d25c937d68b0189a3a23f3e220e3282575ee0b53359e8110e";
const JAVA: &str = "public class Api {\n  public int sign(int value) {\n    if (value > 0) return 1;\n    return -1;\n  }\n  public int old() { return 0; }\n}\n";
const PYTHON: &str = "def sign(value):\n    if value > 0:\n        return 1\n    return -1\n\ndef old():\n    return 0\n";
const DRIVER: &str = r#"
use std::process::Command;
fn run(program: &str, argv: &[&str]) -> i32 {
    Command::new(program).args(argv).status().unwrap().code().unwrap_or(99)
}
fn require(program: &str, argv: &[&str]) {
    let code = run(program, argv);
    if code != 0 { std::process::exit(code); }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--version") { println!("Coverage fixture driver 1"); return; }
    std::fs::create_dir_all("target/classes").unwrap();
    if args[1] == "java" {
        require(&args[2], &[if args[4] == "no-debug" { "-g:none" } else { "-g" }, "-d", "target/classes", "src/Api.java", "tests/Main.java"]);
        require(&args[3], &["-ea", "-javaagent:tools/jacocoagent.jar=destfile=target/jacoco.exec", "-cp", "target/classes", "Main"]);
        require(&args[3], &["-jar", "tools/jacococli.jar", "report", "target/jacoco.exec", "--classfiles", "target/classes", "--sourcefiles", "src", "--sourcefiles", "tests", "--xml", "target/jacoco.xml"]);
    } else {
        let mut command = vec!["-I", "-m", "coverage", "run"];
        if args[3] == "branch" { command.push("--branch"); }
        command.extend(["--source", "src", "-m", "pytest", "-q", "--junitxml=target/junit.xml", "tests"]);
        let code = run(&args[2], &command);
        if ![0,1].contains(&code) { std::process::exit(code); }
        require(&args[2], &["-I", "-m", "coverage", "xml", "-o", "target/coverage.xml"]);
        require(&args[2], &["-I", "-m", "coverage", "json", "-o", "target/coverage.json"]);
        std::process::exit(code);
    }
}
"#;

fn driver(root: &Path) -> PathBuf {
    let source = root.join("driver.rs");
    let binary = root.join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    std::fs::write(&source, DRIVER).unwrap();
    let output = Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn jacoco(root: &Path) {
    let bytes = if let Ok(path) = std::env::var("QUALITYGATE_TEST_JACOCO") {
        std::fs::read(path).unwrap()
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(60))
                    .build()
                    .unwrap();
                let mut response = client
                    .get(JACOCO_URL)
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.unwrap() {
                    bytes.extend_from_slice(&chunk);
                    assert!(
                        bytes.len() <= 8 * 1024 * 1024,
                        "JaCoCo download exceeded budget"
                    );
                }
                bytes
            })
    };
    assert_eq!(qualitygate::snapshot::digest(&bytes), JACOCO_SHA);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    std::fs::create_dir_all(root.join("tools")).unwrap();
    for name in ["jacocoagent.jar", "jacococli.jar"] {
        let file = archive.by_name(&format!("lib/{name}")).unwrap();
        assert!(file.size() <= 2 * 1024 * 1024);
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 2 * 1024 * 1024);
        std::fs::write(root.join("tools").join(name), bytes).unwrap();
    }
}

fn configure(root: &Path, policy: &Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(policy).unwrap(),
    )
    .unwrap();
}

fn run(root: &Path, code: i32) -> Value {
    let output = cli(root, &["check", "--format", "json"]);
    if output.status.code() != Some(code)
        && let Ok(value) = serde_json::from_slice::<Value>(&output.stdout)
        && let Some(artifacts) = value["checks"][0]["execution"]["artifacts"].as_array()
    {
        for artifact in artifacts {
            let path = artifact["path"].as_str().unwrap();
            if path.contains("-report-") {
                eprintln!("{path}: {}", std::fs::read_to_string(path).unwrap());
            }
        }
    }
    report(&output, code)
}

fn java_tests(root: &Path, checks: &str) {
    std::fs::write(root.join("tests/Main.java"), format!("public class Main {{ public static void main(String[] args) {{ Api api = new Api(); assert api.sign(1) == 1; {checks} }} }}\n")).unwrap();
}

#[test]
#[ignore = "requires JDK 21 and digest-pinned JaCoCo; mandatory coverage-producers CI job"]
fn real_jacoco_standard_xml_changed_lines_branches_and_missing_debug_evidence() {
    let root = fixture();
    let root = root.path();
    let external = tempfile::tempdir().unwrap();
    let driver = driver(external.path());
    let javac = std::env::var("QUALITYGATE_TEST_JAVAC").unwrap_or_else(|_| "javac".into());
    let java = std::env::var("QUALITYGATE_TEST_JAVA").unwrap_or_else(|_| "java".into());
    jacoco(root);
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("src/Api.java"), JAVA).unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    java_tests(root, "");
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"coverage","argv":[driver,"java",javac,java,"debug"],"timeout_seconds":90,
        "tools":[{"id":"javac","argv":[javac,"-version"]},
            {"id":"java","argv":[java,"-version"]},
            {"id":"jacoco","argv":[java,"-jar","tools/jacococli.jar","version"],"inputs":["tools/jacococli.jar","tools/jacocoagent.jar"]}],
        "reports":[{"path":"target/jacoco.xml","format":"jacoco","mode":"full","coverage_paths":["src/*.java"],"minimum_coverage":100}]
    }]});
    configure(root, &policy);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "baseline partial branch coverage"]);
    let partial = run(root, 1);
    assert_eq!(
        partial["checks"][0]["metadata"]["target/jacoco.xml:coverage"]["branch_percent"],
        50.0
    );
    let raw = partial["checks"][0]["execution"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
        .unwrap();
    assert!(
        std::fs::read_to_string(raw["path"].as_str().unwrap())
            .unwrap()
            .contains("<!DOCTYPE report PUBLIC")
    );
    std::fs::write(
        root.join("src/Api.java"),
        JAVA.replace("value > 0", "value >= 1")
            .replace("return -1", "return -2"),
    )
    .unwrap();
    policy["checks"][0]["reports"][0]["mode"] = json!("changed_lines");
    configure(root, &policy);
    run(root, 1);
    java_tests(root, "assert api.sign(0) == -2;");
    let repaired = run(root, 0);
    assert_eq!(
        repaired["checks"][0]["metadata"]["target/jacoco.xml:coverage"]["branch_percent"],
        100.0
    );
    policy["checks"][0]["reports"][0]["mode"] = json!("full");
    configure(root, &policy);
    run(root, 1);
    java_tests(root, "assert api.sign(0) == -2; assert api.old() == 0;");
    run(root, 0);
    std::fs::write(root.join("src/Missing.java"), "class Missing {}\n").unwrap();
    run(root, 2);
    std::fs::remove_file(root.join("src/Missing.java")).unwrap();
    policy["checks"][0]["argv"][4] = json!("no-debug");
    configure(root, &policy);
    run(root, 2);
    policy["checks"][0]["argv"][4] = json!("debug");
    configure(root, &policy);
    std::fs::write(root.join("src/Api.java"), "public class Api { broken\n").unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}

fn python_tests(root: &Path, assertions: &str) {
    std::fs::write(root.join("tests/test_calc.py"), format!("from src.calc import sign, old\n\ndef test_sign():\n    assert sign(1) == 1\n{assertions}")).unwrap();
}

#[test]
#[ignore = "requires coverage.py 7.10.7 and pytest 8.4.2; mandatory coverage-producers CI job"]
fn real_python_coverage_reports_repair_and_reject_line_only_branch_claims() {
    let root = fixture();
    let root = root.path();
    let external = tempfile::tempdir().unwrap();
    let driver = driver(external.path());
    let python =
        std::env::var("QUALITYGATE_TEST_COVERAGE_PYTHON").unwrap_or_else(|_| "python3".into());
    for (module, expected) in [("coverage", "7.10.7"), ("pytest", "8.4.2")] {
        let output = Command::new(&python)
            .args(["-I", "-m", module, "--version"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains(expected));
    }
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("src/__init__.py"), "").unwrap();
    std::fs::write(root.join("tests/__init__.py"), "").unwrap();
    std::fs::write(root.join("src/calc.py"), PYTHON).unwrap();
    std::fs::write(
        root.join(".gitignore"),
        "target/\n.coverage\n__pycache__/\n.pytest_cache/\n",
    )
    .unwrap();
    python_tests(root, "");
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"coverage","argv":[driver,"python",python,"branch"],"timeout_seconds":90,"findings_exit_codes":[1],
        "tools":[{"id":"python","argv":[python,"-I","--version"]},
            {"id":"coverage","argv":[python,"-I","-m","coverage","--version"]},
            {"id":"pytest","argv":[python,"-I","-m","pytest","--version"]}],
        "reports":[{"path":"target/junit.xml","format":"junit","minimum_tests":1},
            {"path":"target/coverage.xml","format":"cobertura","mode":"full","coverage_paths":["src/**/*.py"],"minimum_coverage":100},
            {"path":"target/coverage.json","format":"coverage_py","mode":"full","coverage_paths":["src/**/*.py"],"minimum_coverage":100}]
    }]});
    configure(root, &policy);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "baseline Python branch coverage"]);
    let partial = run(root, 1);
    assert_eq!(
        partial["checks"][0]["metadata"]["target/coverage.xml:coverage"]["branch_percent"],
        50.0
    );
    assert_eq!(
        partial["checks"][0]["metadata"]["target/coverage.json:coverage"]["branch_percent"],
        50.0
    );
    std::fs::write(
        root.join("src/calc.py"),
        PYTHON
            .replace("value > 0", "value >= 1")
            .replace("return -1", "return -2"),
    )
    .unwrap();
    policy["checks"][0]["reports"][1]["mode"] = json!("changed_lines");
    policy["checks"][0]["reports"][2]["mode"] = json!("changed_lines");
    configure(root, &policy);
    run(root, 1);
    python_tests(root, "    assert sign(0) == -2\n");
    run(root, 0);
    policy["checks"][0]["reports"][1]["mode"] = json!("full");
    policy["checks"][0]["reports"][2]["mode"] = json!("full");
    configure(root, &policy);
    run(root, 1);
    python_tests(root, "    assert sign(0) == -2\n    assert old() == 0\n");
    run(root, 0);
    policy["checks"][0]["argv"][3] = json!("lines");
    configure(root, &policy);
    run(root, 2);
    policy["checks"][0]["reports"][1]["require_branch_coverage"] = json!(false);
    policy["checks"][0]["reports"][2]["require_branch_coverage"] = json!(false);
    configure(root, &policy);
    run(root, 0);
    policy["checks"][0]["argv"][3] = json!("branch");
    policy["checks"][0]["reports"][1]["require_branch_coverage"] = json!(true);
    policy["checks"][0]["reports"][2]["require_branch_coverage"] = json!(true);
    configure(root, &policy);
    std::fs::write(root.join("src/calc.py"), "def sign(value):\n    return 1\n\ndef old():\n    return 0\n\ndef ignored(): # pragma: no cover\n    return 8\n").unwrap();
    python_tests(root, "    assert old() == 0\n");
    // Cobertura alone cannot establish whether zero branches were measured.
    run(root, 2);
    policy["checks"][0]["reports"][1]["require_branch_coverage"] = json!(false);
    configure(root, &policy);
    let measured = run(root, 0);
    let evidence = &measured["checks"][0]["metadata"]["target/coverage.json:coverage"];
    assert_eq!(evidence["branch_measurement"], true);
    assert_eq!(evidence["branch_percent"], Value::Null);
    assert_eq!(evidence["excluded_lines"], 2);
    assert_eq!(evidence["line_percent"], 100.0);
    assert_eq!(evidence["producer"]["version"], "7.10.7");
    python_tests(root, "    assert old() == 99\n");
    let failed = run(root, 1);
    assert_eq!(failed["checks"][0]["execution"]["status"], "completed");
    assert!(
        failed["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["message"].as_str().unwrap().contains("99"))
    );
    std::fs::write(root.join("src/calc.py"), "def invalid(\n").unwrap();
    let incomplete = run(root, 2);
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
}
