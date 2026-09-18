mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    ("ts-no-eval", "error", "eval(input);\n", "parse(input);\n"),
    (
        "ts-no-debugger",
        "error",
        "debugger;\n",
        "const debuggerEnabled = false;\n",
    ),
    ("ts-no-alert", "warning", "alert('hi');\n", "showAlert();\n"),
    (
        "ts-no-implied-eval",
        "error",
        "setTimeout('tick()', 5);\n",
        "setTimeout(() => tick(), 5);\n",
    ),
    (
        "ts-no-new-function",
        "error",
        "new Function(source);\n",
        "function build() {}\n",
    ),
    (
        "ts-eqeqeq",
        "warning",
        "const equal = left == right;\n",
        "const equal = left === right;\n",
    ),
    (
        "ts-no-extend-native",
        "error",
        "Array.prototype.extra = 1;\n",
        "const extra = Array.prototype.extra;\n",
    ),
    (
        "ts-no-prototype-builtins",
        "warning",
        "obj.hasOwnProperty(key);\n",
        "Object.prototype.hasOwnProperty.call(obj, key);\n",
    ),
    (
        "ts-secure-randomness",
        "warning",
        "Math.random();\n",
        "crypto.getRandomValues(bytes);\n",
    ),
    (
        "ts-no-unsafe-postmessage",
        "warning",
        "window.postMessage(payload, '*');\n",
        "sendMessage(payload);\n",
    ),
    (
        "ts-no-commented-code",
        "warning",
        "// const old = 1;\n",
        "// Explain the behavior.\n",
    ),
    (
        "ts-no-personal-info-in-comments",
        "warning",
        "// email: alice@example.com\n",
        "// Explain the behavior.\n",
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
fn typescript_rules_are_discoverable_opt_in_and_reject_language_drift() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &[
                "rules",
                "list",
                "--source",
                "builtin",
                "--language",
                "typescript",
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
            serde_json::json!(["typescript"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "lang-typescript");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-typescript]\nrules: {ts-no-eval: {parameters: {languages: [rust]}}}\n",
        "schema_version: 1\nrulesets: [lang-typescript]\nrules: {ts-no-alert: {parameters: {prohibited_patterns: {rust: ['alert']}}}}\n",
        "schema_version: 1\nrulesets: [lang-typescript]\nrules: {ts-eqeqeq: {parameters: {prohibited_patterns: {typescript: ['[']}}}}\n",
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
fn typescript_patterns_find_changed_js_and_ts_lines_then_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-typescript]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        fs::write(root.join("src/sample.ts"), bad).unwrap();
        fs::write(root.join("src/other.py"), bad).unwrap();
        let failed = check(root, if *severity == "error" { 1 } else { 0 });
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
        assert_eq!(
            rule(&failed, id)["diagnostics"][0]["evidence"]["language"],
            "typescript"
        );
        fs::write(root.join("src/sample.ts"), good).unwrap();
        assert_eq!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            0,
            "{id}"
        );
    }
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-typescript]\nrules: {ts-no-eval: {}}\n",
    )
    .unwrap();
    fs::write(root.join("src/sample.js"), "eval(input);\n").unwrap();
    assert_eq!(
        rule(&check(root, 1), "ts-no-eval")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
