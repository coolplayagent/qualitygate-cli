mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Copy)]
struct Pack {
    document: &'static str,
    cards: &'static str,
    example: &'static str,
    ids: &'static [&'static str],
}

const PACKS: [Pack; 2] = [
    Pack {
        document: "skills/qualitygate-cli/references/review-pack.md",
        cards: include_str!("../skills/qualitygate-cli/references/review-pack.md"),
        example: include_str!(
            "../skills/qualitygate-cli/references/examples/review-obligations.yaml"
        ),
        ids: &[
            "review-concurrency",
            "review-lifetime",
            "review-bounds",
            "review-consistency",
            "review-test-quality",
            "review-workflow",
        ],
    },
    Pack {
        document: "skills/qualitygate-cli/references/design-review.md",
        cards: include_str!("../skills/qualitygate-cli/references/design-review.md"),
        example: include_str!(
            "../skills/qualitygate-cli/references/examples/design-review-obligations.yaml"
        ),
        ids: &[
            "design-responsibility",
            "design-knowledge",
            "design-behavior",
            "design-evolution",
        ],
    },
];

fn config(pack: Pack) -> Value {
    let example: Value = serde_norway::from_str(pack.example).unwrap();
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

fn setup(pack: Pack) -> tempfile::TempDir {
    let root = fixture();
    let document = root.path().join(pack.document);
    std::fs::create_dir_all(document.parent().unwrap()).unwrap();
    std::fs::write(document, pack.cards).unwrap();
    write_config(root.path(), &config(pack));
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

fn source(root: &Path, document: &str, section: &str) -> Value {
    report(
        &cli(
            root,
            &[
                "rules",
                "source",
                "--document",
                document,
                "--section",
                section,
                "--format",
                "json",
            ],
        ),
        0,
    )["source"]
        .clone()
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
    for pack in PACKS {
        let root = setup(pack);
        let schema = report(&cli(root.path(), &["schema", "agent-obligation"]), 0);
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        let value = config(pack);
        let obligations = value["agent_obligations"].as_array().unwrap();
        assert_eq!(obligations.len(), pack.ids.len());
        for (obligation, id) in obligations.iter().zip(pack.ids) {
            assert!(validator.is_valid(obligation), "{obligation}");
            assert_eq!(obligation["id"], *id);
            assert_eq!(obligation["version"], 1);
            assert_eq!(obligation["required"], false);
            assert!(obligation.get("adoption").is_none());
            assert_eq!(obligation["enforcement"], json!({"kind":"instruction"}));
            assert_eq!(
                obligation["scope"],
                json!({"roles":["implementer", "reviewer"],"tasks":[]})
            );
            assert_eq!(
                obligation["triggers"],
                json!(["start", "after_change", "before_delivery"])
            );
            assert_eq!(
                obligation["source"],
                source(
                    root.path(),
                    pack.document,
                    obligation["source"]["section"].as_str().unwrap()
                )
            );
        }
    }
}

#[test]
fn shipped_attributes_preserve_review_source_bindings_under_autocrlf_checkout() {
    for pack in PACKS {
        let root = setup(pack);
        let root = root.path();
        std::fs::write(
            root.join(".gitattributes"),
            include_str!("../.gitattributes"),
        )
        .unwrap();
        std::fs::write(root.join("autocrlf-control.txt"), "control line\n").unwrap();
        git(
            root,
            &[
                "add",
                ".gitattributes",
                pack.document,
                "autocrlf-control.txt",
            ],
        );
        git(root, &["commit", "-qm", "record packaged review source"]);
        git(root, &["config", "core.autocrlf", "true"]);
        std::fs::remove_file(root.join(pack.document)).unwrap();
        std::fs::remove_file(root.join("autocrlf-control.txt")).unwrap();
        git(
            root,
            &["checkout", "--", pack.document, "autocrlf-control.txt"],
        );

        // The control proves checkout conversion occurred; the bound source must
        // retain its exact packaged bytes because the shipped attributes override it.
        assert_eq!(
            std::fs::read(root.join("autocrlf-control.txt")).unwrap(),
            b"control line\r\n"
        );
        assert_eq!(
            std::fs::read(root.join(pack.document)).unwrap(),
            pack.cards.as_bytes()
        );
        for obligation in config(pack)["agent_obligations"].as_array().unwrap() {
            assert_eq!(
                source(
                    root,
                    pack.document,
                    obligation["source"]["section"].as_str().unwrap()
                ),
                obligation["source"]
            );
        }
    }
}

#[test]
fn real_pack_requires_adoption_and_projects_actionable_instructions_for_selected_roles() {
    for pack in PACKS {
        let root = setup(pack);
        let root = root.path();
        let pending = project(root, "reviewer");
        assert_eq!(pending["ready"], true);
        assert_eq!(
            pending["obligations"].as_array().unwrap().len(),
            pack.ids.len()
        );
        for bound in pending["obligations"].as_array().unwrap() {
            assert_eq!(bound["status"], "unadopted");
            assert!(!bound["reasons"].as_array().unwrap().is_empty());
            assert!(
                !pending["prompt"]
                    .as_str()
                    .unwrap()
                    .contains(bound["obligation"]["must"][1].as_str().unwrap())
            );
        }
        assert!(pending["gaps"].as_array().unwrap().is_empty());

        let mut value = config(pack);
        adopt_for_fixture(root, &mut value);
        for role in ["reviewer", "implementer"] {
            let adopted = project(root, role);
            assert_eq!(adopted["ready"], true);
            assert_eq!(
                adopted["obligations"].as_array().unwrap().len(),
                pack.ids.len()
            );
            for bound in adopted["obligations"].as_array().unwrap() {
                assert_eq!(bound["status"], "ready");
                // Verify actual instruction delivery without locking the cards' prose.
                for field in ["must", "must_not"] {
                    for instruction in bound["obligation"][field].as_array().unwrap() {
                        assert!(
                            adopted["prompt"]
                                .as_str()
                                .unwrap()
                                .contains(instruction.as_str().unwrap())
                        );
                    }
                }
            }
        }
        let other_role = project(root, "observer");
        assert_eq!(other_role["obligations"], json!([]));
        for obligation in value["agent_obligations"].as_array().unwrap() {
            assert!(
                !other_role["prompt"]
                    .as_str()
                    .unwrap()
                    .contains(obligation["must"][1].as_str().unwrap())
            );
        }
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
                .all(|check| { !pack.ids.contains(&check["id"].as_str().unwrap()) })
        );
    }
}

fn assert_excluded_card(projection: &Value, value: &Value, id: &str, status: &str) {
    assert_eq!(projection["ready"], true);
    assert!(projection["gaps"].as_array().unwrap().is_empty());
    for bound in projection["obligations"].as_array().unwrap() {
        let affected = bound["obligation"]["id"] == id;
        assert_eq!(bound["status"], if affected { status } else { "ready" });
        if affected {
            assert!(!bound["reasons"].as_array().unwrap().is_empty());
        }
    }
    for obligation in value["agent_obligations"].as_array().unwrap() {
        assert_eq!(
            projection["prompt"]
                .as_str()
                .unwrap()
                .contains(obligation["must"][1].as_str().unwrap()),
            obligation["id"] != id
        );
    }
}

#[test]
fn changing_a_card_or_its_interpretation_excludes_stale_optional_instructions() {
    for pack in PACKS {
        let root = setup(pack);
        let root = root.path();
        let mut value = config(pack);
        adopt_for_fixture(root, &mut value);
        let initial = project(root, "reviewer");
        for (index, id) in pack.ids.iter().enumerate() {
            let marker = format!("Card `{id}`, version 1.");
            assert_eq!(pack.cards.matches(&marker).count(), 1);
            std::fs::write(
                root.join(pack.document),
                pack.cards
                    .replace(&marker, &format!("Card `{id}`, version 2.")),
            )
            .unwrap();
            let changed_source = project(root, "reviewer");
            assert_excluded_card(&changed_source, &value, id, "source_invalid");
            assert_ne!(changed_source["prompt_digest"], initial["prompt_digest"]);

            std::fs::write(root.join(pack.document), pack.cards).unwrap();
            let mut changed_value = value.clone();
            changed_value["agent_obligations"][index]["must"][1] =
                json!("A changed review interpretation requiring separate adoption");
            write_config(root, &changed_value);
            let changed_instruction = project(root, "reviewer");
            assert_excluded_card(&changed_instruction, &value, id, "stale_adoption");
            assert!(
                !changed_instruction["prompt"]
                    .as_str()
                    .unwrap()
                    .contains("changed review interpretation")
            );
            write_config(root, &value);
        }
    }
}

#[test]
fn widening_role_scope_or_changing_triggers_requires_fresh_adoption() {
    for pack in PACKS {
        let root = setup(pack);
        let root = root.path();
        let mut value = config(pack);
        adopt_for_fixture(root, &mut value);
        let mut widened = value.clone();
        widened["agent_obligations"][0]["scope"]["roles"] =
            json!(["implementer", "reviewer", "observer"]);
        write_config(root, &widened);
        assert_excluded_card(
            &project(root, "reviewer"),
            &value,
            pack.ids[0],
            "stale_adoption",
        );
        let observer = project(root, "observer");
        assert_eq!(observer["obligations"].as_array().unwrap().len(), 1);
        assert_eq!(observer["obligations"][0]["status"], "stale_adoption");
        assert!(
            !observer["prompt"]
                .as_str()
                .unwrap()
                .contains(value["agent_obligations"][0]["must"][1].as_str().unwrap())
        );

        let mut changed_triggers = value.clone();
        changed_triggers["agent_obligations"][0]["triggers"] = json!(["start"]);
        write_config(root, &changed_triggers);
        assert_excluded_card(
            &project(root, "reviewer"),
            &value,
            pack.ids[0],
            "stale_adoption",
        );
    }
}

#[test]
fn relocating_a_selected_card_uses_cli_rebinding_without_adopting_other_cards() {
    for pack in PACKS {
        let root = fixture();
        let root = root.path();
        let relative = "qualitygate/review/selected-card.md";
        std::fs::create_dir_all(root.join("qualitygate/review")).unwrap();
        let mut value = config(pack);
        let section = value["agent_obligations"][0]["source"]["section"]
            .as_str()
            .unwrap();
        let heading = format!("## {section}\n");
        let card = pack
            .cards
            .split_once(&heading)
            .unwrap()
            .1
            .split_once("\n## ")
            .unwrap()
            .0;
        std::fs::write(root.join(relative), format!("{heading}{card}\n")).unwrap();
        let binding = source(root, relative, section);
        value["agent_obligations"]
            .as_array_mut()
            .unwrap()
            .truncate(1);
        value["agent_obligations"][0]["source"] = binding;
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
        let original = config(pack);
        for (index, obligation) in original["agent_obligations"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                projected["prompt"]
                    .as_str()
                    .unwrap()
                    .contains(obligation["must"][1].as_str().unwrap()),
                index == 0
            );
        }
    }
}
