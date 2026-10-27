use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};

pub fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    // These fixtures assert exact staged/worktree bytes on every platform.
    git(root.path(), &["config", "core.autocrlf", "false"]);
    git(root.path(), &["config", "core.safecrlf", "false"]);
    git(root.path(), &["config", "user.name", "Fixture"]);
    git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(root.path().join("hello.txt"), "initial\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "initial"]);
    root
}

#[allow(dead_code)] // Core-only integration targets share the fixture/report helpers.
pub fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        // Fixture Cargo projects must not share the parent test runner's artifacts.
        .env_remove("CARGO_TARGET_DIR")
        .env(
            "QUALITYGATE_HOME",
            root.join(".git/qualitygate-test-evidence"),
        )
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/skills/qualitygate-cli/references/rules"
            ),
        )
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

pub fn report(output: &Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[allow(dead_code)] // Only producer-failure integration targets inspect raw stderr.
pub fn assert_artifact_contains(execution: &Value, suffix: &str, expected: &str) {
    let artifact = execution["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| {
            Path::new(artifact["path"].as_str().unwrap())
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(suffix))
        })
        .unwrap();
    let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
    assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(expected), "{suffix}: {text}");
}
