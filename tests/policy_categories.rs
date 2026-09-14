mod common;

use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn run(root: &Path, args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);
    report(&cli(root, &args), 0)
}

#[test]
fn memberships_are_additive_and_category_changes_preserve_enforcement() {
    let root = fixture();
    let root = root.path();
    run(root, &["init"]);
    let before = run(root, &["config", "--show"])["config"].clone();
    for category in ["migration", "security-audit"] {
        run(root, &["rules", "categories", "create", category]);
        run(
            root,
            &["rules", "assign", "line-ending", "--category", category],
        );
    }
    let assigned = run(root, &["rules", "describe", "line-ending"]);
    assert_eq!(
        assigned["categories"],
        json!(["migration", "security-audit"])
    );
    let bytes = std::fs::read(root.join("qualitygate.yaml")).unwrap();
    assert_eq!(
        run(
            root,
            &["rules", "assign", "line-ending", "--category", "migration"]
        )["changed"],
        false
    );
    assert_eq!(std::fs::read(root.join("qualitygate.yaml")).unwrap(), bytes);
    for category in ["migration", "security-audit"] {
        let filtered = run(root, &["rules", "list", "--category", category]);
        assert!(
            filtered["rules"]
                .as_array()
                .unwrap()
                .iter()
                .any(|rule| rule["id"] == "line-ending")
        );
    }
    run(
        root,
        &["rules", "categories", "rename", "migration", "refactor"],
    );
    assert_eq!(
        run(root, &["rules", "describe", "line-ending"])["categories"],
        json!(["refactor", "security-audit"])
    );
    run(
        root,
        &["rules", "categories", "delete", "refactor", "--force"],
    );
    assert_eq!(
        run(root, &["rules", "describe", "line-ending"])["categories"],
        json!(["security-audit"])
    );
    run(
        root,
        &[
            "rules",
            "unassign",
            "line-ending",
            "--category",
            "security-audit",
        ],
    );
    assert_eq!(
        run(root, &["rules", "describe", "line-ending"])["categories"],
        json!(["core"])
    );
    let after = run(root, &["config", "--show"])["config"].clone();
    for field in ["rules", "checks", "rulesets", "profiles", "source_reviews"] {
        assert_eq!(before[field], after[field], "{field}");
    }
    assert_eq!(
        run(root, &["rules", "categories"]),
        run(root, &["rules", "categories", "list"])
    );
}

#[test]
fn mandatory_context_survives_filters_and_local_policy_weakening() {
    let root = fixture();
    let root = root.path();
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrules: {line-ending: {required: true}}\nchecks: [{id: required-test, argv: [git, --version]}]\ncategories: {empty: {description: Nothing selected}}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "protected policy"]);
    let digest =
        qualitygate::snapshot::digest(&std::fs::read(root.join("qualitygate.yaml")).unwrap());
    let filtered = run(
        root,
        &[
            "rules",
            "list",
            "--category",
            "empty",
            "--source",
            "project",
            "--language",
            "java",
        ],
    );
    assert_eq!(filtered["rules"], json!([]));
    assert_eq!(filtered["mandatory"][0]["id"], "line-ending");
    assert_eq!(filtered["mandatory_checks"][0]["id"], "required-test");
    assert_eq!(filtered["policy_digest"], digest);
    run(root, &["rules", "disable", "line-ending"]);
    let local = run(root, &["rules", "context", "--category", "empty"]);
    assert_eq!(local["mandatory"], json!([]));
    let protected = run(
        root,
        &[
            "rules",
            "context",
            "--category",
            "empty",
            "--policy-ref",
            "HEAD",
        ],
    );
    assert_eq!(protected["selected"], json!([]));
    assert_eq!(protected["mandatory"][0]["id"], "line-ending");
    assert_eq!(protected["mandatory_checks"][0]["id"], "required-test");
    assert_eq!(protected["policy_digest"], digest);
    assert_eq!(protected["review_trust"], "caller_supplied_ref");
    for format in ["table", "markdown"] {
        let output = cli(
            root,
            &["rules", "list", "--category", "empty", "--format", format],
        );
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Mandatory policy rules"));
        assert!(text.contains("required-test"));
    }
}

#[test]
fn invalid_memberships_fail_without_publication() {
    for memberships in [
        "[]",
        "[core, core]",
        "[unknown]",
        "[core, 42]",
        "{category: core}",
    ] {
        let yaml = format!("schema_version: 1\nrule_categories: {{line-ending: {memberships}}}\n");
        assert!(
            qualitygate::config::parse(yaml.as_bytes()).is_err(),
            "{memberships}"
        );
    }
    let root = fixture();
    run(root.path(), &["init"]);
    let before = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
    for args in [
        vec!["rules", "unassign", "line-ending", "--category", "core"],
        vec!["rules", "unassign", "missing-rule", "--category", "core"],
    ] {
        report(
            &cli(
                root.path(),
                &[args.as_slice(), &["--format", "json"]].concat(),
            ),
            2,
        );
        assert_eq!(
            std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
            before
        );
    }
}
