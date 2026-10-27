use super::*;
use std::process::Command;

const PRODUCER: &str = r#"
fn main() {
    use std::io::Write;
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("preparation fixture 1");
        return;
    }
    let mut marker = std::fs::OpenOptions::new().create(true).append(true).open(&args[1]).unwrap();
    writeln!(marker, "checked_input_present={}", std::path::Path::new("hello.txt").is_file()).unwrap();
    if args[2] == "stdout" {
        println!("{{\"issues\":[]}}");
    } else if args[2] != "noop" {
        std::fs::create_dir_all("target").unwrap();
        std::fs::write(&args[2], "{\"issues\":[]}").unwrap();
    }
}
"#;

struct Producer {
    _external: tempfile::TempDir,
    binary: std::path::PathBuf,
    marker: std::path::PathBuf,
}

impl Producer {
    fn new() -> Self {
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
        Self {
            binary,
            marker: external.path().join("producer-ran"),
            _external: external,
        }
    }

    fn check(&self, output: &str) -> Value {
        json!({"id":"producer","argv":[self.binary,self.marker,output],
            "tools":[{"id":"producer","argv":[self.binary,"--version"]}],
            "reports":[{"path":"target/report.json","format":"diagnostics"}]})
    }

    fn attempts(&self) -> Vec<String> {
        match std::fs::read_to_string(&self.marker) {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(error) => panic!("{error}"),
        }
    }
}

fn configure(root: &Path, producer: Value) {
    policy(
        root,
        json!([
            producer,
            {"id":"independent","argv":["git","--version"]},
            {"id":"dependent","argv":["git","--version"],"depends_on":["producer"]}
        ]),
    );
}

#[test]
fn report_preparation_rejects_input_collisions_without_launching_or_poisoning_other_checks() {
    let root = fixture();
    let root = root.path();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(
        root.join("pom.xml"),
        "<project><artifactId>fixture</artifactId></project>",
    )
    .unwrap();
    let producer = Producer::new();
    for project in [false, true] {
        let mut command = producer.check("target/report.json");
        if project {
            command["reports"] = json!([]);
            command["projects"] = json!([{"root":".","effective_pom":"hello.txt","dependency_tree":"target/tree.json"}]);
        } else {
            command["reports"][0]["path"] = json!("./hello.txt");
        }
        configure(root, command);
        let doctor = report(&cli(root, &["doctor", "--format", "json"]), 2);
        assert_eq!(doctor["complete"], false);
        assert!(
            doctor["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "command.output_overwrites_input")
        );
        assert!(producer.attempts().is_empty());
        let result = report(&cli(root, &["check", "--format", "json"]), 2);
        let first = &result["checks"][0];
        assert_eq!(first["execution"]["status"], "tool_error");
        assert!(
            first["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("would overwrite a checked input")
        );
        assert!(first["execution"]["started_at_ms"].is_null());
        assert_eq!(first["metadata"]["input_integrity"]["after"], "verified");
        assert_eq!(result["checks"][1]["verdict"], "pass");
        assert_eq!(result["checks"][2]["execution"]["status"], "blocked");
        assert!(producer.attempts().is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("hello.txt")).unwrap(),
            "initial\n"
        );
    }
    // Stdout report paths are evidence labels, so they need no writable file.
    let mut stdout = producer.check("stdout");
    stdout["reports"][0]["path"] = json!("./hello.txt");
    stdout["reports"][0]["from_stdout"] = json!(true);
    configure(root, stdout);
    assert_eq!(
        report(&cli(root, &["doctor", "--format", "json"]), 0)["complete"],
        true
    );
    assert_eq!(
        report(&cli(root, &["check", "--format", "json"]), 0)["gate"]["decision"],
        "pass"
    );
    assert_eq!(producer.attempts(), ["checked_input_present=true"]);
}

#[test]
fn baseline_collisions_are_detected_and_generated_reports_still_require_fresh_production() {
    let root = fixture();
    let root = root.path();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let producer = Producer::new();
    let mut command = producer.check("target/report.json");
    command["reports"][0]["mode"] = json!("new_diagnostics");
    command["reports"][0]["baseline"] = json!("./hello.txt");
    configure(root, command);
    let doctor = report(&cli(root, &["doctor", "--format", "json"]), 2);
    assert!(
        doctor["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "command.baseline_output_overwrites_input")
    );
    let result = report(&cli(root, &["check", "--format", "json"]), 2);
    assert!(
        result["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("would overwrite a checked input")
    );
    assert_eq!(result["checks"][1]["verdict"], "pass");
    assert_eq!(producer.attempts(), ["checked_input_present=true"]);

    let mut seed = producer.check("target/report.json");
    seed["id"] = json!("seed");
    seed["reports"] = json!([]);
    policy(root, json!([seed, producer.check("noop")]));
    let result = report(&cli(root, &["check", "--format", "json"]), 2);
    assert_eq!(result["checks"][0]["verdict"], "pass");
    assert!(
        result["checks"][1]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("not produced")
    );
    assert_eq!(
        result["checks"][1]["metadata"]["input_integrity"]["after"],
        "verified"
    );
    assert_eq!(producer.attempts(), ["checked_input_present=true"; 3]);
    configure(root, producer.check("target/report.json"));
    assert_eq!(
        report(&cli(root, &["check", "--format", "json"]), 0)["gate"]["decision"],
        "pass"
    );
}
