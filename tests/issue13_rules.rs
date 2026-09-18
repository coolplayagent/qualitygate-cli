mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const RULES: &[(&str, &str)] = &[
    ("rust-extern-without-abi", "warning"),
    ("rust-untrusted-dynamic-library-loading", "error"),
    ("rust-unsafe-block-in-macro-definition", "warning"),
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
fn rust_rules_are_opt_in_discoverable_and_confined_to_rust() {
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
                "rust",
                "--format",
                "json",
            ],
        ),
        0,
    );
    for (id, severity) in RULES {
        let entry = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap();
        assert_eq!(
            entry["definition"]["builtin"]["language"],
            serde_json::json!(["rust"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrules: {rust-extern-without-abi: {parameters: {languages: [python]}}}\n",
        "schema_version: 1\nrules: {rust-untrusted-dynamic-library-loading: {parameters: {prohibited_patterns: {python: ['dlopen']}}}}\n",
        "schema_version: 1\nrules: {rust-unsafe-block-in-macro-definition: {parameters: {prohibited_patterns: {rust: ['[']}}}}\n",
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
fn rust_source_patterns_report_added_lines_and_repair_with_explicit_limits() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrules: {rust-extern-without-abi: {}, rust-untrusted-dynamic-library-loading: {}, rust-unsafe-block-in-macro-definition: {}}\n").unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "unsafe extern { fn legacy(); }\nunsafe { libloading::Library::new(candidate) };\nmacro_rules! hidden { () => { unsafe { ptr::read(ptr) } } }\n").unwrap();
    fs::write(
        root.join("src/other.py"),
        "extern {\ndlopen(path)\nmacro_rules! x { unsafe { } }\n",
    )
    .unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["gate"]["complete"], true);
    for (id, _) in RULES {
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
    }
    fs::write(root.join("src/lib.rs"), "unsafe extern \"C\" { fn legacy(); }\nunsafe { libloading::Library::new(\"trusted.so\") };\nmacro_rules! hidden { () => {\n    unsafe { ptr::read(ptr) }\n} }\n").unwrap();
    let repaired = check(root, 0);
    assert_eq!(repaired["gate"]["decision"], "pass");
    for (id, _) in RULES {
        assert_eq!(
            rule(&repaired, id)["diagnostics"].as_array().unwrap().len(),
            0,
            "{id}"
        );
    }
}
