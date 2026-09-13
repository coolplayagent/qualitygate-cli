//! Real Clippy -> clippy-sarif -> qualitygate acceptance, separate from parser fixtures.
mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const ORIGINAL: &str = "pub fn empty(values: &[u8]) -> bool { values.len() == 0 }\n";
const NEW: &str = "pub fn text() -> String { format!(\"text\") }\n";
const DRIVER: &str = r#"
use std::{io::Read, process::{Command, Stdio}};
fn main() {
    if std::env::args().any(|arg| arg == "--version") { println!("Clippy SARIF fixture driver 1"); return; }
    let converter = std::env::args().nth(1).unwrap();
    let mut child = Command::new("cargo")
        .args(["clippy", "--locked", "--offline", "--message-format=json", "--", "-W", "clippy::len_zero", "-W", "clippy::useless_format"])
        .stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap();
    let mut bytes = Vec::new();
    child.stdout.take().unwrap().take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes).unwrap();
    if bytes.len() > 2 * 1024 * 1024 {
        child.kill().unwrap(); child.wait().unwrap(); panic!("Clippy output exceeded budget");
    }
    let status = child.wait().unwrap();
    if !status.success() { std::process::exit(status.code().unwrap_or(1)); }
    std::fs::write("target/clippy.json", bytes).unwrap();
    let status = Command::new(converter).args(["--input", "target/clippy.json", "--output", "target/clippy.sarif"])
        .status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
"#;

fn run(root: &Path, code: i32) -> Value {
    report(&cli(root, &["check", "--format", "json"]), code)
}

#[test]
#[ignore = "requires pinned Clippy and clippy-sarif 0.8.0; mandatory SARIF acceptance CI job"]
fn real_clippy_sarif_incremental_repair_and_compile_failure() {
    let root = fixture();
    let external = tempfile::tempdir().unwrap();
    let converter =
        std::env::var("QUALITYGATE_TEST_CLIPPY_SARIF").unwrap_or_else(|_| "clippy-sarif".into());
    let version = Command::new(&converter).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        "clippy-sarif 0.8.0"
    );
    let source = external.path().join("driver.rs");
    let driver = external.path().join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    std::fs::write(&source, DRIVER).unwrap();
    let compiled = Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&driver)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), ORIGINAL).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"sarif-live-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.path().join("rust-toolchain.toml"),
        include_str!("../rust-toolchain.toml"),
    )
    .unwrap();
    std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
    let lock = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"clippy","argv":[driver,converter],"timeout_seconds":90,
        "tools":[{"id":"clippy","argv":["cargo","clippy","--version"]},
            {"id":"converter","argv":[converter,"--version"]},
            {"id":"driver","argv":[driver,"--version"]}],
        "reports":[{"path":"target/clippy.sarif","format":"sarif","mode":"new_diagnostics","baseline":"target/clippy.sarif"}]
    }]});
    let configure = |policy: &Value| {
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            serde_norway::to_string(policy).unwrap(),
        )
        .unwrap()
    };
    configure(&policy);
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "historical Clippy finding"]);
    let original = run(root.path(), 0);
    assert_eq!(
        original["checks"][0]["metadata"]["target/clippy.sarif:filtered"],
        1
    );
    std::fs::write(root.path().join("src/lib.rs"), format!("\n{ORIGINAL}")).unwrap();
    run(root.path(), 0);
    std::fs::write(root.path().join("src/lib.rs"), format!("\n{ORIGINAL}{NEW}")).unwrap();
    let failed = run(root.path(), 1);
    let diagnostics = failed["checks"][0]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["evidence"]["rule"], "clippy::useless_format");
    assert_eq!(diagnostics[0]["file"], "src/lib.rs");
    assert_eq!(
        failed["checks"][0]["metadata"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    policy["checks"][0]["reports"][0] =
        json!({"path":"target/clippy.sarif","format":"sarif","mode":"changed_lines"});
    configure(&policy);
    let changed = run(root.path(), 1);
    assert_eq!(
        changed["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        changed["checks"][0]["diagnostics"][0]["fingerprint"],
        diagnostics[0]["fingerprint"]
    );
    std::fs::write(
        root.path().join("src/lib.rs"),
        format!("\n{ORIGINAL}pub fn text() -> String {{ \"text\".to_owned() }}\n"),
    )
    .unwrap();
    run(root.path(), 0);
    std::fs::write(root.path().join("src/lib.rs"), "pub fn broken(\n").unwrap();
    let incomplete = run(root.path(), 2);
    assert_eq!(incomplete["checks"][0]["verdict"], Value::Null);
    assert_eq!(incomplete["checks"][0]["execution"]["exit_code"], 101);
    assert!(
        incomplete["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["path"].as_str().unwrap().ends_with("stderr.log"))
    );
}
