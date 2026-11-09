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
        "if value == None:\n    pass\n",
        "if value is None:\n    pass\n",
    ),
    (
        "py-walrus-operator",
        "python",
        "lang-python",
        "src/sample.py",
        "if (count := len(items)) > 0:\n    pass\n",
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
        "package sample\nimport \"unsafe\"\nfunc f() { pointer := unsafe.Pointer(&value) }\n",
        "package sample\nfunc f() { pointer := &value }\n",
    ),
    (
        "go-os-exit",
        "go",
        "lang-go",
        "src/sample.go",
        "package sample\nimport \"os\"\nfunc f() { os.Exit(1) }\n",
        "package sample\nfunc f() { return }\n",
    ),
    (
        "go-import-alias",
        "go",
        "lang-go",
        "src/sample.go",
        "package sample\nimport f \"fmt\"\n",
        "package sample\nimport \"fmt\"\n",
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

#[test]
fn review_rules_ignore_documentation_properties_and_non_command_arguments() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir_all(root.join("src")).unwrap();
    for (id, language, package, path, _, _) in CASES {
        let source = match *language {
            "python" => "example = '''\nvalue == None\ncount := 1\nhandler = lambda x: x\n'''\n",
            "go" => {
                "package sample\nvar example = `\nunsafe.Pointer(x)\nos.Exit(1)\nimport f \"fmt\"\n`\n"
            }
            "typescript" => {
                "const example = `\nwith (obj) {}\nvar count = 1;\narguments.length\n`;\nconst obj = {arguments: 1};\nobj.arguments;\n"
            }
            "shell" => {
                "#!/bin/sh\necho 'eval \"$cmd\"'\necho '\nalias ll=ls\n'\nchmod 0644 ./a-r\n"
            }
            _ => unreachable!(),
        };
        fs::write(root.join("qualitygate.yaml"), config(package, id)).unwrap();
        fs::write(root.join(path), source).unwrap();
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
fn changed_go_imports_review_retained_calls_and_local_shadowing_removes_the_signal() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        config("lang-go", "go-os-exit"),
    )
    .unwrap();
    fs::write(
        root.join("sample.go"),
        "package sample\nimport stop \"example.com/os\"\nfunc f(){ stop.Exit(1) }\n",
    )
    .unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "baseline"]);
    fs::write(
        root.join("sample.go"),
        "package sample\nimport stop \"os\"\nfunc f(){ stop.Exit(1) }\n",
    )
    .unwrap();
    let result = check(root, 0);
    let diagnostics = rule(&result, "go-os-exit")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start_line"], 2);
    fs::write(
        root.join("sample.go"),
        "package sample\nimport stop \"os\"\nfunc f(stop Service){ stop.Exit(1) }\n",
    )
    .unwrap();
    assert!(
        rule(&check(root, 0), "go-os-exit")["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn syntax_rules_use_selected_source_and_policy_not_dirty_worktree_bytes() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        config("lang-python", "py-none-comparison"),
    )
    .unwrap();
    fs::write(root.join("sample.py"), "value is None\n").unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "baseline"]);
    fs::write(root.join("sample.py"), "value == None\n").unwrap();
    common::git(root, &["add", "sample.py"]);
    fs::write(root.join("sample.py"), "value is None\n").unwrap();
    assert!(
        rule(&check(root, 0), "py-none-comparison")["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: [lang-python]\nrules: {py-none-comparison: {parameters: {languages: [go]}}}\n").unwrap();
    let staged = report(
        &cli(
            root,
            &[
                "check",
                "--staged",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(
        rule(&staged, "py-none-comparison")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    check(root, 2);
}

#[test]
fn syntax_errors_and_regex_overrides_are_incomplete_but_excluded_sources_are_not_parsed() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        config("lang-python", "py-none-comparison"),
    )
    .unwrap();
    fs::write(root.join("sample.py"), "if (value == None:\n").unwrap();
    let result = check(root, 2);
    assert!(rule(&result, "py-none-comparison")["verdict"].is_null());
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: [lang-python]\nrules: {py-none-comparison: {parameters: {paths: ['src/**']}}}\n").unwrap();
    assert_eq!(
        rule(&check(root, 0), "py-none-comparison")["execution"]["status"],
        "completed"
    );
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: [lang-python]\nrules: {py-none-comparison: {parameters: {prohibited_patterns: {python: ['None']}}}}\n").unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
}
