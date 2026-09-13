mod common;
#[path = "common/reviews.rs"]
mod reviews;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "# Rules\nKeep consistent line endings.\n";

fn policy(root: &Path) -> Value {
    serde_norway::from_slice(&std::fs::read(root.join("qualitygate.yaml")).unwrap()).unwrap()
}
fn write(root: &Path, value: &Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(value).unwrap(),
    )
    .unwrap();
}
fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
    write(
        root.path(),
        &json!({"schema_version":1,"rules":{"line-ending":{"source":{
            "document":"AGENTS.md","section":"Rules","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())
        }}}}),
    );
    root
}
fn run(root: &Path, code: i32, args: &[&str]) -> Value {
    let mut command = vec!["check", "--format", "json"];
    command.extend_from_slice(args);
    report(&cli(root, &command), code)
}

#[test]
fn source_hash_refresh_alone_cannot_substitute_for_a_bound_review() {
    let root = setup();
    let root = root.path();
    let missing = run(root, 2, &[]);
    assert_eq!(
        missing["policy"]["source_reviews"]["line-ending"]["status"],
        "missing"
    );
    let listed = report(&cli(root, &["rules", "list", "--format", "json"]), 0);
    assert_eq!(
        listed["source_reviews"]["line-ending"]["expected_binding_digest"],
        missing["policy"]["source_reviews"]["line-ending"]["expected_binding_digest"]
    );
    reviews::record(root).unwrap();
    let initial = run(root, 0, &[]);
    assert_eq!(
        initial["checks"][0]["metadata"]["source_review"]["policy_trust"],
        "local_candidate"
    );
    let changed = "# Rules\nKeep consistent line endings; new files use LF.\n";
    std::fs::write(root.join("AGENTS.md"), changed).unwrap();
    let source_changed = run(root, 2, &[]);
    assert!(
        source_changed["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("digest changed")
    );
    let mut value = policy(root);
    value["rules"]["line-ending"]["source"]["content_hash"] =
        json!(qualitygate::snapshot::digest(changed.as_bytes()));
    write(root, &value);
    let stale = run(root, 2, &[]);
    assert_eq!(
        stale["policy"]["source_reviews"]["line-ending"]["status"],
        "stale"
    );
    assert_eq!(stale["checks"][0]["verdict"], Value::Null);
    reviews::record(root).unwrap();
    let mut value = policy(root);
    value["source_reviews"]["line-ending"]["resolution"] = json!("rule_unchanged");
    value["source_reviews"]["line-ending"]["reference"] =
        json!("review-system/approved-clarification");
    write(root, &value);
    let reviewed = run(root, 0, &[]);
    assert_ne!(
        reviewed["policy"]["rules_digest"],
        initial["policy"]["rules_digest"]
    );
    value["rules"]["line-ending"]["severity"] = json!("warning");
    write(root, &value);
    assert_eq!(
        run(root, 2, &[])["policy"]["source_reviews"]["line-ending"]["status"],
        "stale"
    );
}

#[test]
fn review_records_follow_selected_policy_and_staged_source_bytes() {
    let root = setup();
    let root = root.path();
    reviews::record(root).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "reviewed source mapping"]);
    let trusted = run(root, 0, &["--policy-ref", "HEAD"]);
    assert_eq!(
        trusted["checks"][0]["metadata"]["source_review"]["policy_trust"],
        "caller_supplied_ref"
    );
    std::fs::write(
        root.join("AGENTS.md"),
        "# Rules\nChanged unreviewed rule.\n",
    )
    .unwrap();
    run(root, 0, &["--staged", "--policy-ref", "HEAD"]);
    run(root, 2, &["--policy-ref", "HEAD"]);
    let mut value = policy(root);
    value["rules"]["line-ending"]["source"]["content_hash"] = json!(qualitygate::snapshot::digest(
        b"# Rules\nChanged unreviewed rule.\n"
    ));
    write(root, &value);
    reviews::record(root).unwrap();
    run(root, 0, &[]);
    let denied = run(root, 2, &["--policy-ref", "HEAD"]);
    assert!(
        denied["policy"]["changes"]
            .as_array()
            .unwrap()
            .contains(&json!("qualitygate.yaml"))
    );
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-qm", "record reviewed mapping in next policy"],
    );
    run(root, 0, &["--policy-ref", "HEAD"]);
    let mut value = policy(root);
    value["source_reviews"]["line-ending"]["reviewer"] = json!("unapproved-replacement");
    write(root, &value);
    run(root, 2, &["--policy-ref", "HEAD"]);
}

#[test]
fn review_shape_keys_and_optional_rule_completeness_remain_explicit() {
    let root = setup();
    let root = root.path();
    reviews::record(root).unwrap();
    let valid = policy(root);
    for (key, value) in [
        ("reviewer", json!("")),
        ("resolution", json!("approved")),
        ("binding_digest", json!("sha256:wrong")),
        ("unknown", json!(true)),
    ] {
        let mut invalid = valid.clone();
        invalid["source_reviews"]["line-ending"][key] = value;
        write(root, &invalid);
        report(&cli(root, &["config", "--show", "--format", "json"]), 2);
    }
    let mut orphan = valid.clone();
    orphan["source_reviews"]["foreign"] = orphan["source_reviews"]["line-ending"].clone();
    write(root, &orphan);
    run(root, 2, &[]);
    orphan["rules"]["foreign"] = json!({});
    write(root, &orphan);
    run(root, 2, &[]);
    let mut unbound = valid.clone();
    unbound["rules"]["line-ending"]
        .as_object_mut()
        .unwrap()
        .remove("source");
    write(root, &unbound);
    run(root, 2, &[]);
    let mut optional = valid;
    optional.as_object_mut().unwrap().remove("source_reviews");
    optional["rules"]["line-ending"]["required"] = json!(false);
    optional["checks"] = json!([{"id":"usable","argv":["git","--version"]}]);
    write(root, &optional);
    let incomplete_optional = run(root, 0, &[]);
    assert_eq!(incomplete_optional["checks"][0]["verdict"], Value::Null);
    assert_eq!(
        incomplete_optional["policy"]["source_reviews"]["line-ending"]["status"],
        "missing"
    );
}
