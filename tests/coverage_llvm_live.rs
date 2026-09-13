//! Real LLVM LCOV exporter evidence, separate from toolchain-free integration.
mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const DRIVER: &str = r#"
use std::process::Command;
fn run(program: &str, args: &[&str]) {
    let status = Command::new(program).args(args).status().unwrap();
    if !status.success() { std::process::exit(status.code().unwrap_or(99)); }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    std::fs::create_dir_all("target").unwrap();
    let binary = if cfg!(windows) { "target/program.exe" } else { "target/program" };
    run(&args[1], &["-C", "instrument-coverage", "src/main.rs", "-o", binary]);
    let status = Command::new(binary).env("LLVM_PROFILE_FILE", "target/run.profraw").status().unwrap();
    if !status.success() { std::process::exit(status.code().unwrap_or(99)); }
    run(&args[3], &["merge", "-sparse", "target/run.profraw", "-o", "target/run.profdata"]);
    let output = Command::new(&args[2]).args(["export", "-format=lcov", "-instr-profile=target/run.profdata", binary]).output().unwrap();
    if !output.status.success() { eprintln!("{}", String::from_utf8_lossy(&output.stderr)); std::process::exit(99); }
    std::fs::write("target/coverage.info", output.stdout).unwrap();
}
"#;

fn output(program: &str, args: &[&str]) -> String {
    let output = Command::new(program).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires pinned Rust LLVM tools; mandatory coverage-producers CI job"]
fn rust_lcov_zero_counters_do_not_claim_branch_measurement() {
    let root = fixture();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("driver.rs");
    let binary = external.path().join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    std::fs::write(&source, DRIVER).unwrap();
    output(
        "rustc",
        &[source.to_str().unwrap(), "-o", binary.to_str().unwrap()],
    );
    let sysroot = output("rustc", &["--print", "sysroot"]);
    let version = output("rustc", &["-vV"]);
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();
    let tools = Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin");
    let cov = tools.join(if cfg!(windows) {
        "llvm-cov.exe"
    } else {
        "llvm-cov"
    });
    let profdata = tools.join(if cfg!(windows) {
        "llvm-profdata.exe"
    } else {
        "llvm-profdata"
    });
    assert!(
        cov.is_file() && profdata.is_file(),
        "Install pinned llvm-tools-preview"
    );
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("src/main.rs"),
        "fn main() { if std::env::args().count() > 1 { println!(\"argument\"); } }\n",
    )
    .unwrap();
    std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"coverage","argv":[binary,"rustc",cov,profdata],"timeout_seconds":60,
        "tools":[{"id":"rustc","argv":["rustc","-vV"]},
            {"id":"llvm-cov","argv":[cov,"--version"]},
            {"id":"llvm-profdata","argv":[profdata,"--version"]}],
        "reports":[{"path":"target/coverage.info","format":"lcov","mode":"full","coverage_paths":["src/*.rs"],"minimum_coverage":100}]
    }]});
    let run = |policy: &Value, code| {
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            serde_norway::to_string(policy).unwrap(),
        )
        .unwrap();
        report(&cli(root.path(), &["check", "--format", "json"]), code)
    };
    let incomplete = run(&policy, 2);
    assert!(
        incomplete["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("branch coverage")
    );
    let artifact = incomplete["checks"][0]["execution"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"].as_str().unwrap().ends_with("report-0"))
        .unwrap();
    let text = std::fs::read_to_string(artifact["path"].as_str().unwrap()).unwrap();
    assert!(text.contains("BRF:0\nBRH:0"));
    assert!(!text.contains("BRDA:"));
    policy["checks"][0]["reports"][0]["require_branch_coverage"] = json!(false);
    let lines = run(&policy, 0);
    let evidence = &lines["checks"][0]["metadata"]["target/coverage.info:coverage"];
    assert_eq!(evidence["line_percent"], 100.0);
    assert_eq!(evidence["branch_percent"], Value::Null);
    assert_eq!(evidence["branch_measurement"], false);
}
