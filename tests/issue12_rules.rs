mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const RULES: &[(&str, &str)] = &[
    ("shell-hardcoded-secret", "error"),
    ("shell-debug-mode", "warning"),
    ("shell-env-dump", "warning"),
    ("shell-weak-crypto", "warning"),
    ("shell-password-echo", "warning"),
    ("shell-sql-injection", "error"),
    ("shell-exec-terminates", "warning"),
    ("shell-assignment-spaces", "error"),
    ("shell-comparison-spaces", "error"),
    ("shell-line-start-operator", "warning"),
    ("shell-stream-merge-position", "warning"),
    ("shell-trap-uncapturable", "error"),
    ("shell-trap-numeric-signal", "warning"),
    ("shell-trap-double-quotes", "warning"),
    ("shell-tilde-path", "warning"),
    ("shell-missing-shebang", "error"),
    ("shell-temp-file-hardcoded", "warning"),
    ("shell-commented-dead-code", "warning"),
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
fn all_shell_rules_are_opt_in_discoverable_and_fixed_to_shell_scope() {
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
                "shell",
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
            serde_json::json!(["shell"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "shared");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrules: {shell-hardcoded-secret: {parameters: {languages: [python]}}}\n",
        "schema_version: 1\nrules: {shell-debug-mode: {parameters: {prohibited_patterns: {python: ['set -x']}}}}\n",
        "schema_version: 1\nrules: {shell-sql-injection: {parameters: {prohibited_patterns: {shell: ['[']}}}}\n",
        "schema_version: 1\nrules: {shell-missing-shebang: {parameters: {languages: []}}}\n",
        "schema_version: 1\nrules: {shell-commented-dead-code: {parameters: {languages: [python]}}}\n",
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
fn source_patterns_cover_shell_extensions_and_shebang_scripts_only() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {shell-hardcoded-secret: {}, shell-assignment-spaces: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/run.sh"),
        "#!/bin/sh\npassword='secret'\nVALUE = wrong\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/build.bash"),
        "#!/bin/bash\ntoken='secret'\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/deploy"),
        "#!/usr/bin/env -S bash -e\napi_key='secret'\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/other.py"),
        "password='secret'\nVALUE = wrong\n",
    )
    .unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["gate"]["complete"], true);
    assert_eq!(
        rule(&failed, "shell-hardcoded-secret")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        rule(&failed, "shell-assignment-spaces")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    fs::write(
        root.join("scripts/run.sh"),
        "#!/bin/sh\npassword=\"$SECRET\"\nVALUE=right\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/build.bash"),
        "#!/bin/bash\ntoken=\"$TOKEN\"\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/deploy"),
        "#!/usr/bin/env -S bash -e\napi_key=\"$KEY\"\n",
    )
    .unwrap();
    assert_eq!(check(root, 0)["gate"]["decision"], "pass");
}

#[test]
fn first_line_and_three_line_comment_rules_keep_bad_text_incomplete() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {shell-missing-shebang: {}, shell-commented-dead-code: {}}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/run.sh"),
        "# if old\n# for stale\n# exit 1\n",
    )
    .unwrap();
    let failed = check(root, 1);
    for id in ["shell-missing-shebang", "shell-commented-dead-code"] {
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1
        );
    }
    fs::write(
        root.join("scripts/run.sh"),
        "#!/bin/sh\n# Describe the process.\n# Explain the output.\n",
    )
    .unwrap();
    assert_eq!(check(root, 0)["gate"]["decision"], "pass");
    fs::write(root.join("scripts/run.sh"), [0xff, 0xfe]).unwrap();
    let incomplete = check(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    for id in ["shell-missing-shebang", "shell-commented-dead-code"] {
        assert_ne!(rule(&incomplete, id)["execution"]["status"], "completed");
    }
}

#[test]
fn source_pattern_diagnostic_budget_blocks_excessive_matches() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {shell-debug-mode: {}}\n",
    )
    .unwrap();
    fs::write(
        root.join("large.sh"),
        format!("#!/bin/sh\n{}", "set -x\n".repeat(10_001)),
    )
    .unwrap();
    let incomplete = check(root, 2);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert!(
        rule(&incomplete, "shell-debug-mode")["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("diagnostics exceed 10000")
    );
}
