mod common;
#[path = "common/reviews.rs"]
mod reviews;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "# Tests\nAdd at least one named test for the changed behavior.\n";
fn candidate(expression: Value) -> Value {
    json!({"schema_version":2,"id":"test-contract","version":1,
        "source":{"document":"AGENTS.md","section":"Tests","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "language":["rust"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},
        "expression":expression,"fix":"Add a test named test_ for the changed behavior"})
}
fn program() -> Value {
    json!({"op":"and","args":[
        {"op":"compare","left":{"op":"count","input":{"op":"entities"}},"relation":"ge","right":{"op":"number","value":1}},
        {"op":"all","input":{"op":"entities"},"predicate":{"op":"matches","value":{"op":"field","name":"name"},"pattern":"^test_"}}
    ]})
}
fn write_candidate(root: &Path, value: &Value) {
    std::fs::write(
        root.join("candidate.yaml"),
        serde_norway::to_string(value).unwrap(),
    )
    .unwrap();
}
fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
    write_candidate(root.path(), &candidate(program()));
    let generated = report(
        &cli(
            root.path(),
            &[
                "rules",
                "generate",
                "--input",
                "candidate.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(generated["schema_id"], "urn:qualitygate:project-rule:2");
    std::fs::write(root.path().join("qualitygate.yaml"),"schema_version: 1\nrulesets: []\ncustom_rules: qualitygate/rules\nrules: {test-contract: {}}\n").unwrap();
    reviews::record(root.path()).unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    root
}
fn check(root: &Path, code: i32) -> Value {
    report(&cli(root, &["check", "--format", "json"]), code)
}

#[test]
fn v2_operators_compose_ast_and_regex_without_changing_the_v1_schema() {
    let root = setup();
    let root = root.path();
    let empty = check(root, 1);
    assert_eq!(empty["checks"][0]["metadata"]["expression"]["entities"], 0);
    std::fs::write(
        root.join("src/lib.rs"),
        "// #[test] fn test_fake() {}\n#[test] fn bad_name() {}\n",
    )
    .unwrap();
    let bad = check(root, 1);
    assert_eq!(bad["checks"][0]["metadata"]["expression"]["entities"], 1);
    let observation = &bad["checks"][0]["metadata"]["expression"]["false_observations"][0];
    assert_eq!(
        observation["expression_path"],
        "/expression/args/1/predicate"
    );
    assert_eq!(observation["file"], "src/lib.rs");
    assert_eq!(observation["range"]["start_line"], 2);
    std::fs::write(
        root.join("src/lib.rs"),
        "#[test] fn test_behavior() { assert!(true); }\n",
    )
    .unwrap();
    let passed = check(root, 0);
    assert_eq!(
        passed["checks"][0]["metadata"]["expression"]["passed"],
        true
    );
    let schema = report(&cli(root, &["schema", "operator-rule"]), 0);
    assert_eq!(
        schema,
        serde_json::from_str::<Value>(include_str!(
            "../skills/qualitygate-cli/references/schemas/operator-rule.schema.json"
        ))
        .unwrap()
    );
    assert!(
        jsonschema::draft202012::new(&schema)
            .unwrap()
            .is_valid(&candidate(program()))
    );
    let v1 = report(&cli(root, &["rules", "schema"]), 0);
    assert_eq!(v1["$id"], "urn:qualitygate:project-rule:1");
    assert!(
        !jsonschema::draft202012::new(&v1)
            .unwrap()
            .is_valid(&candidate(program()))
    );
    let mut old = candidate(program());
    old["schema_version"] = json!(1);
    old["then"] = json!({"min_count":1});
    write_candidate(root, &old);
    report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        1,
    );
    old.as_object_mut().unwrap().remove("expression");
    write_candidate(root, &old);
    report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        0,
    );
}

#[test]
fn v2_authoring_binds_the_actual_schema_and_rejects_invalid_types_and_budgets() {
    let root = setup();
    let root = root.path();
    let validated = report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        0,
    );
    assert_eq!(
        validated["files"][0]["schema_id"],
        "urn:qualitygate:project-rule:2"
    );
    let family = report(&cli(root, &["schema", "project-rule-family"]), 0);
    assert_eq!(validated["schema_id"], family["$id"]);
    assert_eq!(
        validated["schema_digest"],
        qualitygate::snapshot::digest(&serde_json::to_vec(&family).unwrap())
    );
    let validator = jsonschema::draft202012::new(&family).unwrap();
    assert!(validator.is_valid(&candidate(program())));
    let mut old = candidate(program());
    old["schema_version"] = json!(1);
    old["id"] = json!("old-contract");
    old["then"] = json!({"min_count":1});
    old.as_object_mut().unwrap().remove("expression");
    assert!(validator.is_valid(&old));
    std::fs::write(
        root.join("qualitygate/rules/old-contract.yaml"),
        serde_norway::to_string(&old).unwrap(),
    )
    .unwrap();
    let mixed = report(
        &cli(
            root,
            &["rules", "validate", "qualitygate/rules", "--format", "json"],
        ),
        0,
    );
    assert_eq!(mixed["schema_id"], family["$id"]);
    assert_eq!(mixed["files"].as_array().unwrap().len(), 2);
    let mut nested = json!({"op":"bool","value":true});
    for _ in 0..17 {
        nested = json!({"op":"not","arg":nested});
    }
    for expression in [
        json!({"op":"all","input":{"op":"number","value":1},"predicate":{"op":"bool","value":true}}),
        json!({"op":"field","name":"name"}),
        json!({"op":"eval","code":"return true"}),
        json!({"op":"bool","value":true,"extra":1}),
        json!({"op":"matches","value":{"op":"string","value":"x"},"pattern":"["}),
        nested,
    ] {
        write_candidate(root, &candidate(expression));
        report(
            &cli(
                root,
                &["rules", "validate", "candidate.yaml", "--format", "json"],
            ),
            1,
        );
    }
    let mut missing = candidate(program());
    missing.as_object_mut().unwrap().remove("expression");
    write_candidate(root, &missing);
    report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        1,
    );
}

#[test]
fn parser_and_required_fact_gaps_cannot_be_hidden_by_a_true_expression() {
    let root = setup();
    let root = root.path();
    let mut rule = candidate(json!({"op":"not","arg":{"op":"bool","value":false}}));
    std::fs::write(
        root.join("qualitygate/rules/test-contract.yaml"),
        serde_norway::to_string(&rule).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
    std::fs::write(root.join("src/lib.rs"), "#[test] fn broken( {\n").unwrap();
    let malformed = check(root, 2);
    assert!(malformed["checks"][0]["verdict"].is_null());
    assert!(malformed["checks"][0]["metadata"]["expression"].is_null());
    std::fs::write(root.join("src/lib.rs"), "#[test] fn test_behavior() {}\n").unwrap();
    rule["requires_capabilities"] = json!(["test_methods", "dependency_resolution"]);
    std::fs::write(
        root.join("qualitygate/rules/test-contract.yaml"),
        serde_norway::to_string(&rule).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
    let missing = check(root, 2);
    assert!(missing.to_string().contains("dependency_resolution"));
    assert!(missing["checks"][0]["verdict"].is_null());
}

#[test]
fn source_changes_and_staged_selection_bind_expression_results_to_the_selected_snapshot() {
    let root = setup();
    let root = root.path();
    std::fs::write(root.join("src/lib.rs"), "#[test] fn test_behavior() {}\n").unwrap();
    git(root, &["add", "."]);
    let staged = report(&cli(root, &["check", "--staged", "--format", "json"]), 0);
    std::fs::write(root.join("src/lib.rs"), "#[test] fn bad_name() {}\n").unwrap();
    report(&cli(root, &["check", "--staged", "--format", "json"]), 0);
    let changed = check(root, 1);
    assert_ne!(
        staged["snapshot"]["content_digest"],
        changed["snapshot"]["content_digest"]
    );
    std::fs::write(root.join("AGENTS.md"), "# Tests\nChanged requirement.\n").unwrap();
    assert!(
        check(root, 2)["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("digest changed")
    );
}

#[test]
fn operator_exhaustion_is_incomplete_and_retains_fixed_assertion_violations() {
    let root = setup();
    let root = root.path();
    let mut rule = candidate(json!({"op":"all","input":{"op":"entities"},"predicate":{
        "op":"all","input":{"op":"entities"},"predicate":{"op":"bool","value":true}}}));
    rule["then"] = json!({"min_count":400});
    std::fs::write(
        root.join("qualitygate/rules/test-contract.yaml"),
        serde_norway::to_string(&rule).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
    let source = (0..300)
        .map(|i| format!("#[test] fn test_{i}() {{}}\n"))
        .collect::<String>();
    std::fs::write(root.join("src/lib.rs"), source).unwrap();
    let exhausted = check(root, 2);
    let result = &exhausted["checks"][0];
    assert!(result["verdict"].is_null());
    assert!(
        result["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("operations")
    );
    assert_eq!(
        result["diagnostics"][0]["evidence"]["assertion"],
        "min_count"
    );
}

#[tokio::test]
async fn unused_declared_project_facts_still_require_a_supported_snapshot_binding() {
    let root = setup();
    let snapshot = qualitygate::snapshot::capture(
        root.path(),
        &qualitygate::snapshot::Selection::Worktree {
            base: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    let mut value = candidate(json!({"op":"bool","value":true}));
    value["requires_capabilities"] = json!(["test_methods", "dependency_resolution"]);
    let rule =
        qualitygate::config::rule_schema::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    let project = json!({"schema_version":1,"ecosystem":"maven","root":".","manifest":"pom.xml","coordinate":"fixture",
        "source_root":"src/main/java","test_source_root":"src/test/java","declared":[],"resolved":[],
        "producer_check":"fixture-producer","snapshot_digest":snapshot.identity.content_digest});
    for (field, invalid) in [
        ("schema_version", json!(99)),
        ("ecosystem", json!("unknown")),
        ("snapshot_digest", json!("another snapshot")),
    ] {
        let mut facts = project.clone();
        facts[field] = invalid;
        let facts = serde_json::from_value(facts).unwrap();
        let result = qualitygate::adapters::custom_rules::evaluate_with_projects(
            &rule,
            &qualitygate::config::RuleSetting::default(),
            &snapshot,
            &[facts],
        );
        assert!(result.verdict.is_none());
        assert!(
            result
                .execution
                .reason
                .unwrap()
                .contains("Required project facts")
        );
    }
}

#[path = "operators/v3.rs"]
mod v3;
