use super::*;
use std::process::Command;

#[test]
fn contradictory_junit_is_incomplete_with_retained_report_and_blocked_dependents() {
    let root = fixture();
    let root = root.path();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("producer.rs");
    let binary = external.path().join("producer.exe");
    std::fs::write(
        &source,
        r#"
fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("junit fixture 1");
        return;
    }
    std::fs::create_dir_all("target").unwrap();
    std::fs::copy("report.input", "target/tests.xml").unwrap();
}
"#,
    )
    .unwrap();
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
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let checks = json!([
        {"id":"tests","argv":[binary],"tools":[{"id":"producer","argv":[binary,"--version"]}],
         "reports":[{"path":"target/tests.xml","format":"junit","mode":"full","minimum_tests":1}]},
        {"id":"dependent","argv":["git","--version"],"depends_on":["tests"]}
    ]);
    policy(root, checks.clone());
    let deep = format!(
        "{}<testcase/>{}",
        "<testsuite>".repeat(2_000),
        "</testsuite>".repeat(2_000)
    );
    let attributes = format!(
        "<testsuite {}><testcase/></testsuite>",
        (0..1_025).map(|i| format!("a{i}='v' ")).collect::<String>()
    );
    let document_attributes = format!(
        "<testsuite>{}</testsuite>",
        "<testcase a='1' b='2'/>".repeat(50_001)
    );
    let namespace_work = format!(
        "<testsuite {}>{}</testsuite>",
        (0..1_000)
            .map(|i| format!("xmlns:p{i}='urn:{i}' "))
            .collect::<String>(),
        "<testcase xmlns:q='urn:q'/>".repeat(10)
    );
    for (xml, reason) in [
        (deep.as_str(), "nesting exceeds"),
        (attributes.as_str(), "element exceeds 1024 attributes"),
        (namespace_work.as_str(), "namespace resolution exceeds"),
        (
            document_attributes.as_str(),
            "document exceeds 100000 attributes",
        ),
        (
            "<testsuite tests='1' failures='1'><testcase/></testsuite>",
            "failures counter",
        ),
        (
            "<testsuite tests='1' errors='1'><testcase/></testsuite>",
            "errors counter",
        ),
        (
            "<testsuite tests='1' skipped='1'><testcase/></testsuite>",
            "skipped counter",
        ),
        (
            "<testsuites tests='1'><testsuite tests='2'><testcase/></testsuite></testsuites>",
            "tests counter",
        ),
        (
            "<testsuite><testcase/><testcase><skipped/><failure/></testcase></testsuite>",
            "contradictory outcomes",
        ),
        (
            "<testsuite><testcase/><failure/></testsuite>",
            "outcome outside",
        ),
    ] {
        std::fs::write(root.join("report.input"), xml).unwrap();
        let value = report(&cli(root, &["check", "--format", "json"]), 2);
        assert_eq!(value["gate"]["complete"], false);
        let check = &value["checks"][0];
        assert_eq!(check["execution"]["status"], "tool_error");
        assert!(
            check["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains(reason)
        );
        assert!(check["verdict"].is_null());
        assert!(check["metadata"]["tests"].is_null());
        assert!(check["diagnostics"].as_array().unwrap().is_empty());
        let artifact = check["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
            .unwrap();
        assert_eq!(
            std::fs::read(artifact["path"].as_str().unwrap()).unwrap(),
            xml.as_bytes()
        );
        assert_eq!(
            artifact["digest"],
            qualitygate::snapshot::digest(xml.as_bytes())
        );
        assert_eq!(artifact["bytes"], xml.len());
        assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
        assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
    }
    let failed = "<testsuites tests='2' failures='1' errors='1' skipped='0'><testsuite><testcase><failure message='first'/><failure message='second'/></testcase><testcase><error/></testcase></testsuite></testsuites>";
    std::fs::write(root.join("report.input"), failed).unwrap();
    // A valid failing producer remains a violation while its required dependent
    // keeps the overall gate incomplete. With no blocked check, the gate exits 1.
    let value = report(&cli(root, &["check", "--format", "json"]), 2);
    assert_eq!(value["checks"][0]["verdict"], "fail");
    assert_eq!(value["checks"][0]["execution"]["status"], "completed");
    assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
    assert_eq!(
        value["checks"][0]["metadata"]["tests"],
        json!({"executed":2,"failures":2,"skipped":0})
    );
    assert!(
        value["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("first; second"))
    );
    policy(root, json!([checks[0]]));
    let complete_failure = report(&cli(root, &["check", "--format", "json"]), 1);
    assert_eq!(complete_failure["gate"]["complete"], true);
    assert_eq!(complete_failure["checks"][0]["verdict"], "fail");

    policy(root, checks);
    std::fs::write(root.join("report.input"), "<testsuites tests='1' failures='0' errors='0' skipped='0'><testsuite><testcase/></testsuite></testsuites>").unwrap();
    let repaired = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(repaired["gate"]["complete"], true);
    assert_eq!(repaired["checks"][0]["verdict"], "pass");
    assert_eq!(
        repaired["checks"][0]["metadata"]["tests"],
        json!({"executed":1,"failures":0,"skipped":0})
    );
    assert_eq!(repaired["checks"][1]["verdict"], "pass");
}
