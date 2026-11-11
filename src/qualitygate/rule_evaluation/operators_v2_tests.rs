use super::*;
use serde_json::{Value, json};

fn expr(value: Value) -> Expr {
    serde_json::from_value(value).unwrap()
}
fn execute(value: Value, entities: &[Entity<'_>]) -> Result<Outcome> {
    evaluate(
        &expr(value),
        Facts {
            entities,
            complete: true,
        },
    )
}
fn entity(name: &str) -> Entity<'_> {
    Entity {
        name,
        text: "body",
        path: Some("tests.rs"),
        range: None,
    }
}

#[test]
fn bounded_lambda_composition_filters_counts_and_checks_typed_entities() {
    let facts = [entity("test_one"), entity("helper"), entity("test_two")];
    let selected = json!({"op":"filter","input":{"op":"entities"},"predicate":{
        "op":"matches","value":{"op":"field","name":"name"},"pattern":"^test_"}});
    let value = json!({"op":"and","args":[
        {"op":"compare","left":{"op":"count","input":selected},"relation":"eq","right":{"op":"number","value":2}},
        {"op":"all","input":selected,"predicate":{"op":"in","value":{"op":"field","name":"path"},"set":[{"type":"string","value":"tests.rs"}]}},
        {"op":"any","input":{"op":"entities"},"predicate":{"op":"compare","left":{"op":"field","name":"text"},"relation":"eq","right":{"op":"string","value":"body"}}}
    ]});
    let outcome = execute(value, &facts).unwrap();
    assert!(outcome.passed);
    assert_eq!(outcome.entities, 3);
    assert!(outcome.operations > 10 && outcome.scanned_bytes > 0);
    for (relation, expected) in [
        ("eq", false),
        ("ne", true),
        ("lt", true),
        ("le", true),
        ("gt", false),
        ("ge", false),
    ] {
        assert_eq!(execute(json!({"op":"compare","relation":relation,"left":{"op":"number","value":1},"right":{"op":"number","value":2}}),&[]).unwrap().passed,expected);
    }
    for (value, set) in [
        (
            json!({"op":"bool","value":true}),
            json!([{"type":"bool","value":false},{"type":"bool","value":true}]),
        ),
        (
            json!({"op":"number","value":42}),
            json!([{"type":"number","value":42}]),
        ),
    ] {
        assert!(
            execute(json!({"op":"in","value":value,"set":set}), &[])
                .unwrap()
                .passed
        );
    }
    assert!(!execute(json!({"op":"not","arg":{"op":"or","args":[{"op":"bool","value":false},{"op":"bool","value":true}]}}),&[]).unwrap().passed);
}

#[test]
fn complete_empty_sets_have_defined_semantics_but_incomplete_facts_never_pass() {
    for (operator, expected) in [("all", true), ("any", false)] {
        assert_eq!(execute(json!({"op":operator,"input":{"op":"entities"},"predicate":{"op":"bool","value":true}}),&[]).unwrap().passed,expected);
    }
    let count = expr(
        json!({"op":"compare","left":{"op":"count","input":{"op":"entities"}},"relation":"gt","right":{"op":"number","value":0}}),
    );
    assert!(
        !evaluate(
            &count,
            Facts {
                entities: &[],
                complete: true
            }
        )
        .unwrap()
        .passed
    );
    assert!(
        evaluate(
            &Expr::Bool { value: true },
            Facts {
                entities: &[],
                complete: false
            }
        )
        .is_err()
    );
    let missing_path = [Entity {
        name: "a",
        text: "body",
        path: None,
        range: None,
    }];
    let unused = json!({"op":"or","args":[{"op":"bool","value":true},
        {"op":"all","input":{"op":"entities"},"predicate":{"op":"matches","value":{"op":"field","name":"path"},"pattern":".*"}}]});
    assert!(
        execute(unused, &missing_path)
            .unwrap_err()
            .to_string()
            .contains("path fact")
    );
}

#[test]
fn one_budget_covers_nested_quantifiers_boolean_branches_regex_scans_and_deadlines() {
    let many: Vec<_> = (0..300).map(|_| entity("a")).collect();
    let expensive = json!({"op":"all","input":{"op":"entities"},"predicate":{
        "op":"all","input":{"op":"entities"},"predicate":{"op":"bool","value":true}}});
    for operator in ["and", "or"] {
        let value = json!({"op":operator,"args":[{"op":"bool","value":operator=="or"},expensive]});
        assert!(
            execute(value, &many)
                .unwrap_err()
                .to_string()
                .contains("operations")
        );
    }
    let large = "x".repeat(MAX_SCAN_BYTES / 2 + 1);
    let facts = [Entity {
        name: "",
        text: &large,
        path: None,
        range: None,
    }];
    let scan = json!({"op":"matches","value":{"op":"field","name":"text"},"pattern":""});
    let twice =
        json!({"op":"all","input":{"op":"entities"},"predicate":{"op":"and","args":[scan,scan]}});
    assert!(
        execute(twice, &facts)
            .unwrap_err()
            .to_string()
            .contains("scanned bytes")
    );
    let started = Instant::now() - Duration::from_secs(MAX_SECONDS + 1);
    assert!(
        evaluate_since(
            &Expr::Bool { value: true },
            Facts {
                entities: &[],
                complete: true
            },
            started
        )
        .unwrap_err()
        .to_string()
        .contains("deadline")
    );
    let too_many: Vec<_> = (0..=MAX_ENTITIES).map(|_| entity("")).collect();
    assert!(execute(json!({"op":"bool","value":true}), &too_many).is_err());
    let too_large = [Entity {
        name: &large,
        text: &large,
        path: None,
        range: None,
    }];
    assert!(
        execute(json!({"op":"bool","value":true}), &too_large)
            .unwrap_err()
            .to_string()
            .contains("facts exceed")
    );
}

#[test]
fn bounded_observations_preserve_locations_without_claiming_each_false_branch_is_a_violation() {
    let long = "界".repeat(1100);
    let range = crate::domain::Range {
        start_line: 7,
        end_line: 9,
    };
    let facts: Vec<_> = (0..100)
        .map(|_| Entity {
            name: &long,
            text: "",
            path: Some(&long),
            range: Some(&range),
        })
        .collect();
    let outcome = execute(
        json!({"op":"all","input":{"op":"entities"},"predicate":{"op":"bool","value":false}}),
        &facts,
    )
    .unwrap();
    assert!(!outcome.passed);
    assert_eq!(outcome.false_observations.len(), 64);
    assert_eq!(outcome.omitted_observations, 37);
    let first = &outcome.false_observations[0];
    assert_eq!(first.expression_path, "/expression/predicate");
    assert!(first.name_truncated && first.file_truncated);
    assert_eq!(first.range.as_ref().unwrap().start_line, 7);
    assert_eq!(first.name.as_ref().unwrap().chars().count(), 256);
    assert_eq!(first.file.as_ref().unwrap().chars().count(), 1024);
    let or = execute(
        json!({"op":"or","args":[{"op":"bool","value":false},{"op":"bool","value":true}]}),
        &[],
    )
    .unwrap();
    assert!(or.passed);
    assert_eq!(or.false_observations.len(), 1);
}
