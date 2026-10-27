use super::*;
use std::process::Command;

const PRODUCER: &str = r#"
fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("counter fixture 1");
        return;
    }
    std::fs::create_dir_all("target").unwrap();
    for name in ["first", "second"] {
        std::fs::copy(format!("{name}.input"), format!("target/{name}.json")).unwrap();
    }
}
"#;

fn tests(executed: usize, failures: usize) -> Value {
    json!({"issues":[],"tests":{"executed":executed,"failures":failures,"skipped":0}})
}

#[test]
fn report_counter_overflow_is_incomplete_with_preserved_evidence_and_blocked_dependents() {
    let root = fixture();
    let root = root.path();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("producer.rs");
    let binary = external.path().join("producer.exe");
    std::fs::write(&source, PRODUCER).unwrap();
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
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn value() {}\n").unwrap();

    for coverage in [false, true] {
        let (second, second_spec, second_count) = if coverage {
            (
                json!({"issues":[],"branch_coverage":true,"coverage_files":["src/a.rs"],"coverage":[{
                    "file":"src/a.rs","line":1,"hits":1,"branches_found":0,"branches_hit":0
                }]}),
                json!({"path":"target/second.json","format":"diagnostics","coverage_paths":["src/*.rs"],"minimum_coverage":100}),
                1,
            )
        } else {
            (
                tests(3, 0),
                json!({"path":"target/second.json","format":"diagnostics","minimum_tests":1}),
                3,
            )
        };
        policy(
            root,
            json!([
                {"id":"counts","argv":[binary],"tools":[{"id":"producer","argv":[binary,"--version"]}],
                 "reports":[{"path":"target/first.json","format":"diagnostics","minimum_tests":1},second_spec]},
                {"id":"dependent","argv":["git","--version"],"depends_on":["counts"]}
            ]),
        );
        let second_bytes = serde_json::to_vec(&second).unwrap();
        std::fs::write(root.join("second.input"), &second_bytes).unwrap();
        for failures in [0, 1] {
            let first_bytes = serde_json::to_vec(&tests(usize::MAX, failures)).unwrap();
            std::fs::write(root.join("first.input"), &first_bytes).unwrap();
            let output = cli(root, &["check", "--format", "json"]);
            assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
            let value = report(&output, 2);
            assert_eq!(value["gate"]["complete"], false);
            let counts = &value["checks"][0];
            assert_eq!(counts["execution"]["status"], "tool_error");
            assert!(
                counts["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains("Matched entity count overflow"),
                "coverage={coverage}, failures={failures}: {}",
                counts["execution"]["reason"]
            );
            assert!(counts["verdict"].is_null());
            assert_eq!(counts["matched_entities"], usize::MAX);
            assert_eq!(counts["metadata"]["tests"]["executed"], usize::MAX);
            assert_eq!(counts["diagnostics"].as_array().unwrap().len(), failures);
            for (index, bytes) in [&first_bytes, &second_bytes].into_iter().enumerate() {
                let artifact = counts["execution"]["artifacts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|artifact| {
                        artifact["path"]
                            .as_str()
                            .unwrap()
                            .ends_with(&format!("report-{index}"))
                    })
                    .unwrap();
                assert_eq!(
                    std::fs::read(artifact["path"].as_str().unwrap()).unwrap(),
                    *bytes
                );
                assert_eq!(artifact["digest"], qualitygate::snapshot::digest(bytes));
            }
            assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
            assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
        }
        std::fs::write(
            root.join("first.input"),
            serde_json::to_vec(&tests(2, 0)).unwrap(),
        )
        .unwrap();
        let repaired = report(&cli(root, &["check", "--format", "json"]), 0);
        assert_eq!(repaired["checks"][0]["matched_entities"], 2 + second_count);
        assert_eq!(repaired["checks"][1]["verdict"], "pass");
    }
}
