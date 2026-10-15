mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "# Agent behavior\nRecheck the final snapshot before delivery.\n";
fn obligation() -> Value {
    json!({"schema_version":1,"id":"final-check","version":1,"required":true,
        "source":{"document":"AGENTS.md","section":"Agent behavior","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "scope":{"roles":[],"tasks":[]},"triggers":["start","repair","resume","handoff","before_delivery"],
        "must":["Recheck the final snapshot before delivery"],"must_not":["Treat agent-authored reports as verified evidence"],
        "enforcement":{"kind":"host","control":"final_full_check"}})
}
fn config() -> Value {
    json!({"schema_version":1,"rules":{"line-ending":{}},"agent_obligations":[obligation()]})
}
fn write(root: &Path, config: &Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(config).unwrap(),
    )
    .unwrap();
}
fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
    write(root.path(), &config());
    root
}
fn adopt(root: &Path, value: &mut Value) {
    write(root, value);
    let effective = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    for o in value["agent_obligations"].as_array_mut().unwrap() {
        o["adoption"] = json!({"id":format!("adopt-{}",o["id"].as_str().unwrap()),"reviewer":"fixture reviewer",
            "reference":"fixture review","resolution":"initial_mapping","rationale":"Reviewed behavior and enforcement boundary",
            "binding_digest":effective["agent_bindings"][o["id"].as_str().unwrap()]});
    }
    write(root, value);
}
fn project(root: &Path, code: i32, args: &[&str]) -> Value {
    let mut argv = vec!["agent-contract", "--format", "json"];
    argv.extend_from_slice(args);
    report(&cli(root, &argv), code)
}

#[test]
fn only_adopted_obligations_enter_the_consuming_agent_prompt_with_distinct_bindings() {
    let root = setup();
    let root = root.path();
    let pending = project(root, 2, &[]);
    assert_eq!(pending["obligations"][0]["status"], "unadopted");
    assert!(
        !pending["prompt"]
            .as_str()
            .unwrap()
            .contains("\"final-check\"")
    );
    let mut value = config();
    adopt(root, &mut value);
    let first = project(root, 0, &[]);
    assert_eq!(first["ready"], true);
    assert_eq!(first["policy"]["trust"], "local_candidate");
    assert!(
        first["prompt"]
            .as_str()
            .unwrap()
            .contains("Recheck the final snapshot before delivery")
    );
    assert_eq!(
        first["prompt_digest"],
        qualitygate::snapshot::digest(first["prompt"].as_str().unwrap().as_bytes())
    );
    assert_eq!(
        first["contract_digest"],
        project(root, 0, &[])["contract_digest"]
    );
    std::fs::write(root.join("changed.txt"), "new code\n").unwrap();
    let changed = project(root, 0, &[]);
    assert_eq!(changed["contract_digest"], first["contract_digest"]);
    assert_ne!(
        changed["snapshot"]["content_digest"],
        first["snapshot"]["content_digest"]
    );
    assert_ne!(changed["prompt_digest"], first["prompt_digest"]);
    value["agent_obligations"][0]["must"] = json!(["Changed interpretation"]);
    write(root, &value);
    let stale = project(root, 2, &[]);
    assert_eq!(stale["obligations"][0]["status"], "stale_adoption");
    assert_ne!(stale["contract_digest"], first["contract_digest"]);
}

#[test]
fn source_policy_and_staged_bindings_cannot_be_replaced_by_live_candidate_edits() {
    let root = setup();
    let root = root.path();
    let mut value = config();
    adopt(root, &mut value);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "adopt agent contract"]);
    let pinned = project(root, 0, &["--policy-ref", "HEAD"]);
    assert_eq!(pinned["policy"]["trust"], "caller_supplied_ref");
    std::fs::write(
        root.join("AGENTS.md"),
        "# Agent behavior\nA changed requirement.\n",
    )
    .unwrap();
    project(root, 0, &["--staged", "--policy-ref", "HEAD"]);
    project(root, 0, &["--diff", "HEAD..HEAD", "--policy-ref", "HEAD"]);
    let invalid = project(root, 2, &["--policy-ref", "HEAD"]);
    assert_eq!(invalid["obligations"][0]["status"], "source_invalid");
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    value["agent_obligations"] = json!([]);
    write(root, &value);
    let changed = project(root, 2, &["--policy-ref", "HEAD"]);
    assert_eq!(changed["obligations"].as_array().unwrap().len(), 1);
    assert_eq!(changed["policy"]["changes"], json!(["qualitygate.yaml"]));
    value = config();
    value["exclude"] = json!(["AGENTS.md"]);
    write(root, &value);
    assert!(project(root, 2, &[]).to_string().contains("protected"));
}

#[test]
fn check_obligations_bind_definitions_and_projection_never_runs_formal_checks() {
    let root = setup();
    let root = root.path();
    let mut value = config();
    value["checks"] =
        json!([{"id":"external","kind":"command","argv":["agent-contract-must-not-launch-this"]}]);
    value["agent_obligations"][0]["enforcement"] = json!({"kind":"checks","checks":["external"]});
    adopt(root, &mut value);
    let first = project(root, 0, &[]);
    assert_eq!(first["obligations"][0]["status"], "ready");
    value["checks"][0]["argv"] = json!(["another-producer"]);
    write(root, &value);
    assert_eq!(
        project(root, 2, &[])["obligations"][0]["status"],
        "stale_adoption"
    );
    value["profiles"] = json!({"full":{"include":["line-ending"]}});
    value["checks"][0]["required"] = json!(false);
    adopt(root, &mut value);
    assert_eq!(
        project(root, 2, &[])["obligations"][0]["status"],
        "unavailable_check"
    );
}

#[test]
fn role_task_scope_conflicts_and_optional_gaps_remain_explicit() {
    let root = setup();
    let root = root.path();
    let mut value = config();
    value["agent_obligations"][0]["scope"] = json!({"roles":["implementer"],"tasks":["task-a"]});
    adopt(root, &mut value);
    assert_eq!(project(root, 0, &[])["obligations"], json!([]));
    std::fs::write(root.join("task.yaml"), "schema_version: 1\ntask_id: task-a\nacceptance:\n  - id: acceptance\n    description: required producer\n    verification: {kind: command, check_id: producer, argv: [git, --version]}\n").unwrap();
    assert_eq!(
        project(root, 0, &["--task", "task.yaml"])["obligations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        project(root, 0, &["--task", "task.yaml", "--role", "reviewer"])["obligations"],
        json!([])
    );
    let mut other = obligation();
    other["id"] = json!("conflict");
    other["must_not"] = value["agent_obligations"][0]["must"].clone();
    value["agent_obligations"]
        .as_array_mut()
        .unwrap()
        .push(other);
    adopt(root, &mut value);
    assert!(
        project(root, 2, &["--task", "task.yaml"])["gaps"]
            .to_string()
            .contains("Conflicting")
    );
    value["agent_obligations"][1]["required"] = json!(false);
    value["agent_obligations"][1]
        .as_object_mut()
        .unwrap()
        .remove("adoption");
    write(root, &value);
    let optional = project(root, 0, &["--task", "task.yaml"]);
    assert_eq!(optional["obligations"][1]["status"], "unadopted");
    assert!(
        !optional["prompt"]
            .as_str()
            .unwrap()
            .contains("\"conflict\"")
    );
}

#[test]
fn obligation_schema_and_runtime_reject_unknown_contracts_and_unbounded_inputs() {
    let root = setup();
    let root = root.path();
    let schema = report(&cli(root, &["schema", "agent-obligation"]), 0);
    assert_eq!(
        schema,
        serde_json::from_str::<Value>(include_str!(
            "../skills/qualitygate-cli/references/schemas/agent-obligation.schema.json"
        ))
        .unwrap()
    );
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert!(validator.is_valid(&obligation()));
    for (field, bad) in [
        ("schema_version", json!(2)),
        ("unknown", json!(true)),
        ("enforcement", json!({"kind":"host","control":"anything"})),
        ("triggers", json!(["start", "start"])),
        ("scope", json!({"roles":["bad role"],"tasks":[]})),
        ("must", json!(["x".repeat(4097)])),
        ("must_not", json!(["\ncontrol"])),
    ] {
        let mut value = config();
        value["agent_obligations"][0][field] = bad;
        write(root, &value);
        report(&cli(root, &["config", "--show", "--format", "json"]), 2);
    }
    let mut value = config();
    value["agent_obligations"] = json!(vec![obligation(); 129]);
    write(root, &value);
    report(&cli(root, &["config", "--show", "--format", "json"]), 2);
}
