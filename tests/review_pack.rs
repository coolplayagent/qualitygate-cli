mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const DOCUMENT: &str = "skills/qualitygate-cli/references/review-pack.md";
const CARDS: &str = include_str!("../skills/qualitygate-cli/references/review-pack.md");
const EXAMPLE: &str =
    include_str!("../skills/qualitygate-cli/references/examples/review-obligations.yaml");

fn config() -> Value {
    let example: Value = serde_norway::from_str(EXAMPLE).unwrap();
    json!({"schema_version":1,"rules":{"line-ending":{}},
        "agent_obligations":example["agent_obligations"]})
}

fn write_config(root: &Path, value: &Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(value).unwrap(),
    )
    .unwrap();
}

fn setup() -> tempfile::TempDir {
    let root = fixture();
    let document = root.path().join(DOCUMENT);
    std::fs::create_dir_all(document.parent().unwrap()).unwrap();
    std::fs::write(document, CARDS).unwrap();
    write_config(root.path(), &config());
    root
}

fn project(root: &Path, role: &str) -> Value {
    report(
        &cli(
            root,
            &[
                "agent-contract",
                "--worktree",
                "--role",
                role,
                "--format",
                "json",
            ],
        ),
        0,
    )
}

// Synthetic approval belongs only to this isolated test repository.
fn adopt_for_fixture(root: &Path, value: &mut Value) {
    write_config(root, value);
    let effective = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    for obligation in value["agent_obligations"].as_array_mut().unwrap() {
        let id = obligation["id"].as_str().unwrap();
        obligation["adoption"] = json!({"id":format!("fixture-{id}"),
            "reviewer":"isolated fixture reviewer","reference":"synthetic fixture review",
            "resolution":"initial_mapping","rationale":"Exercise instruction projection only",
            "binding_digest":effective["agent_bindings"][id]});
    }
    write_config(root, value);
}

#[test]
fn packaged_review_obligations_match_schema_and_exact_cli_sources() {
    let root = setup();
    let schema = report(&cli(root.path(), &["schema", "agent-obligation"]), 0);
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let value = config();
    let obligations = value["agent_obligations"].as_array().unwrap();
    assert_eq!(obligations.len(), 6);
    for obligation in obligations {
        assert!(validator.is_valid(obligation), "{obligation}");
        assert_eq!(obligation["required"], false);
        assert!(obligation.get("adoption").is_none());
        assert_eq!(obligation["enforcement"], json!({"kind":"instruction"}));
        let source = report(
            &cli(
                root.path(),
                &[
                    "rules",
                    "source",
                    "--document",
                    DOCUMENT,
                    "--section",
                    obligation["source"]["section"].as_str().unwrap(),
                    "--format",
                    "json",
                ],
            ),
            0,
        );
        assert_eq!(obligation["source"], source["source"]);
    }
}

#[test]
fn shipped_attributes_preserve_review_source_bindings_under_autocrlf_checkout() {
    let root = setup();
    let root = root.path();
    std::fs::write(
        root.join(".gitattributes"),
        include_str!("../.gitattributes"),
    )
    .unwrap();
    std::fs::write(root.join("autocrlf-control.txt"), "control line\n").unwrap();
    git(
        root,
        &["add", ".gitattributes", DOCUMENT, "autocrlf-control.txt"],
    );
    git(root, &["commit", "-qm", "record packaged review source"]);
    git(root, &["config", "core.autocrlf", "true"]);
    std::fs::remove_file(root.join(DOCUMENT)).unwrap();
    std::fs::remove_file(root.join("autocrlf-control.txt")).unwrap();
    git(root, &["checkout", "--", DOCUMENT, "autocrlf-control.txt"]);

    // The control proves checkout conversion occurred; the bound source must
    // retain its exact packaged bytes because the shipped attributes override it.
    assert_eq!(
        std::fs::read(root.join("autocrlf-control.txt")).unwrap(),
        b"control line\r\n"
    );
    assert_eq!(
        std::fs::read(root.join(DOCUMENT)).unwrap(),
        CARDS.as_bytes()
    );
    for obligation in config()["agent_obligations"].as_array().unwrap() {
        let source = report(
            &cli(
                root,
                &[
                    "rules",
                    "source",
                    "--document",
                    DOCUMENT,
                    "--section",
                    obligation["source"]["section"].as_str().unwrap(),
                    "--format",
                    "json",
                ],
            ),
            0,
        );
        assert_eq!(source["source"], obligation["source"]);
    }
}

#[test]
fn real_pack_requires_adoption_and_projects_actionable_instructions_for_selected_roles() {
    let root = setup();
    let root = root.path();
    let pending = project(root, "reviewer");
    assert_eq!(pending["ready"], true);
    assert_eq!(pending["obligations"].as_array().unwrap().len(), 6);
    for bound in pending["obligations"].as_array().unwrap() {
        assert_eq!(bound["status"], "unadopted");
        assert!(!bound["reasons"].as_array().unwrap().is_empty());
    }
    assert!(pending["gaps"].as_array().unwrap().is_empty());
    assert!(!pending["prompt"].as_str().unwrap().contains("CAS failure"));

    let mut value = config();
    adopt_for_fixture(root, &mut value);
    for role in ["reviewer", "implementer"] {
        let adopted = project(root, role);
        assert_eq!(adopted["ready"], true);
        for bound in adopted["obligations"].as_array().unwrap() {
            assert_eq!(bound["status"], "ready");
        }
        let prompt = adopted["prompt"].as_str().unwrap();
        for evidence in [
            "CAS failure handling",
            "dominating null checks",
            "source availability and destination capacity",
            "shared behavior contract",
            "replacement assertions and execution scope",
            "root-cause tracking",
            "insufficient_evidence",
            "counterexamples",
        ] {
            assert!(prompt.contains(evidence), "missing {evidence}: {prompt}");
        }
    }
    let other_role = project(root, "observer");
    assert_eq!(other_role["obligations"], json!([]));
    assert!(
        !other_role["prompt"]
            .as_str()
            .unwrap()
            .contains("CAS failure")
    );
    let checked = report(
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
        0,
    );
    assert_eq!(checked["gate"]["complete"], true);
    assert_eq!(checked["gate"]["decision"], "pass");
    assert!(
        checked["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| { !check["id"].as_str().unwrap().starts_with("review-") })
    );
}

#[test]
fn changing_a_card_or_its_interpretation_excludes_stale_optional_instructions() {
    let root = setup();
    let root = root.path();
    let mut value = config();
    adopt_for_fixture(root, &mut value);
    let initial = project(root, "reviewer");
    std::fs::write(
        root.join(DOCUMENT),
        CARDS.replace(
            "Card `review-concurrency`, version 1.",
            "Card `review-concurrency`, version 2.",
        ),
    )
    .unwrap();
    let changed_source = project(root, "reviewer");
    assert_eq!(changed_source["obligations"][0]["status"], "source_invalid");
    assert_ne!(changed_source["prompt_digest"], initial["prompt_digest"]);
    assert!(
        !changed_source["prompt"]
            .as_str()
            .unwrap()
            .contains("CAS failure")
    );
    assert!(
        changed_source["prompt"]
            .as_str()
            .unwrap()
            .contains("dominating null checks")
    );

    std::fs::write(root.join(DOCUMENT), CARDS).unwrap();
    value["agent_obligations"][0]["must"][1] = json!("A changed concurrency interpretation");
    write_config(root, &value);
    let changed_instruction = project(root, "reviewer");
    assert_eq!(
        changed_instruction["obligations"][0]["status"],
        "stale_adoption"
    );
    assert!(
        !changed_instruction["prompt"]
            .as_str()
            .unwrap()
            .contains("changed concurrency")
    );
    assert!(changed_instruction["gaps"].as_array().unwrap().is_empty());
    assert!(
        !changed_instruction["obligations"][0]["reasons"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn relocating_a_selected_card_uses_cli_rebinding_without_adopting_other_cards() {
    let root = fixture();
    let root = root.path();
    let relative = "qualitygate/review/review-pack.md";
    std::fs::create_dir_all(root.join("qualitygate/review")).unwrap();
    let card = CARDS
        .split_once("## Concurrency and atomicity\n")
        .unwrap()
        .1
        .split_once("## Pointers and lifetime\n")
        .unwrap()
        .0;
    std::fs::write(
        root.join(relative),
        format!("## Concurrency and atomicity\n{card}"),
    )
    .unwrap();
    let source = report(
        &cli(
            root,
            &[
                "rules",
                "source",
                "--document",
                relative,
                "--section",
                "Concurrency and atomicity",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let mut value = config();
    value["agent_obligations"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    value["agent_obligations"][0]["source"] = source["source"].clone();
    write_config(root, &value);
    assert_eq!(
        project(root, "reviewer")["obligations"][0]["status"],
        "unadopted"
    );
    adopt_for_fixture(root, &mut value);
    let projected = project(root, "reviewer");
    assert_eq!(projected["obligations"].as_array().unwrap().len(), 1);
    assert_eq!(projected["obligations"][0]["status"], "ready");
    assert_eq!(
        projected["obligations"][0]["obligation"]["source"]["document"],
        relative
    );
    assert!(
        projected["prompt"]
            .as_str()
            .unwrap()
            .contains("CAS failure")
    );
    assert!(
        !projected["prompt"]
            .as_str()
            .unwrap()
            .contains("dominating null checks")
    );
}
