mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "java-thread-stop",
        "error",
        "thread.stop();\n",
        "thread.interrupt();\n",
    ),
    (
        "java-thread-yield",
        "warning",
        "Thread.yield();\n",
        "Thread.sleep(1);\n",
    ),
    ("java-manual-gc", "error", "System.gc();\n", "doWork();\n"),
    (
        "java-finalizer",
        "warning",
        "protected void finalize() {}\n",
        "protected void close() {}\n",
    ),
    (
        "java-implicit-charset",
        "warning",
        "bytes = text.getBytes();\n",
        "bytes = text.getBytes(StandardCharsets.UTF_8);\n",
    ),
    (
        "java-implicit-locale",
        "warning",
        "text.toLowerCase();\n",
        "text.toLowerCase(Locale.ROOT);\n",
    ),
    (
        "java-insecure-random",
        "warning",
        "Random r = new Random();\n",
        "SecureRandom r = new SecureRandom();\n",
    ),
    (
        "java-weak-crypto",
        "error",
        "MessageDigest.getInstance(\"MD5\");\n",
        "MessageDigest.getInstance(\"SHA-256\");\n",
    ),
    (
        "java-empty-catch",
        "warning",
        "catch (Exception e) {}\n",
        "catch (Exception e) { report(e); }\n",
    ),
    (
        "java-finally-exit",
        "error",
        "finally { return result; }\n",
        "finally { cleanup(); }\n",
    ),
    (
        "java-sql-concat",
        "warning",
        "stmt.executeQuery(\"SELECT \" + user);\n",
        "stmt.executeQuery(\"SELECT ?\");\n",
    ),
    (
        "java-runtime-exec",
        "warning",
        "Runtime.getRuntime().exec(command);\n",
        "safeProcess(command);\n",
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
fn java_rules_are_opt_in_and_reject_language_drift() {
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
                "java",
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
            serde_json::json!(["java"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "lang-java");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-java]\nrules: {java-thread-stop: {parameters: {languages: [python]}}}\n",
        "schema_version: 1\nrulesets: [lang-java]\nrules: {java-thread-stop: {parameters: {prohibited_patterns: {python: ['stop']}}}}\n",
        "schema_version: 1\nrulesets: [lang-java]\nrules: {java-thread-stop: {parameters: {prohibited_patterns: {java: ['[']}}}}\n",
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
fn java_source_signals_find_changed_java_lines_then_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-java]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        fs::write(root.join("src/Main.java"), bad).unwrap();
        fs::write(root.join("src/Other.kt"), bad).unwrap();
        let failed = check(root, if *severity == "error" { 1 } else { 0 });
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
        assert_eq!(
            rule(&failed, id)["diagnostics"][0]["evidence"]["language"],
            "java"
        );
        fs::write(root.join("src/Main.java"), good).unwrap();
        assert!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id}"
        );
    }
}

#[test]
fn java_ratchet_references_require_supported_formats_and_baselines() {
    for name in ["checkstyle", "pmd", "spotbugs"] {
        let repository = fixture();
        let root = repository.path();
        let source = fs::read_to_string(format!(
            "{}/skills/qualitygate-cli/references/{name}-ratchet.yaml",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        fs::write(root.join("qualitygate.yaml"), &source).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(0),
            "{name}"
        );
        let invalid = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("baseline:"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(root.join("qualitygate.yaml"), invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{name}"
        );
    }
}
