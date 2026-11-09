mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "py-none-comparison",
        "python",
        "lang-python",
        "src/sample.py",
        "if value == None:\n",
        "if value is None:\n",
    ),
    (
        "py-walrus-operator",
        "python",
        "lang-python",
        "src/sample.py",
        "if (count := len(items)) > 0:\n",
        "count = len(items)\n",
    ),
    (
        "py-lambda-assignment",
        "python",
        "lang-python",
        "src/sample.py",
        "handler = lambda x: x + 1\n",
        "def handler(x):\n    return x + 1\n",
    ),
    (
        "go-unsafe-package",
        "go",
        "lang-go",
        "src/sample.go",
        "pointer := unsafe.Pointer(&value)\n",
        "pointer := &value\n",
    ),
    (
        "go-os-exit",
        "go",
        "lang-go",
        "src/sample.go",
        "os.Exit(1)\n",
        "return\n",
    ),
    (
        "go-import-alias",
        "go",
        "lang-go",
        "src/sample.go",
        "import f \"fmt\"\n",
        "import \"fmt\"\n",
    ),
    (
        "ts-no-with",
        "typescript",
        "lang-typescript",
        "src/sample.ts",
        "with (obj) { run(); }\n",
        "run(obj);\n",
    ),
    (
        "ts-no-var",
        "typescript",
        "lang-typescript",
        "src/sample.ts",
        "var count = 0;\n",
        "let count = 0;\n",
    ),
    (
        "ts-no-arguments",
        "typescript",
        "lang-typescript",
        "src/sample.ts",
        "function sum() { return arguments.length; }\n",
        "function sum(...items) { return items.length; }\n",
    ),
    (
        "shell-eval",
        "shell",
        "shared",
        "src/sample.sh",
        "#!/bin/sh\neval \"$cmd\"\n",
        "#!/bin/sh\nrun \"$cmd\"\n",
    ),
    (
        "shell-alias",
        "shell",
        "shared",
        "src/sample.sh",
        "#!/bin/sh\nalias ll='ls -l'\n",
        "#!/bin/sh\nll() { ls -l \"$@\"; }\n",
    ),
    (
        "shell-chmod-read",
        "shell",
        "shared",
        "src/sample.sh",
        "#!/bin/sh\nchmod a-r secret.txt\n",
        "#!/bin/sh\nchmod u-r secret.txt\n",
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

fn config(package: &str, rule: &str) -> String {
    if package == "shared" {
        format!("schema_version: 1\nrules: {{{rule}: {{}}}}\n")
    } else {
        format!("schema_version: 1\nrulesets: [{package}]\nrules: {{{rule}: {{}}}}\n")
    }
}

#[test]
fn internal_language_rules_are_discoverable_with_fixed_scope() {
    let repository = fixture();
    let root = repository.path();
    for (id, language, package, _, _, _) in CASES {
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
        let entry = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap();
        assert_eq!(
            entry["definition"]["builtin"]["language"],
            serde_json::json!([language])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            "warning"
        );
        assert_eq!(entry["definition"]["package"], *package);
        assert_eq!(entry["enabled"], false);
    }
}

#[test]
fn internal_language_rules_match_only_their_language_and_clear_after_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir_all(root.join("src")).unwrap();
    for (id, language, package, path, bad, good) in CASES {
        fs::write(root.join("qualitygate.yaml"), config(package, id)).unwrap();
        fs::write(root.join(path), bad).unwrap();
        let failed = check(root, 0);
        let diagnostics = rule(&failed, id)["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1, "{id}");
        assert_eq!(diagnostics[0]["evidence"]["language"], *language);
        fs::write(root.join(path), good).unwrap();
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
fn internal_language_rules_reject_language_drift() {
    let repository = fixture();
    let root = repository.path();
    for (rule_id, language) in [
        ("py-none-comparison", "go"),
        ("go-os-exit", "python"),
        ("ts-no-var", "go"),
        ("shell-eval", "python"),
    ] {
        let package = match rule_id.split('-').next().unwrap() {
            "py" => "lang-python",
            "go" => "lang-go",
            "ts" => "lang-typescript",
            _ => "shared",
        };
        fs::write(
            root.join("qualitygate.yaml"),
            format!(
                "schema_version: 1\nrulesets: [{package}]\nrules: {{{rule_id}: {{parameters: {{languages: [{language}]}}}}}}\n"
            ),
        )
        .unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{rule_id}"
        );
    }
}
