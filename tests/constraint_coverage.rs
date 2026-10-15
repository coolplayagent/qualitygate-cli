mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "# Quality\nUse consistent line endings.\n";

fn candidate() -> Value {
    json!({"schema_version":1,"rules":{"line-ending":{}},"constraints":[{
        "schema_version":1,"id":"line-endings","version":1,"required":true,
        "statement":"Use consistent line endings in changed files.",
        "source":{"document":"AGENTS.md","section":"Quality","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "realization":{"kind":"checks","mechanism":"regex","checks":["line-ending"],
            "positive_examples":["LF file"],"negative_examples":["mixed LF and CRLF"]}
    }]})
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
    write(root.path(), &candidate());
    root
}
fn adopt(root: &Path, value: &mut Value) {
    write(root, value);
    let config = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    for c in value["constraints"].as_array_mut().unwrap() {
        c["adoption"] = json!({"id":format!("adopt-{}",c["id"].as_str().unwrap()),
            "reviewer":"fixture reviewer","reference":"fixture review","resolution":"initial_mapping",
            "rationale":"Check scope and examples reviewed",
            "binding_digest":config["constraint_bindings"][c["id"].as_str().unwrap()]});
    }
    write(root, value);
}
fn run(root: &Path, code: i32, args: &[&str]) -> Value {
    let mut command = vec!["check", "--format", "json"];
    command.extend_from_slice(args);
    report(&cli(root, &command), code)
}
fn coverage(report: &Value) -> &Value {
    &report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "constraint-coverage")
        .unwrap()["metadata"]["constraint_coverage"]
}

#[test]
fn clauses_require_adoption_and_current_check_definitions_before_execution_counts() {
    let root = setup();
    let root = root.path();
    let unadopted = run(root, 2, &[]);
    assert_eq!(coverage(&unadopted)["clauses"][0]["status"], "unadopted");
    let mut value = candidate();
    adopt(root, &mut value);
    let passed = run(root, 0, &[]);
    assert_eq!(coverage(&passed)["clauses"][0]["status"], "verified");
    assert_eq!(coverage(&passed)["snapshot"], passed["snapshot"]);
    assert_eq!(
        coverage(&passed)["rules_digest"],
        passed["policy"]["rules_digest"]
    );
    assert_eq!(coverage(&passed)["policy_trust"], "local_candidate");
    assert_eq!(
        passed["plan"]["execution_order"],
        json!(["line-ending", "constraint-coverage"])
    );
    value["rules"]["line-ending"]["severity"] = json!("warning");
    write(root, &value);
    let stale = run(root, 2, &[]);
    assert_eq!(coverage(&stale)["clauses"][0]["status"], "stale_adoption");
    assert_ne!(
        passed["policy"]["rules_digest"],
        stale["policy"]["rules_digest"]
    );
    adopt(root, &mut value);
    std::fs::write(root.join("hello.txt"), "first\r\nsecond\n").unwrap();
    let failed = run(root, 1, &[]);
    assert_eq!(coverage(&failed)["clauses"][0]["status"], "failed");
    // A required clause cannot silently inherit warning-only enforcement.
    assert_eq!(failed["checks"][1]["verdict"], "fail");
}

#[test]
fn snapshot_policy_changes_and_excluded_sources_cannot_reuse_coverage() {
    let root = setup();
    let root = root.path();
    let mut value = candidate();
    adopt(root, &mut value);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "adopt clauses"]);
    let trusted = run(root, 0, &["--policy-ref", "HEAD"]);
    assert_eq!(coverage(&trusted)["policy_trust"], "caller_supplied_ref");
    std::fs::write(
        root.join("AGENTS.md"),
        "# Quality\nUse a different policy.\n",
    )
    .unwrap();
    run(root, 0, &["--staged", "--policy-ref", "HEAD"]);
    let changed = run(root, 2, &["--policy-ref", "HEAD"]);
    assert_eq!(coverage(&changed)["clauses"][0]["status"], "source_invalid");
    assert_ne!(
        changed["snapshot"]["content_digest"],
        trusted["snapshot"]["content_digest"]
    );
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    value["constraints"][0]["statement"] = json!("Changed interpretation");
    adopt(root, &mut value);
    let altered = run(root, 2, &["--policy-ref", "HEAD"]);
    assert_eq!(altered["policy"]["changes"], json!(["qualitygate.yaml"]));
    value["exclude"] = json!(["AGENTS.md"]);
    write(root, &value);
    let excluded = run(root, 2, &[]);
    assert!(
        excluded
            .to_string()
            .contains("protected verification input")
    );
}

#[test]
fn unsupported_optional_and_unselected_checks_remain_visible_without_false_coverage() {
    let root = setup();
    let root = root.path();
    let mut value = candidate();
    value["constraints"][0]["realization"] = json!({"kind":"unsupported","reason":"Needs host interception","evidence_owner":"Agent host"});
    write(root, &value);
    assert_eq!(
        coverage(&run(root, 2, &[]))["clauses"][0]["status"],
        "unsupported"
    );
    value["constraints"][0]["required"] = json!(false);
    write(root, &value);
    assert_eq!(
        coverage(&run(root, 0, &[]))["clauses"][0]["status"],
        "unsupported"
    );
    value = candidate();
    value["checks"] = json!([{"id":"version","argv":["git","--version"]}]);
    value["constraints"][0]["realization"]["checks"] = json!(["version"]);
    adopt(root, &mut value);
    assert_eq!(
        coverage(&run(root, 2, &["--profile", "quick"]))["clauses"][0]["status"],
        "pending"
    );
    assert_eq!(
        coverage(&run(root, 0, &[]))["clauses"][0]["status"],
        "verified"
    );
    value["rules"]["line-ending"]["enabled"] = json!(false);
    value["constraints"][0]["realization"]["checks"] = json!(["line-ending"]);
    adopt(root, &mut value);
    assert_eq!(
        coverage(&run(root, 2, &[]))["clauses"][0]["status"],
        "pending"
    );
}

#[test]
fn timeout_and_failed_producers_keep_distinct_clause_evidence() {
    let root = setup();
    let root = root.path();
    let producer = tempfile::tempdir().unwrap();
    let source = producer.path().join("probe.rs");
    let binary = producer.path().join("probe.exe");
    std::fs::write(&source, include_str!("doctor/native_probe.rs")).unwrap();
    let built = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let mut value = candidate();
    for (mode, exit, status) in [("timeout-long", 2, "pending"), ("failure", 1, "failed")] {
        value["checks"] = json!([{"id":"producer","argv":[binary,mode],"timeout_seconds":1}]);
        value["constraints"][0]["realization"]["checks"] = json!(["producer"]);
        adopt(root, &mut value);
        let result = run(root, exit, &[]);
        assert_eq!(coverage(&result)["clauses"][0]["status"], status);
        if mode == "timeout-long" {
            assert_eq!(
                result["checks"][1]["execution"]["status"], "timed_out",
                "{result:#}"
            );
        }
    }
}

#[test]
fn clause_schema_is_exported_offline_and_runtime_rejects_invalid_inventories() {
    let root = setup();
    let root = root.path();
    let schema = report(&cli(root, &["schema", "constraint"]), 0);
    assert_eq!(
        schema,
        serde_json::from_str::<Value>(include_str!(
            "../skills/qualitygate-cli/references/schemas/constraint.schema.json"
        ))
        .unwrap()
    );
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert!(validator.is_valid(&candidate()["constraints"][0]));
    let cases = [
        ("/constraints/0/version", json!(0)),
        ("/constraints/0/statement", json!(" ")),
        ("/constraints/0/source/document", json!("../outside")),
        ("/constraints/0/realization/checks", json!([])),
        ("/constraints/0/realization/checks", json!(["missing"])),
        (
            "/constraints/0/realization/checks",
            json!(["line-ending", "line-ending"]),
        ),
        (
            "/constraints/0/realization/checks",
            json!(["constraint-coverage"]),
        ),
        ("/constraints/0/realization/positive_examples", json!([])),
        (
            "/constraints/0/realization/negative_examples",
            json!(["x", "x"]),
        ),
        ("/constraints/0/schema_version", json!(2)),
    ];
    for (path, replacement) in cases {
        let mut value = candidate();
        *value.pointer_mut(path).unwrap() = replacement;
        write(root, &value);
        report(&cli(root, &["config", "--show", "--format", "json"]), 2);
    }
    let mut value = candidate();
    value["constraints"][0]["unknown"] = json!(true);
    assert!(!validator.is_valid(&value["constraints"][0]));
    write(root, &value);
    run(root, 2, &[]);
    let mut value = candidate();
    let duplicate = value["constraints"][0].clone();
    value["constraints"].as_array_mut().unwrap().push(duplicate);
    write(root, &value);
    run(root, 2, &[]);
}

#[test]
fn inventory_limits_and_review_bindings_reject_oversized_or_ambiguous_declarations() {
    let valid = candidate();
    let c = valid["constraints"][0].clone();
    let cases = [
        ("/constraints", json!(vec![c; 257])),
        ("/constraints/0/statement", json!("x".repeat(4097))),
        ("/constraints/0/statement", json!("hidden\ncontrol")),
        (
            "/constraints/0/realization/checks",
            json!(vec!["line-ending"; 65]),
        ),
        (
            "/constraints/0/realization/positive_examples",
            json!(vec!["example"; 17]),
        ),
        ("/constraints/0/source/section", json!("x".repeat(1025))),
        ("/constraints/0/source/document", json!("./AGENTS.md")),
    ];
    for (path, replacement) in cases {
        let mut value = valid.clone();
        *value.pointer_mut(path).unwrap() = replacement;
        assert!(
            qualitygate::config::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{path}"
        );
    }
    let root = setup();
    let root = root.path();
    let mut adopted = valid;
    adopt(root, &mut adopted);
    let mut second = adopted["constraints"][0].clone();
    second["id"] = json!("another");
    adopted["constraints"].as_array_mut().unwrap().push(second);
    assert!(qualitygate::config::parse(&serde_json::to_vec(&adopted).unwrap()).is_err());
    adopted["constraints"].as_array_mut().unwrap().pop();
    adopted["constraints"][0]["adoption"]["reviewer"] = json!("");
    assert!(qualitygate::config::parse(&serde_json::to_vec(&adopted).unwrap()).is_err());
    let mut reserved = candidate();
    reserved["checks"] = json!([{"id":"constraint-coverage","argv":["git","--version"]}]);
    assert!(qualitygate::config::parse(&serde_json::to_vec(&reserved).unwrap()).is_err());
}

#[test]
fn frozen_policy_retains_clause_sources_and_doctor_preserves_the_same_plan() {
    let root = setup();
    let root = root.path();
    let mut value = candidate();
    adopt(root, &mut value);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "clause policy"]);
    let evidence = json!({"schema_version":1,"kind":"review",
        "source_digest":qualitygate::snapshot::digest(SOURCE.as_bytes()),"scope":"project","timestamp":1,
        "actor":{"id":"fixture-reviewer","kind":"human"},"claims":["Review clause mapping"],
        "sensitivity":"internal","rule_ids":["line-ending"],"known_limits":["Fixture declaration"],"unverified_assumptions":[]});
    std::fs::write(
        root.join("evidence.json"),
        serde_json::to_vec(&evidence).unwrap(),
    )
    .unwrap();
    let retained = report(
        &cli(
            root,
            &[
                "policy",
                "evidence",
                "add",
                "--input",
                "evidence.json",
                "--source",
                "AGENTS.md",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let created = report(
        &cli(
            root,
            &[
                "policy",
                "candidate",
                "create",
                "--from-policy-ref",
                "HEAD",
                "--evidence",
                retained["evidence_ref"].as_str().unwrap(),
                "--reason",
                "Review source coverage",
                "--actor",
                "fixture-author",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let frozen = report(
        &cli(
            root,
            &[
                "policy",
                "show",
                created["revision"]["parent_policy_digest"]
                    .as_str()
                    .unwrap(),
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(
        frozen["version"]["files"]["AGENTS.md"]["digest"],
        qualitygate::snapshot::digest(SOURCE.as_bytes())
    );
    assert_eq!(frozen["config"]["constraints"], value["constraints"]);
    let doctor = report(
        &cli(
            root,
            &[
                "doctor",
                "--worktree",
                "--profile",
                "full",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let checked = run(root, 0, &[]);
    assert_eq!(doctor["planned_checks"], checked["plan"]["execution_order"]);
}
