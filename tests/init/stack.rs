//! Poll the real command dispatcher under a portable stack allowance. A child
//! process contains the old-code stack overflow instead of aborting this suite.
use super::{cli, fixture, report, write};
use clap::Parser;
use qualitygate::interfaces::cli::Cli;
use serde_json::Value;
use std::{ffi::OsString, path::Path, time::Duration};

const CHILD_ROOT: &str = "QUALITYGATE_TEST_BOUNDED_DISPATCH_ROOT";
const TEST_NAME: &str = "stack::rust_check_dispatch_preserves_outcomes_on_a_bounded_stack";
const FAILING: &str = "pub fn add(a:i32,b:i32)->i32 {a-b}\n#[cfg(test)]mod tests {#[test]fn adds(){assert_eq!(super::add(2,3),5);}}\n";

#[test]
fn rust_check_dispatch_preserves_outcomes_on_a_bounded_stack() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        std::thread::Builder::new()
            .name("bounded-check-dispatch".into())
            .stack_size(768 * 1024)
            .spawn(move || exercise(Path::new(&root)))
            .unwrap()
            .join()
            .unwrap();
        return;
    }
    let root = fixture();
    write(
        root.path(),
        "Cargo.toml",
        "[package]\nname='stack-fixture'\nversion='0.1.0'\nedition='2024'\n",
    );
    write(
        root.path(),
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"stack-fixture\"\nversion = \"0.1.0\"\n",
    );
    write(root.path(), "src/lib.rs", FAILING);
    report(
        &cli(root.path(), &["init", "--with-checks", "--format", "json"]),
        0,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(180),
            tokio::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env(CHILD_ROOT, root.path())
                .env_remove("CARGO_TARGET_DIR")
                .env("QUALITYGATE_HOME", root.path().join(".git/stack-evidence"))
                .env(
                    "QUALITYGATE_BUILTIN_RULES_DIR",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/skills/qualitygate-cli/references/rules"
                    ),
                )
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("bounded dispatcher child exceeded 180 seconds")
        .unwrap()
    });
    assert!(
        output.status.success(),
        "bounded dispatcher child failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn exercise(root: &Path) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for (source, expected, count) in [
        (FAILING.to_owned(), 1, 1),
        (FAILING.replace("a-b", "a+b"), 0, 1),
        ("pub fn add(a:i32,b:i32)->i32 {a+b}\n".to_owned(), 1, 0),
    ] {
        write(root, "src/lib.rs", &source);
        let cli = Cli::try_parse_from([
            OsString::from("qualitygate"),
            OsString::from("--root"),
            root.as_os_str().to_owned(),
            OsString::from("check"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();
        let (output, code) = runtime.block_on(cli.run()).unwrap();
        assert_eq!(code, expected, "{output}");
        let value: Value = serde_json::from_str(&output).unwrap();
        let tests = value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"].as_str().unwrap().contains("cargo-test"))
            .unwrap();
        assert_eq!(tests["execution"]["status"], "completed");
        assert_eq!(tests["metadata"]["tests"]["executed"], count);
        assert_eq!(
            tests["verdict"],
            if expected == 0 { "pass" } else { "fail" }
        );
        if count == 0 {
            assert!(
                tests["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|diagnostic| { diagnostic["message"] == "No tests executed" })
            );
        }
    }
}
