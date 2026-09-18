mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "c-array-safety",
        "error",
        "int values[count];\n",
        "int values[16];\n",
    ),
    (
        "c-assertion-discipline",
        "warning",
        "assert(index++ < 10);\n",
        "assert(index < 10);\n",
    ),
    (
        "c-control-flow",
        "warning",
        "for (;;) { break; }\n",
        "for (int i = 0; i < 10; ++i) { }\n",
    ),
    (
        "c-expression-safety",
        "warning",
        "size_t n = sizeof(index++);\n",
        "size_t n = sizeof(index);\n",
    ),
    (
        "c-file-security",
        "error",
        "char *name = tmpnam(0);\n",
        "char *name = safe_name();\n",
    ),
    (
        "c-function-safety",
        "warning",
        "abort();\n",
        "return error;\n",
    ),
    (
        "c-numeric-literal",
        "error",
        "long n = 123l;\n",
        "long n = 123L;\n",
    ),
];

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
fn c_rules_are_opt_in_discoverable_and_have_fixed_language_scope() {
    let repository = fixture();
    let root = repository.path();
    for language in ["c", "cpp"] {
        let list = report(
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
        for (id, severity, _, _) in CASES {
            let entry = list["rules"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["id"] == *id)
                .unwrap();
            assert_eq!(
                entry["definition"]["builtin"]["language"],
                serde_json::json!(["c", "cpp"])
            );
            assert_eq!(
                entry["definition"]["builtin"]["defaults"]["severity"],
                *severity
            );
            assert_eq!(entry["definition"]["package"], "lang-c");
            assert_eq!(entry["enabled"], false);
        }
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-c]\nrules: {c-array-safety: {parameters: {languages: [cpp]}}}\n",
        "schema_version: 1\nrulesets: [lang-c]\nrules: {c-array-safety: {parameters: {prohibited_patterns: {go: ['int']}}}}\n",
        "schema_version: 1\nrulesets: [lang-c]\nrules: {c-array-safety: {parameters: {prohibited_patterns: {c: ['[']}}}}\n",
    ] {
        fs::write(root.join("qualitygate.yaml"), invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
fn c_patterns_find_changed_lines_in_both_languages_and_clear_after_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-c]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        for (extension, language) in [("c", "c"), ("cpp", "cpp")] {
            let source = root.join(format!("src/sample.{extension}"));
            fs::write(&source, bad).unwrap();
            fs::write(root.join("src/other.py"), bad).unwrap();
            let found = check(root, if *severity == "error" { 1 } else { 0 });
            assert_eq!(
                rule(&found, id)["diagnostics"].as_array().unwrap().len(),
                if extension == "c" { 1 } else { 2 },
                "{id} {language}"
            );
            assert!(
                rule(&found, id)["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|diagnostic| diagnostic["evidence"]["language"] == language)
            );
        }
        for extension in ["c", "cpp"] {
            fs::write(root.join(format!("src/sample.{extension}")), good).unwrap();
        }
        assert!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id}"
        );
    }
}
