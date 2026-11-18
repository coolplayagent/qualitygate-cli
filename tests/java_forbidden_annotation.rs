//! The paired Skill example with a real PMD 7 producer and JDK dependency JAR.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const POLICY: &str =
    include_str!("../skills/qualitygate-cli/references/java-forbidden-annotations.yaml");
const RULESET: &str =
    include_str!("../skills/qualitygate-cli/references/java-forbidden-annotations.xml");
const RULESET_PATH: &str = "config/java-forbidden-annotations.xml";
const JAR: &str = "deps/annotations.jar";
const IMPORTED: &str = "src/main/java/app/Imported.java";
const QUALIFIED: &str = "src/main/java/app/Qualified.java";
const ALLOWED: &str = "src/main/java/app/Allowed.java";

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn forbidden_annotations_reference_policy_is_valid() {
    let repository = fixture();
    let root = repository.path();
    write(root, "qualitygate.yaml", POLICY);
    let output = cli(root, &["config", "--show", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn tool(directory: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{program}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn dependency_jar(root: &Path) {
    let sources = tempfile::tempdir().unwrap();
    let sources = sources.path();
    let annotations = [
        ("legacy", "LegacyAudit"),
        ("legacy", "LegacyTrace"),
        ("current", "LegacyAudit"),
    ];
    let files: Vec<_> = annotations
        .iter()
        .map(|(package, name)| {
            let file = format!("{package}/{name}.java");
            write(
                sources,
                &file,
                &format!(
                    "package com.acme.{package};\n                    @java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.SOURCE)\n                    public @interface {name} {{}}\n"
                ),
            );
            file
        })
        .collect();
    let mut args = vec!["-d", "classes"];
    args.extend(files.iter().map(String::as_str));
    tool(sources, "javac", &args);
    tool(
        sources,
        "jar",
        &[
            "--create",
            "--file",
            "annotations.jar",
            "-C",
            "classes",
            ".",
        ],
    );
    fs::create_dir_all(root.join("deps")).unwrap();
    fs::copy(sources.join("annotations.jar"), root.join(JAR)).unwrap();
}

fn check(root: &Path, exit: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "full",
                "--format",
                "json",
            ],
        ),
        exit,
    )
}

fn annotation_result(value: &Value) -> &Value {
    value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "java-forbidden-annotations")
        .unwrap()
}

#[test]
#[ignore = "requires JDK 21 and PMD 7; mandatory live Java diagnostics CI job"]
fn real_pmd_resolves_source_retention_annotations_and_independent_tool_failure() {
    let repository = fixture();
    let root = repository.path();
    dependency_jar(root);
    let ruleset = RULESET.replace(
        "com.acme.legacy.LegacyAudit",
        "com.acme.legacy.LegacyAudit, com.acme.legacy.LegacyTrace",
    );
    write(root, RULESET_PATH, &ruleset);
    write(
        root,
        IMPORTED,
        "package app;\nimport com.acme.legacy.LegacyAudit;\n@LegacyAudit\nclass Imported {}\n",
    );
    write(
        root,
        QUALIFIED,
        "package app;\n@com.acme.legacy.LegacyTrace\nclass Qualified {}\n",
    );
    write(
        root,
        ALLOWED,
        "package app;\nimport com.acme.current.LegacyAudit;\n@LegacyAudit\n         class Allowed {\n         Class<?> literal = com.acme.legacy.LegacyAudit.class;\n         String text = \"@com.acme.legacy.LegacyTrace\";\n         }\n",
    );
    write(root, ".gitignore", "target/\n");
    write(
        root,
        "config/java-sources.args",
        &format!("-classpath {JAR}\n{IMPORTED}\n{QUALIFIED}\n{ALLOWED}\n"),
    );
    let mut policy: Value = serde_norway::from_str(POLICY).unwrap();
    let command = policy["checks"][1]["argv"].as_array_mut().unwrap();
    let classpath_index = command
        .iter()
        .position(|arg| arg == "--aux-classpath")
        .unwrap()
        + 1;
    command[classpath_index] = json!(JAR);
    write(
        root,
        "qualitygate.yaml",
        &serde_norway::to_string(&policy).unwrap(),
    );
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-qm", "Java sources and annotation dependency"],
    );
    write(root, "hello.txt", "Only a non-Java file was changed\n");

    let failure = check(root, 1);
    let result = annotation_result(&failure);
    assert_eq!(result["verdict"], "fail");
    let mut locations = result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic["evidence"]["rule"], "ForbiddenAnnotation");
            (
                diagnostic["file"].as_str().unwrap(),
                diagnostic["range"]["start_line"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    locations.sort_unstable();
    assert_eq!(locations, [(IMPORTED, 3), (QUALIFIED, 2)]);

    write(
        root,
        IMPORTED,
        "package app;\nimport com.acme.current.LegacyAudit;\n@LegacyAudit\nclass Imported {}\n",
    );
    write(
        root,
        QUALIFIED,
        "package app;\n@com.acme.current.LegacyAudit\nclass Qualified {}\n",
    );
    assert_eq!(annotation_result(&check(root, 0))["verdict"], "pass");

    write(root, RULESET_PATH, "<invalid");
    let incomplete = check(root, 2);
    let result = annotation_result(&incomplete);
    assert_eq!(incomplete["gate"]["decision"], "incomplete");
    assert!(result["verdict"].is_null());
}
