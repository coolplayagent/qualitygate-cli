mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

fn check(root: &Path, expected: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        expected,
    )
}

fn rule<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn six_opt_in_rules_are_discoverable_and_reject_invalid_scope() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &["rules", "list", "--source", "builtin", "--format", "json"],
        ),
        0,
    );
    for (id, languages, severity) in [
        ("no-bare-except", vec!["python"], "error"),
        ("no-os-path", vec!["python"], "error"),
        ("no-print", vec!["python"], "warning"),
        ("no-emoji", vec![], "error"),
        ("commit-message-format", vec![], "error"),
        ("python-test-naming", vec!["python"], "error"),
    ] {
        let entry = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap();
        assert_eq!(
            entry["definition"]["builtin"]["language"],
            serde_json::json!(languages)
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            severity
        );
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrules: {no-bare-except: {parameters: {languages: [java]}}}\n",
        "schema_version: 1\nrules: {no-os-path: {parameters: {languages: []}}}\n",
        "schema_version: 1\nrules: {no-print: {parameters: {pattern: '['}}}\n",
        "schema_version: 1\nrules: {no-emoji: {parameters: {languages: [python]}}}\n",
        "schema_version: 1\nrules: {python-test-naming: {parameters: {languages: [java]}}}\n",
        "schema_version: 1\nrules: {commit-message-format: {parameters: {pattern: '['}}}\n",
    ] {
        fs::write(root.join("qualitygate.yaml"), invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{invalid}"
        );
    }
}

#[test]
fn python_and_neutral_file_rules_have_independent_scopes_and_warning_gate() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-bare-except: {}, no-os-path: {}, no-print: {}, no-emoji: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.py"),
        "try:\n    work()\nexcept:\n    print(os.path.join('a', 'b'))\n",
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "// Unicode marker 🧪\n").unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["gate"]["complete"], true);
    for id in ["no-bare-except", "no-os-path", "no-print", "no-emoji"] {
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1
        );
    }
    assert_eq!(rule(&failed, "no-print")["severity"], "warning");

    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-print: {}}\n",
    )
    .unwrap();
    let warning = check(root, 0);
    assert_eq!(rule(&warning, "no-print")["verdict"], "fail");
    assert_eq!(warning["gate"]["decision"], "pass");
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-print: {parameters: {paths: ['tests/**/*.py']}}}\n",
    )
    .unwrap();
    let scoped = check(root, 0);
    assert_eq!(rule(&scoped, "no-print")["matched_entities"], 0);
}

#[test]
fn python_naming_detects_all_pytest_prefixes_and_commit_format_is_configurable() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {python-test-naming: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/test_api.py"),
        "def test_valid_case():\n    assert True\n\ndef test1():\n    assert True\n\ndef testAgent():\n    assert True\n",
    )
    .unwrap();
    fs::write(root.join("tests/test_api.rs"), "fn testBad() {}\n").unwrap();
    let failed = check(root, 1);
    let naming = rule(&failed, "python-test-naming");
    assert_eq!(naming["matched_entities"], 3);
    assert_eq!(naming["diagnostics"].as_array().unwrap().len(), 2);
    fs::write(
        root.join("tests/test_api.py"),
        "def test_valid_case():\n    assert True\n",
    )
    .unwrap();
    assert_eq!(check(root, 0)["gate"]["decision"], "pass");

    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "feat: add tests"]);
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {commit-message-format: {}}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/test_api.py"),
        "def test_next_case():\n    assert True\n",
    )
    .unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "feature: reject me"]);
    let failed = report(
        &cli(
            root,
            &[
                "check",
                "--diff",
                "HEAD~1..HEAD",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        1,
    );
    assert_eq!(
        rule(&failed, "commit-message-format")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {commit-message-format: {parameters: {pattern: '^feature: .+'}}}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/test_api.py"),
        "def test_final_case():\n    assert True\n",
    )
    .unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "feature: allowed by policy"]);
    let accepted = report(
        &cli(
            root,
            &[
                "check",
                "--diff",
                "HEAD~1..HEAD",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(rule(&accepted, "commit-message-format")["verdict"], "pass");
}
