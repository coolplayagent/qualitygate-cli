mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::fs;

#[test]
fn four_conventions_are_discoverable_and_fail_closed_on_invalid_parameters() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &["rules", "list", "--source", "builtin", "--format", "json"],
        ),
        0,
    );
    for (id, package, language) in [
        ("commit-message-convention", "core", None),
        ("test-naming-strict", "shared", Some("java")),
        ("test-annotation-dependency", "shared", Some("java")),
        ("no-hardcoded-secrets", "shared", Some("java")),
    ] {
        let rule = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(rule["definition"]["package"], package);
        assert_eq!(rule["source"], "builtin");
        assert_eq!(
            rule["definition"]["builtin"]["defaults"]["severity"],
            "error"
        );
        assert_eq!(rule["enabled"], false);
        if let Some(language) = language {
            assert!(
                rule["definition"]["builtin"]["language"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value == language)
            );
        }
    }
    let base = "schema_version: 1\nrulesets: [core]\nrules:\n  test-annotation-dependency:\n    parameters:\n      annotation: Trace\n";
    fs::write(root.join("qualitygate.yaml"), base).unwrap();
    report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    for invalid in [
        base.replace("Trace", ""),
        base.replace("Trace", "Trace.Name"),
        base.replace("annotation: Trace", "annotation: Trace\n      group: ''"),
        base.replace(
            "annotation: Trace",
            "annotation: Trace\n      languages: [python]",
        ),
    ] {
        fs::write(root.join("qualitygate.yaml"), &invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{invalid}"
        );
    }
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {no-hardcoded-secrets: {parameters: {pattern: '['}}}\n",
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn commit_test_name_and_secret_rules_report_violations_then_repairs() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: [core]\nrules:\n  commit-message-convention: {}\n  test-naming-strict: {}\n  no-hardcoded-secrets: {}\n").unwrap();
    fs::create_dir_all(root.join("src/test/java")).unwrap();
    fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test void badName() {} String password = \"private-value\"; }\n",
    )
    .unwrap();
    let broken = report(
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
        1,
    );
    assert_eq!(broken["gate"]["complete"], true);
    let checks: Vec<&Value> = broken["checks"].as_array().unwrap().iter().collect();
    for id in ["test-naming-strict", "no-hardcoded-secrets"] {
        let check = checks.iter().find(|check| check["id"] == id).unwrap();
        assert_eq!(check["verdict"], "fail");
        assert_eq!(check["diagnostics"].as_array().unwrap().len(), 1);
    }
    assert!(!broken.to_string().contains("private-value"));
    fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test void should_work_when_ready() {} }\n",
    )
    .unwrap();
    let repaired = report(
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
        0,
    );
    assert_eq!(repaired["gate"]["decision"], "pass");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "bad commit subject"]);
    let history = report(
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
    let commit = history["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "commit-message-convention")
        .unwrap();
    assert_eq!(commit["verdict"], "fail");
    assert_eq!(commit["diagnostics"].as_array().unwrap().len(), 1);
    git(
        root,
        &["commit", "--allow-empty", "-qm", "[BUG9]feat: repaired"],
    );
    let valid = report(
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
    let accepted = valid["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "commit-message-convention")
        .unwrap();
    assert_eq!(accepted["verdict"], "pass");
}

#[test]
fn dependency_rule_without_bound_maven_producer_is_incomplete() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  test-annotation-dependency: {}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src/test/java")).unwrap();
    fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test void should_work_when_ready() {} }\n",
    )
    .unwrap();
    let output = report(
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
        2,
    );
    assert_eq!(output["gate"]["complete"], false);
    let check = &output["checks"][0];
    assert_eq!(check["id"], "test-annotation-dependency");
    assert!(
        check["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("dependency_resolution")
    );
    fs::write(
        root.join("src/test/java/T.java"),
        "class T { void ordinary() {} }\n",
    )
    .unwrap();
    let empty = report(
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
        0,
    );
    assert_eq!(empty["checks"][0]["verdict"], "pass");
    assert_eq!(empty["checks"][0]["matched_entities"], 0);
}

#[cfg(unix)]
#[test]
fn annotation_dependency_uses_fresh_resolved_maven_project_facts() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    fs::write(
        root.join("pom.xml"),
        "<project><artifactId>sample</artifactId></project>\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src/test/java")).unwrap();
    fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test void should_work_when_ready() {} }\n",
    )
    .unwrap();
    let generator = r#"mkdir -p target; sed "s|WORKSPACE_PLACEHOLDER|$PWD|g" model-template.xml > target/effective.xml; cp tree-template.json target/tree.json"#;
    let model = |dependency: &str| {
        format!(
            "<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>sample</artifactId><version>1</version><build><sourceDirectory>WORKSPACE_PLACEHOLDER/src/main/java</sourceDirectory><testSourceDirectory>WORKSPACE_PLACEHOLDER/src/test/java</testSourceDirectory></build>{dependency}</project>"
        )
    };
    fs::write(
        root.join("model-template.xml"),
        model("<dependencies><dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter-api</artifactId><version>5.0</version><scope>test</scope></dependency></dependencies>"),
    )
    .unwrap();
    let config = json!({
        "schema_version": 1,
        "rules": {"test-annotation-dependency": {"depends_on": ["facts"]}},
        "checks": [{
            "id": "facts",
            "argv": ["sh", "-c", generator],
            "tools": [{"id": "git", "argv": ["git", "--version"]}],
            "projects": [{"root": ".", "effective_pom": "target/effective.xml", "dependency_tree": "target/tree.json"}]
        }],
        "profiles": {"quick": {"include": ["test-annotation-dependency"]}}
    });
    fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    let tree = json!({
        "groupId": "fixture", "artifactId": "sample", "version": "1", "type": "jar",
        "classifier": "", "optional": "false", "scope": "",
        "children": [{
            "groupId": "org.junit.jupiter", "artifactId": "junit-jupiter-api", "version": "5.0",
            "type": "jar", "classifier": "", "optional": "false", "scope": "test"
        }]
    });
    fs::write(
        root.join("tree-template.json"),
        serde_json::to_vec(&tree).unwrap(),
    )
    .unwrap();
    let run = |code| {
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
            code,
        )
    };
    let passed = run(0);
    assert_eq!(
        passed["plan"]["execution_order"],
        json!(["facts", "test-annotation-dependency"])
    );
    let checks = passed["checks"].as_array().unwrap();
    let rule = checks
        .iter()
        .find(|check| check["id"] == "test-annotation-dependency")
        .unwrap();
    assert_eq!(rule["verdict"], "pass");
    assert_eq!(rule["matched_entities"], 1);
    assert_eq!(
        checks.iter().find(|check| check["id"] == "facts").unwrap()["metadata"]["projects"][0]["snapshot_digest"],
        passed["snapshot"]["content_digest"]
    );
    let mut absent = tree;
    absent["children"] = json!([]);
    fs::write(root.join("model-template.xml"), model("")).unwrap();
    fs::write(
        root.join("tree-template.json"),
        serde_json::to_vec(&absent).unwrap(),
    )
    .unwrap();
    let failed = run(1);
    let rule = failed["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "test-annotation-dependency")
        .unwrap();
    assert_eq!(rule["verdict"], "fail");
    assert_eq!(
        rule["diagnostics"][0]["evidence"]["assertion"],
        "require_dependency"
    );
}

#[test]
fn commit_pattern_override_changes_the_enforced_convention() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrules:\n  commit-message-convention:\n    parameters:\n      pattern: '^release: .+$'\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "release: publish"]);
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
    assert_eq!(accepted["checks"][0]["id"], "commit-message-convention");
    assert_eq!(accepted["checks"][0]["verdict"], "pass");
    git(root, &["commit", "--allow-empty", "-qm", "bad subject"]);
    let rejected = report(
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
    assert_eq!(rejected["checks"][0]["verdict"], "fail");
}
