use super::*;
use serde_json::{Value as Json, json};
fn facts<'a>(source: &'a [&'a str], tests: &'a [&'a str]) -> BTreeMap<String, Collection<'a>> {
    [("entities", source), ("tests", tests)]
        .into_iter()
        .map(|(name, paths)| {
            (
                name.into(),
                Collection {
                    complete: true,
                    entities: paths
                        .iter()
                        .map(|path| Entity {
                            name: path,
                            text: path,
                            path: Some(path),
                            range: None,
                        })
                        .collect(),
                },
            )
        })
        .collect()
}
fn execute(value: Json, facts: &BTreeMap<String, Collection<'_>>) -> Result<Outcome> {
    evaluate(&serde_json::from_value(value)?, facts)
}
fn paths(source: &str) -> Json {
    json!({"op":"map","input":if source=="entities" {json!({"op":"entities"})} else {json!({"op":"source","name":source})},"as":"file","body":{"op":"field","name":"path"}})
}
#[test]
fn projection_and_outer_capture_agree_on_correspondence_and_preserve_locations() {
    let expected = json!({"op":"replace","value":{"op":"replace","value":{"op":"field","name":"path","of":{"op":"var","name":"source"}},"from":"src/","to":"tests/"},"from":".rs","to":"_test.rs"});
    let mapped = json!({"op":"map","input":{"op":"entities"},"as":"source","body":expected});
    let subset =
        json!({"op":"subset","left":{"op":"distinct","input":mapped},"right":paths("tests")});
    let nested = json!({"op":"all","input":{"op":"entities"},"as":"source","predicate":{"op":"any","input":{"op":"source","name":"tests"},"as":"test","predicate":{"op":"compare","left":expected,"relation":"eq","right":{"op":"field","name":"path","of":{"op":"var","name":"test"}}}}});
    for (files, pass) in [
        (vec!["tests/a_test.rs", "tests/sub/a_test.rs"], true),
        (vec!["tests/a_test.rs"], false),
        (vec![], false),
    ] {
        let inputs = facts(&["src/a.rs", "src/sub/a.rs"], &files);
        assert_eq!(execute(subset.clone(), &inputs).unwrap().passed, pass);
        let result = execute(nested.clone(), &inputs).unwrap();
        assert_eq!(result.passed, pass);
        if !files.is_empty() {
            let located = result
                .false_observations
                .iter()
                .find(|o| o.bindings.len() == 2)
                .unwrap();
            assert_eq!(located.bindings["source"].source, "entities");
            assert_eq!(located.bindings["test"].source, "tests");
        }
    }
    assert!(execute(subset, &facts(&[], &[])).unwrap().passed);
}
#[test]
fn scalar_collections_compose_strings_booleans_counts_filters_and_sets() {
    let inputs = facts(&["甲", "乙", "甲"], &[]);
    let unique = json!({"op":"distinct","input":paths("entities")});
    let filtered = json!({"op":"filter","input":unique,"as":"value","predicate":{"op":"matches","value":{"op":"var","name":"value"},"pattern":"甲"}});
    let expr = json!({"op":"and","args":[
        {"op":"compare","left":{"op":"count","input":unique},"relation":"eq","right":{"op":"number","value":2}},
        {"op":"contains","input":filtered,"value":{"op":"string","value":"甲"}},
        {"op":"all","input":filtered,"as":"s","predicate":{"op":"in","value":{"op":"var","name":"s"},"set":[{"type":"string","value":"甲"}]}},
        {"op":"compare","left":{"op":"replace","value":{"op":"concat","args":[{"op":"string","value":"界界"},{"op":"string","value":"x"}]},"from":"界","to":"世"},"relation":"eq","right":{"op":"string","value":"世世x"}},
        {"op":"not","arg":{"op":"or","args":[{"op":"bool","value":false},{"op":"bool","value":false}]}}
    ]});
    let result = execute(expr, &inputs).unwrap();
    assert!(result.passed && result.materialized_bytes > 0 && result.scanned_bytes > 0);
    for (body, value) in [
        (
            json!({"op":"number","value":3}),
            json!({"op":"number","value":3}),
        ),
        (
            json!({"op":"bool","value":false}),
            json!({"op":"bool","value":false}),
        ),
    ] {
        assert!(execute(json!({"op":"contains","input":{"op":"distinct","input":{"op":"map","input":{"op":"entities"},"as":"e","body":body}},"value":value}), &inputs).unwrap().passed);
    }
    for (relation, pass) in [
        ("eq", false),
        ("ne", true),
        ("lt", true),
        ("le", true),
        ("gt", false),
        ("ge", false),
    ] {
        assert_eq!(execute(json!({"op":"compare","left":{"op":"number","value":1},"relation":relation,"right":{"op":"number","value":2}}),&inputs).unwrap().passed,pass);
    }
}
#[test]
fn complete_empty_is_distinct_from_missing_facts_and_unavailable_paths() {
    let mut inputs = facts(&[], &[]);
    for (op, pass) in [("all", true), ("any", false)] {
        assert_eq!(
            execute(
                json!({"op":op,"input":{"op":"entities"},"predicate":{"op":"bool","value":true}}),
                &inputs
            )
            .unwrap()
            .passed,
            pass
        );
    }
    inputs.get_mut("tests").unwrap().complete = false;
    assert!(
        execute(json!({"op":"bool","value":true}), &inputs)
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
    inputs = facts(&["a"], &["b"]);
    inputs.get_mut("entities").unwrap().entities[0].path = None;
    assert!(execute(json!({"op":"all","input":{"op":"source","name":"tests"},"predicate":{"op":"matches","value":{"op":"field","name":"path"},"pattern":".*"}}),&inputs).unwrap().passed);
    assert!(execute(json!({"op":"or","args":[{"op":"bool","value":true},{"op":"contains","input":paths("entities"),"value":{"op":"string","value":"a"}}]}),&inputs).unwrap_err().to_string().contains("path fact"));
    inputs.remove("entities");
    assert!(execute(json!({"op":"bool","value":true}), &inputs).is_err());
}
#[test]
fn global_budgets_bound_cartesian_work_allocations_scans_and_observations() {
    let names = vec!["x"; 300];
    let inputs = facts(&names, &[]);
    let costly = json!({"op":"all","input":{"op":"entities"},"predicate":{"op":"all","input":{"op":"entities"},"predicate":{"op":"bool","value":true}}});
    assert!(
        execute(
            json!({"op":"or","args":[{"op":"bool","value":true},costly]}),
            &inputs
        )
        .unwrap_err()
        .to_string()
        .contains("operations")
    );
    let result=execute(json!({"op":"all","input":{"op":"entities"},"as":"e","predicate":{"op":"bool","value":false}}),&inputs).unwrap();
    assert_eq!(result.false_observations.len(), 64);
    assert!(result.omitted_observations > 0);
    let repeat = json!({"op":"replace","value":{"op":"field","name":"text"},"from":"x","to":"x".repeat(4096)});
    let large = "x".repeat(9000);
    let paths = [large.as_str()];
    let inputs = facts(&paths, &[]);
    assert!(execute(json!({"op":"all","input":{"op":"entities"},"predicate":{"op":"matches","value":repeat,"pattern":".*"}}),&inputs).unwrap_err().to_string().contains("materialized"));
    let started = Instant::now() - Duration::from_secs(31);
    assert!(
        evaluate_since(&Expr::Bool { value: true }, &facts(&[], &[]), started)
            .unwrap_err()
            .to_string()
            .contains("deadline")
    );
    let too_many = vec![""; MAX_ENTITIES + 1];
    assert!(execute(json!({"op":"bool","value":true}), &facts(&too_many, &[])).is_err());
    let large = "x".repeat(MAX_FACT_BYTES / 2);
    let paths = [large.as_str()];
    assert!(execute(json!({"op":"bool","value":true}), &facts(&paths, &[])).is_err());
}
