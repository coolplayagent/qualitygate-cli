use super::*;
use crate::rule_catalog::custom_validation;
use serde_json::{Value, json};

fn source() -> Value {
    json!({"document":"AGENTS.md","section":"Verification","content_hash":crate::config::policy_store::digest(b"instructions")})
}
fn obligation() -> Value {
    json!({"schema_version":1,"id":"obligation","version":1,"source":source(),"required":true,
    "scope":{"roles":[],"tasks":[]},"triggers":["start"],"must":["Verify behavior"],"must_not":[],"enforcement":{"kind":"checks","checks":["line-ending"]}})
}
fn clause() -> Value {
    json!({"schema_version":1,"id":"clause","version":1,"source":source(),"required":true,"statement":"Require LF",
    "realization":{"kind":"checks","mechanism":"regex","checks":["line-ending"],"positive_examples":["lf"],"negative_examples":["crlf"]}})
}
fn config() -> Config {
    parse(b"schema_version: 1\nrules: {line-ending: {enabled: true}}\n").unwrap()
}

#[test]
fn agent_instructions_require_valid_scope_triggers_text_and_real_check_bindings() {
    let mut config = config();
    config.agent_obligations = vec![serde_json::from_value(obligation()).unwrap()];
    agent_contract::validate(&config).unwrap();
    for (pointer, replacement) in [
        ("/schema_version", json!(2)),
        ("/version", json!(0)),
        ("/source/document", json!("./AGENTS.md")),
        ("/source/section", json!("bad\nsection")),
        ("/scope/roles", json!(["implementer", "implementer"])),
        ("/scope/tasks", json!(vec!["task"; 33])),
        ("/triggers", json!([])),
        ("/triggers", json!(["start", "start"])),
        ("/must", json!([])),
        ("/must", json!([" "])),
        ("/must", json!(["same", " same "])),
        ("/must", json!(["a".repeat(4097)])),
        ("/must_not", json!(vec!["do not"; 17])),
        ("/enforcement/checks", json!([])),
        ("/enforcement/checks", json!(["unknown"])),
        ("/enforcement/checks", json!(["line-ending", "line-ending"])),
    ] {
        let mut changed = obligation();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        config.agent_obligations = vec![serde_json::from_value(changed).unwrap()];
        assert!(agent_contract::validate(&config).is_err(), "{pointer}");
    }
    config.agent_obligations = vec![serde_json::from_value(obligation()).unwrap(); 2];
    assert!(agent_contract::validate(&config).is_err());
    config.agent_obligations = vec![serde_json::from_value(obligation()).unwrap(); 129];
    assert!(agent_contract::validate(&config).is_err());
}

#[test]
fn constraint_inventory_refuses_missing_examples_self_reference_and_unsupported_claims_without_owner()
 {
    let mut config = config();
    config.constraints = vec![serde_json::from_value(clause()).unwrap()];
    constraint_inventory::validate(&config).unwrap();
    for (pointer, replacement) in [
        ("/schema_version", json!(2)),
        ("/version", json!(0)),
        ("/statement", json!("")),
        ("/statement", json!("a\nb")),
        ("/source/document", json!("./AGENTS.md")),
        ("/source/section", json!("x".repeat(1025))),
        ("/realization/checks", json!([])),
        ("/realization/checks", json!(["unknown"])),
        ("/realization/checks", json!(["constraint-coverage"])),
        ("/realization/checks", json!(["line-ending", "line-ending"])),
        ("/realization/positive_examples", json!([])),
        ("/realization/positive_examples", json!(["same", "same"])),
        ("/realization/negative_examples", json!(["x".repeat(1025)])),
        ("/realization/negative_examples", json!(vec!["case"; 17])),
    ] {
        let mut changed = clause();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        config.constraints = vec![serde_json::from_value(changed).unwrap()];
        assert!(
            constraint_inventory::validate(&config).is_err(),
            "{pointer}"
        );
    }
    for (reason, owner, valid) in [
        ("manual observation", "reviewer", true),
        ("", "reviewer", false),
        ("manual observation", "", false),
    ] {
        let mut changed = clause();
        changed["realization"] =
            json!({"kind":"unsupported","reason":reason,"evidence_owner":owner});
        config.constraints = vec![serde_json::from_value(changed).unwrap()];
        assert_eq!(constraint_inventory::validate(&config).is_ok(), valid);
    }
    config.constraints = vec![serde_json::from_value(clause()).unwrap(); 257];
    assert!(constraint_inventory::validate(&config).is_err());
    config.constraints.truncate(2);
    assert!(constraint_inventory::validate(&config).is_err());
}

#[test]
fn adoption_binding_excludes_review_signature_but_changes_with_enforced_check_definition() {
    let mut config = config();
    let catalog = catalog::Catalog::load(&config, []).unwrap();
    let mut obligation: crate::domain::agent_contract::Obligation =
        serde_json::from_value(obligation()).unwrap();
    let mut constraint: crate::domain::constraint::Constraint =
        serde_json::from_value(clause()).unwrap();
    let agent_before = agent_contract::binding(&obligation, &config, &catalog).unwrap();
    let constraint_before = constraint_inventory::binding(&constraint, &config, &catalog).unwrap();
    let review = |binding: String| {
        serde_json::from_value(
            json!({"id":"review","reviewer":"reviewer","reference":"review-1",
        "resolution":"initial_mapping","rationale":"Matches source","binding_digest":binding}),
        )
        .unwrap()
    };
    obligation.adoption = Some(review(agent_before.clone()));
    constraint.adoption = Some(review(constraint_before.clone()));
    assert_eq!(
        agent_contract::binding(&obligation, &config, &catalog).unwrap(),
        agent_before
    );
    assert_eq!(
        constraint_inventory::binding(&constraint, &config, &catalog).unwrap(),
        constraint_before
    );
    config.rules.get_mut("line-ending").unwrap().severity = crate::domain::Severity::Warning;
    assert_ne!(
        agent_contract::binding(&obligation, &config, &catalog).unwrap(),
        agent_before
    );
    assert_ne!(
        constraint_inventory::binding(&constraint, &config, &catalog).unwrap(),
        constraint_before
    );
    config.agent_obligations = vec![obligation.clone()];
    config.constraints = vec![constraint.clone()];
    agent_contract::validate(&config).unwrap();
    constraint_inventory::validate(&config).unwrap();
    obligation.id = "second-obligation".into();
    constraint.id = "second-clause".into();
    config.agent_obligations.push(obligation);
    config.constraints.push(constraint);
    assert!(agent_contract::validate(&config).is_err());
    assert!(constraint_inventory::validate(&config).is_err());
}

fn custom() -> Value {
    json!({"id":"scenario-test","version":1,"source":source(),"language":["rust"],"requires_capabilities":["test_methods"],
    "when":{"entity":"test_method","change":"added"},"then":{"name_pattern":"^test_"},"fix":"Describe the behavior"})
}

#[test]
fn custom_assertions_require_compatible_entities_capabilities_and_marker_provenance() {
    let rule: CustomRule = serde_json::from_value(custom()).unwrap();
    custom_validation::validate(&rule).unwrap();
    for (pointer, value) in [
        ("/version", json!(0)),
        ("/fix", json!(" ")),
        ("/language", json!([""])),
        ("/requires_capabilities", json!(["unknown"])),
        (
            "/requires_capabilities",
            json!(["test_methods", "test_methods"]),
        ),
        ("/requires_capabilities", json!([])),
        ("/when/entity", json!("unknown")),
        ("/when/change", json!("all")),
        ("/then", json!({})),
        ("/then", json!({"max_lines":10})),
        ("/then", json!({"min_count":3,"max_count":2})),
        ("/then", json!({"name_pattern":"["})),
        ("/then", json!({"require_dependency":{"artifact":"junit"}})),
        ("/then", json!({"require_marker":true})),
    ] {
        let mut changed = custom();
        *changed.pointer_mut(pointer).unwrap() = value;
        let rule = serde_json::from_value(changed).unwrap();
        assert!(custom_validation::validate(&rule).is_err(), "{pointer}");
    }
    for (kind, capability) in [
        ("annotation", "annotations"),
        ("comment", "comments"),
        ("git_trailer", "commits"),
    ] {
        let mut changed = custom();
        changed["then"] = json!({"require_marker":true});
        changed["requires_capabilities"] = json!(["test_methods", capability]);
        changed["binding"] = json!({"marker":{"type":kind,"name":"Generated","fields":["author"]}});
        changed["applies_to"] = json!({"paths":["tests/**"],"provenance_scope":"all_added_tests"});
        custom_validation::validate(&serde_json::from_value(changed.clone()).unwrap()).unwrap();
        for (pointer, value) in [
            ("/binding/marker/type", json!("unknown")),
            ("/binding/marker/name", json!("")),
            ("/binding/marker/fields", json!(["bad field"])),
            ("/applies_to/provenance_scope", json!("unknown")),
            ("/requires_capabilities", json!(["test_methods"])),
        ] {
            let mut invalid = changed.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                custom_validation::validate(&serde_json::from_value(invalid).unwrap()).is_err()
            );
        }
    }
}

#[test]
fn file_inventory_contracts_reject_globs_duplicates_escape_and_noninventory_use() {
    let mut value = custom();
    value["language"] = json!([]);
    value["when"] = json!({"entity":"file","change":"all"});
    value["requires_capabilities"] = json!(["files"]);
    value["then"] = json!({"required_paths":["README.md"],"max_lines":100});
    custom_validation::validate(&serde_json::from_value(value.clone()).unwrap()).unwrap();
    for paths in [
        json!(["../outside"]),
        json!([""]),
        json!(["a", "a"]),
        json!(["*.md"]),
        json!(["./README.md"]),
        json!(vec!["README.md"; 257]),
    ] {
        let mut changed = value.clone();
        changed["then"]["required_paths"] = paths;
        assert!(custom_validation::validate(&serde_json::from_value(changed).unwrap()).is_err());
    }
    for (entity, capability, change, good) in [
        ("commit", "commits", "added", true),
        ("commit", "commits", "modified", false),
        ("comment", "comments", "any", true),
        ("comment", "comments", "modified", false),
        ("import", "imports", "added", true),
        ("import", "imports", "renamed", false),
    ] {
        let mut changed = custom();
        changed["language"] = json!([]);
        changed["requires_capabilities"] = json!([capability]);
        changed["when"] = json!({"entity":entity,"change":change});
        changed["then"] = json!({"max_count":10});
        assert_eq!(
            custom_validation::validate(&serde_json::from_value(changed).unwrap()).is_ok(),
            good
        );
    }
}
