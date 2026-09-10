mod common;
use common::*;
use std::path::Path;

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

#[test]
fn init_in_linked_git_worktree_excludes_metadata_file() {
    let root = fixture();
    let checkout = tempfile::tempdir().unwrap();
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "--detach",
            checkout.path().to_str().unwrap(),
            "HEAD",
        ],
    );
    assert!(checkout.path().join(".git").is_file());
    let metadata = std::fs::read(checkout.path().join(".git")).unwrap();
    write(
        checkout.path(),
        "Cargo.toml",
        "[package]\nname='worktree-fixture'\nversion='0.1.0'\nedition='2024'\n",
    );
    write(checkout.path(), "src/lib.rs", "pub fn example() {}\n");
    let initialized = report(&cli(checkout.path(), &["init", "--format", "json"]), 0);
    assert_eq!(initialized["created"], true);
    assert_eq!(
        initialized["config"]["languages"],
        serde_json::json!(["rust"])
    );
    assert_eq!(initialized["discovery"]["files_seen"], 3);
    assert!(checkout.path().join("qualitygate.yaml").is_file());
    assert!(!root.path().join("qualitygate.yaml").exists());
    assert_eq!(
        std::fs::read(checkout.path().join(".git")).unwrap(),
        metadata
    );
}

#[test]
fn init_describes_nested_projects_without_running_or_imposing_commands() {
    let root = fixture();
    write(
        root.path(),
        "web/package.json",
        r#"{"scripts":{"build":"echo executed > should-not-exist","test":"exit 1"}}"#,
    );
    write(root.path(), "web/src/api.ts", "export const api = 1;\n");
    write(
        root.path(),
        "python/pyproject.toml",
        "[project]\nname='example'\nversion='1.0'\n[tool.pytest.ini_options]\ntestpaths=['tests']\n",
    );
    write(root.path(), "scripts/check.sh", "exit 0\n");
    let initialized = report(
        &cli(
            root.path(),
            &[
                "init",
                "--config",
                "policy/qualitygate.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(initialized["created"], true);
    assert_eq!(initialized["with_checks_applied"], false);
    assert_eq!(
        initialized["config"]["languages"],
        serde_json::json!(["python", "shell", "typescript"])
    );
    assert!(
        initialized["config"]["checks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(initialized["discovery"]["commands_executed"], false);
    assert!(!root.path().join("web/should-not-exist").exists());
    let original = std::fs::read(root.path().join("policy/qualitygate.yaml")).unwrap();
    for format in ["json", "table", "markdown"] {
        let output = cli(
            root.path(),
            &[
                "init",
                "--with-checks",
                "--config",
                "policy/qualitygate.yaml",
                "--format",
                format,
            ],
        );
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("suggested_checks"));
        assert_eq!(
            std::fs::read(root.path().join("policy/qualitygate.yaml")).unwrap(),
            original
        );
    }
    assert!(!root.path().join("qualitygate.yaml").exists());
}

#[test]
fn init_with_checks_excludes_unmapped_test_commands_and_preserves_all_suggestions() {
    let root = fixture();
    write(root.path(), "java/pom.xml", "<project/>");
    write(
        root.path(),
        "web/package.json",
        r#"{"scripts":{"build":"echo build","test":"some-tests"}}"#,
    );
    write(root.path(), "go/go.mod", "module example.test/app\n");
    let initialized = report(
        &cli(root.path(), &["init", "--with-checks", "--format", "json"]),
        0,
    );
    let checks = initialized["config"]["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 3);
    assert!(checks.iter().all(|check| {
        !check["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "verify" || arg == "test")
    }));
    assert_eq!(
        initialized["discovery"]["suggested_checks"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(initialized["with_checks_applied"], true);
    assert!(
        initialized["config"]["verification_assets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "web/package.json")
    );
    // Invalid existing policy is preserved, even when new checks were requested.
    write(
        root.path(),
        "qualitygate.yaml",
        "schema_version: 1\nmisspelled: true\n",
    );
    report(
        &cli(root.path(), &["init", "--with-checks", "--format", "json"]),
        2,
    );
    assert!(
        std::fs::read_to_string(root.path().join("qualitygate.yaml"))
            .unwrap()
            .contains("misspelled")
    );
}

#[test]
fn generated_rust_candidate_runs_real_tests_then_repairs_and_rejects_zero_tests() {
    let root = fixture();
    write(
        root.path(),
        "Cargo.toml",
        "[package]\nname='init-fixture'\nversion='0.1.0'\nedition='2024'\n",
    );
    write(
        root.path(),
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"init-fixture\"\nversion = \"0.1.0\"\n",
    );
    let failing = "pub fn add(a: i32, b: i32) -> i32 { a - b }\n#[cfg(test)] mod tests { #[test] fn adds() { assert_eq!(super::add(2, 3), 5); } }\n";
    write(root.path(), "src/lib.rs", failing);
    let initialized = report(
        &cli(root.path(), &["init", "--with-checks", "--format", "json"]),
        0,
    );
    assert_eq!(initialized["config"]["checks"].as_array().unwrap().len(), 2);
    assert!(!root.path().join("target").exists());
    let failed = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    let tests = failed["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"].as_str().unwrap().contains("cargo-test"))
        .unwrap();
    assert_eq!(tests["execution"]["status"], "completed");
    assert_eq!(tests["verdict"], "fail");
    write(
        root.path(),
        "src/lib.rs",
        &failing.replace("a - b", "a + b"),
    );
    let passed = report(&cli(root.path(), &["check", "--format", "json"]), 0);
    assert_eq!(passed["scope"], "repository");
    write(
        root.path(),
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    );
    let empty = report(&cli(root.path(), &["check", "--format", "json"]), 1);
    let tests = empty["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"].as_str().unwrap().contains("cargo-test"))
        .unwrap();
    assert_eq!(tests["metadata"]["tests"]["executed"], 0);
    assert_eq!(tests["verdict"], "fail");
    assert!(
        tests["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["message"] == "No tests executed")
    );
}
