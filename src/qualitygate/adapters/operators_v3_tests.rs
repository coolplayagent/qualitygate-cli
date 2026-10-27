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

#[test]
fn duplicate_heavy_scalar_sets_still_fit_the_existing_operation_budget() {
    let names = vec!["x"; 20_000];
    let inputs = facts(&names, &["one"]);
    for body in [
        json!({"op":"bool","value":true}),
        json!({"op":"number","value":7}),
        json!({"op":"string","value":"same"}),
    ] {
        let mapped = |tests: bool| json!({"op":"map","input":if tests {json!({"op":"source","name":"tests"})} else {json!({"op":"entities"})},"as":"item","body":body});
        for expression in [
            json!({"op":"compare","left":{"op":"count","input":{"op":"distinct","input":mapped(false)}},"relation":"eq","right":{"op":"number","value":1}}),
            json!({"op":"subset","left":mapped(false),"right":mapped(true)}),
            json!({"op":"subset","left":mapped(true),"right":mapped(false)}),
        ] {
            let outcome = execute(expression, &inputs).unwrap();
            assert!(outcome.passed);
            assert!(outcome.operations < 100_000);
        }
    }
}

#[test]
fn indexed_sets_match_exact_membership_for_unsorted_duplicates_and_unicode() {
    let mut values: Vec<_> = (0..24).map(|i| format!("item-{i}")).collect();
    values.extend(["".into(), "甲".into(), "é".into(), "e\u{301}".into()]);
    let mut cases = vec![Vec::new(), vec!["é"], vec!["e\u{301}"]];
    for seed in 0..8 {
        let mut entries: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|(index, _)| (index + seed) % 5 != 0)
            .map(|(_, value)| value.as_str())
            .collect();
        entries.reverse();
        entries.extend(entries.clone());
        cases.push(entries);
    }
    for left in &cases {
        for right in &cases {
            let unique: std::collections::BTreeSet<_> = left.iter().copied().collect();
            let expected = left.iter().all(|value| right.contains(value));
            let expression = json!({"op":"and","args":[
                {"op":"compare","left":{"op":"count","input":{"op":"distinct","input":paths("entities")}},"relation":"eq","right":{"op":"number","value":unique.len()}},
                {"op":"compare","left":{"op":"subset","left":paths("entities"),"right":paths("tests")},"relation":"eq","right":{"op":"bool","value":expected}}
            ]});
            assert!(execute(expression, &facts(left, right)).unwrap().passed);
        }
    }
}

#[test]
fn ordered_distinct_preserves_first_occurrence_order_and_binding_origins() {
    let mut values: Vec<_> = (0..24).map(|i| format!("item-{}", (i * 7) % 24)).collect();
    values.extend(values.clone().into_iter().rev());
    let names: Vec<_> = values.iter().map(String::as_str).collect();
    let expression = json!({"op":"all","input":{"op":"distinct","input":paths("entities")},"as":"path","predicate":{"op":"matches","value":{"op":"var","name":"path"},"pattern":"^never$"}});
    let result = execute(expression, &facts(&names, &[])).unwrap();
    assert!(!result.passed);
    let observed: Vec<_> = result
        .false_observations
        .iter()
        .filter_map(|observation| {
            observation.current.as_ref().map(|location| {
                assert_eq!(location.source, "entities");
                assert_eq!(
                    observation.bindings["path"].entity_index,
                    location.entity_index
                );
                location.entity_index
            })
        })
        .collect();
    assert_eq!(observed, (0..24).collect::<Vec<_>>());
}

#[test]
fn indexed_set_work_still_exhausts_shared_operation_and_scan_budgets() {
    let expression = json!({"op":"compare","left":{"op":"count","input":{"op":"distinct","input":paths("entities")}},"relation":"ge","right":{"op":"number","value":0}});
    let many: Vec<_> = (0..6000).map(|i| format!("file-{i}")).collect();
    let names: Vec<_> = many.iter().map(String::as_str).collect();
    assert!(
        execute(expression.clone(), &facts(&names, &[]))
            .unwrap_err()
            .to_string()
            .contains("operations")
    );
    let long: Vec<_> = (0..64)
        .map(|i| format!("{}{i:02}", "x".repeat(60_000)))
        .collect();
    let names: Vec<_> = long.iter().map(String::as_str).collect();
    assert!(
        execute(expression, &facts(&names, &[]))
            .unwrap_err()
            .to_string()
            .contains("scanned bytes")
    );
}

#[test]
fn empty_subsets_evaluate_both_operands_without_building_an_unused_index() {
    let values: Vec<_> = (0..20_000).map(|i| format!("file-{i}")).collect();
    let names: Vec<_> = values.iter().map(String::as_str).collect();
    let inputs = facts(&[], &names);
    assert!(
        execute(
            json!({"op":"subset","left":paths("entities"),"right":paths("tests")}),
            &inputs,
        )
        .unwrap()
        .passed
    );
    let costly = json!({"op":"map","input":{"op":"source","name":"tests"},"as":"entry","body":{"op":"concat","args":vec![json!({"op":"string","value":"x"});6]}});
    assert!(
        execute(
            json!({"op":"subset","left":paths("entities"),"right":costly}),
            &inputs
        )
        .unwrap_err()
        .to_string()
        .contains("operations")
    );
}

#[test]
fn few_subset_lookups_do_not_sort_large_unique_right_operands() {
    for (right_count, left_count, complete) in [
        (20_000, 1, true),
        (18_000, 2, true),
        (4500, 17, true),
        (20_000, 2, false),
    ] {
        let values: Vec<_> = (0..right_count).map(|i| format!("file-{i}")).collect();
        let right: Vec<_> = values.iter().map(String::as_str).collect();
        let mut left = right[right_count - left_count..].to_vec();
        for expected in [true, false] {
            if !expected {
                left[0] = "absent";
            }
            let outcome = execute(
                json!({"op":"subset","left":paths("entities"),"right":paths("tests")}),
                &facts(&left, &right),
            );
            if complete {
                let outcome = outcome.unwrap_or_else(|error| {
                    panic!("subset with {left_count} left and {right_count} right values: {error}")
                });
                assert_eq!(outcome.passed, expected);
                assert!(outcome.operations < 100_000);
            } else {
                assert!(outcome.unwrap_err().to_string().contains("operations"));
            }
        }
    }
}
