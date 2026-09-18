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

fn verdict<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn three_rules_are_discoverable_and_language_scope_is_fixed() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &["rules", "list", "--source", "builtin", "--format", "json"],
        ),
        0,
    );
    for (id, languages) in [
        ("no-printf-log", vec!["c", "cpp"]),
        ("no-unsafe-string", vec!["c", "cpp"]),
        ("no-test-sleep", vec![]),
    ] {
        let row = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(row["definition"]["package"], "shared");
        assert_eq!(
            row["definition"]["builtin"]["language"],
            serde_json::json!(languages)
        );
        assert_eq!(
            row["definition"]["builtin"]["defaults"]["severity"],
            "error"
        );
        assert_eq!(row["enabled"], false);
    }
    for language in ["c", "cpp"] {
        let filtered = report(
            &cli(
                root,
                &[
                    "rules",
                    "list",
                    "--source",
                    "builtin",
                    "--language",
                    language,
                    "--format",
                    "json",
                ],
            ),
            0,
        );
        assert!(
            filtered["rules"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| { row["id"] == "no-printf-log" })
        );
        assert!(
            filtered["rules"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| { row["id"] == "no-unsafe-string" })
        );
    }
    for invalid in [
        "schema_version: 1\nrules: {no-printf-log: {parameters: {languages: [java]}}}\n",
        "schema_version: 1\nrules: {no-unsafe-string: {parameters: {pattern: '['}}}\n",
        "schema_version: 1\nrules: {no-test-sleep: {parameters: {languages: [c]}}}\n",
        "schema_version: 1\nrules: {no-test-sleep: {parameters: {paths: ['[']}}}\n",
        "schema_version: 1\nrules: {no-test-sleep: {parameters: {paths: []}}}\n",
        "schema_version: 1\nrules: {no-test-sleep: {parameters: {paths: ['']}}}\n",
        "schema_version: 1\nrules: {no-hardcoded-secrets: {parameters: {languages: []}}}\n",
        "schema_version: 1\nrules: {security-sensitive-api: {parameters: {languages: [c]}}}\n",
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
fn native_rules_find_added_c_and_cpp_calls_and_ignore_modified_or_other_languages() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-printf-log: {}, no-unsafe-string: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.c"),
        "void f(){ printf(\"x\"); strcpy(a,b); }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/other.cpp"),
        "void g(){ fprintf(stderr, \"x\"); gets(x); }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Header.h"),
        "void h(){ sprintf(x, \"x\"); }\n",
    )
    .unwrap();
    fs::write(root.join("src/Main.java"), "printf(x); strcpy(a,b);\n").unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["gate"]["complete"], true);
    assert_eq!(
        verdict(&failed, "no-printf-log")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        verdict(&failed, "no-unsafe-string")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(verdict(&failed, "no-printf-log")["matched_entities"], 3);
    fs::write(
        root.join("src/main.c"),
        "void f(){ safe_log(); copy_bounded(); }\n",
    )
    .unwrap();
    fs::write(root.join("src/other.cpp"), "void g(){ safe_log(); }\n").unwrap();
    fs::write(root.join("src/Header.h"), "void h(){ copy_bounded(); }\n").unwrap();
    assert_eq!(check(root, 0)["gate"]["decision"], "pass");
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "native base"]);
    fs::write(
        root.join("src/main.c"),
        "void f(){ printf(\"modified\"); }\n",
    )
    .unwrap();
    assert_eq!(
        verdict(&check(root, 0), "no-printf-log")["matched_entities"],
        0
    );
}

#[test]
fn test_sleep_uses_default_and_configured_test_paths_across_languages() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-test-sleep: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("tests/test_clock.py"), "sleep(1)\n").unwrap();
    fs::write(
        root.join("src/clock_test.go"),
        "func TestClock(){ time.Sleep(1) }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/ServiceTest.java"),
        "class ServiceTest { void check(){ nanosleep(1); } }\n",
    )
    .unwrap();
    fs::write(root.join("src/runtime.rs"), "sleep(1);\n").unwrap();
    let failed = check(root, 1);
    let sleep = verdict(&failed, "no-test-sleep");
    assert_eq!(sleep["matched_entities"], 3, "{sleep}");
    assert_eq!(sleep["diagnostics"].as_array().unwrap().len(), 3);
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-test-sleep: {parameters: {paths: ['tests/**/*.py']}}}\n",
    )
    .unwrap();
    let scoped = check(root, 1);
    assert_eq!(verdict(&scoped, "no-test-sleep")["matched_entities"], 1);
    fs::write(root.join("tests/test_clock.py"), "clock.advance(1)\n").unwrap();
    assert_eq!(check(root, 0)["gate"]["decision"], "pass");
}
