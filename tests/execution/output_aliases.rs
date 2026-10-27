use super::*;
use std::process::Command;

const PRODUCER: &str = r#"
fn main() {
    use std::io::Write;
    let args: Vec<_> = std::env::args().collect();
    let probe = args.iter().any(|arg| arg == "--version");
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&args[1]).unwrap();
    writeln!(log, "{}", if probe { "probe" } else { "producer" }).unwrap();
    if probe { println!("output fixture 1"); return; }
    std::fs::create_dir_all("target").unwrap();
    let data = "{\"issues\":[],\"tests\":{\"executed\":1,\"failures\":0,\"skipped\":0}}";
    for name in ["first", "second"] { std::fs::write(format!("target/{name}.json"), data).unwrap(); }
    println!("{data}");
}
"#;

#[test]
fn aliased_outputs_are_rejected_before_probes_and_distinct_reports_still_count_independently() {
    let root = fixture();
    let root = root.path();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("producer.rs");
    let binary = external.path().join("producer.exe");
    let log = external.path().join("attempts");
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
    let first = json!({"path":"target/first.json","format":"diagnostics","minimum_tests":1});
    let second = json!({"path":"target/second.json","format":"diagnostics","minimum_tests":1});
    let command = |reports, projects| {
        json!({"id":"counts","argv":[binary,log],
        "tools":[{"id":"producer","argv":[binary,log,"--version"]}],
        "reports":reports,"projects":projects})
    };
    let mut aliased = first.clone();
    aliased["path"] = json!("./target//first.json");
    let mut base_first = first.clone();
    base_first["mode"] = json!("new_diagnostics");
    base_first["baseline"] = json!("target/first.json");
    let mut base_second = second.clone();
    base_second["mode"] = json!("new_diagnostics");
    base_second["baseline"] = json!("./target/first.json");
    let project = json!([{"root":".","effective_pom":"target/first.json","dependency_tree":"target/tree.json"}]);
    for invalid in [
        command(json!([first, aliased]), json!([])),
        command(json!([aliased]), project),
        command(json!([base_first, base_second]), json!([])),
    ] {
        policy(root, json!([invalid]));
        for args in [
            vec!["check", "--format", "json"],
            vec!["doctor", "--probe-tools", "--format", "json"],
        ] {
            let result = report(&cli(root, &args), 2);
            assert!(
                result.to_string().contains("distinct after normalization"),
                "{result}"
            );
            assert!(
                !log.exists(),
                "invalid output identities must not launch probes or producers"
            );
        }
    }
    for baseline in [false, true] {
        let mut reports = json!([first, second]);
        reports[0]["path"] = json!("./target/first.json");
        if baseline {
            for report in reports.as_array_mut().unwrap() {
                report["mode"] = json!("new_diagnostics");
                report["baseline"] = report["path"].clone();
            }
        }
        policy(root, json!([command(reports, json!([]))]));
        let result = report(&cli(root, &["check", "--format", "json"]), 0);
        let check = &result["checks"][0];
        assert_eq!(check["matched_entities"], 2);
        let artifacts: Vec<_> = check["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| {
                a["path"].as_str().unwrap().ends_with("-report-0")
                    || a["path"].as_str().unwrap().ends_with("-report-1")
            })
            .collect();
        assert_eq!(artifacts.len(), if baseline { 4 } else { 2 });
        assert!(
            artifacts
                .iter()
                .all(|a| a["digest"] == artifacts[0]["digest"])
        );
        let attempts = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            attempts,
            if baseline {
                "probe\nproducer\nprobe\nproducer\n"
            } else {
                "probe\nproducer\n"
            }
        );
        std::fs::remove_file(&log).unwrap();
    }
}
