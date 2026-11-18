use super::*;
use crate::rule_catalog::operator_v4::{Dimension, Field, SourceType};
use crate::{
    domain::operator::MAX_FACT_BYTES, rule_catalog::operator_v4::Scalar,
    tool_evidence::decimal::Decimal as ExactDecimal,
};
use serde_json::{Value, json};

fn expression(value: Value) -> Expr {
    serde_json::from_value(value).unwrap()
}
fn collections() -> BTreeMap<String, Collection> {
    let dimension = Dimension {
        schema: crate::rule_catalog::operator_v4::FactSchema::FunctionMetric,
        unit: "score".into(),
        measurement: "observed-v1".into(),
    };
    let metric = |value: &str| Record {
        fields: BTreeMap::from([
            (
                Field::Value,
                Scalar::Decimal(ExactDecimal::parse(value).unwrap()),
            ),
            (Field::Unit, Scalar::String("score".into())),
            (Field::Measurement, Scalar::String("observed-v1".into())),
            (Field::Path, Scalar::String("src/lib.rs".into())),
        ]),
        locations: vec![crate::tool_evidence::contracts::Location {
            file: "src/lib.rs".into(),
            start_line: 2,
            end_line: 4,
            symbol: "calculate".into(),
        }],
    };
    BTreeMap::from([
        (
            "entities".into(),
            Collection {
                records: vec![
                    Record::syntax("a", "text-a", Some("a.rs"), None),
                    Record::syntax("b", "text-b", Some("b.rs"), None),
                ],
                complete: true,
                source_type: SourceType::Syntax,
            },
        ),
        (
            "metrics".into(),
            Collection {
                records: vec![metric("0.81"), metric("0.8")],
                complete: true,
                source_type: SourceType::Metric(dimension),
            },
        ),
    ])
}
fn metric_assertion(limit: &str) -> Expr {
    expression(
        json!({"op":"all","input":{"op":"source","name":"metrics"},"as":"m","predicate":{"op":"compare","left":{"op":"field","name":"value","of":{"op":"var","name":"m"}},"relation":"le","right":{"op":"decimal","value":ExactDecimal::parse(limit).unwrap()}}}),
    )
}
#[test]
fn exact_thresholds_keep_false_observation_locations_and_bound_source_metadata() {
    let inputs = collections();
    assert!(evaluate(&metric_assertion("0.81"), &inputs).unwrap().passed);
    let outcome = evaluate(&metric_assertion("0.809999999999999999"), &inputs).unwrap();
    assert!(!outcome.passed);
    assert_eq!(outcome.sources["metrics"], 2);
    let observation = outcome
        .false_observations
        .iter()
        .find(|observation| observation.bindings.contains_key("m"))
        .unwrap();
    assert_eq!(
        observation.bindings["m"].file.as_deref(),
        Some("src/lib.rs")
    );
    assert_eq!(observation.bindings["m"].related.len(), 1);
    let mut bad = collections();
    bad.get_mut("metrics").unwrap().records[0]
        .fields
        .insert(Field::Unit, Scalar::String("percent".into()));
    assert!(evaluate(&metric_assertion("1"), &bad).is_err());
}
#[test]
fn incomplete_missing_and_invalid_facts_never_become_empty_passes() {
    let mut inputs = collections();
    inputs.get_mut("metrics").unwrap().complete = false;
    assert!(evaluate(&metric_assertion("1"), &inputs).is_err());
    inputs.get_mut("metrics").unwrap().complete = true;
    inputs.get_mut("metrics").unwrap().records.clear();
    assert!(evaluate(&metric_assertion("1"), &inputs).unwrap().passed);
    let any = expression(
        json!({"op":"any","input":{"op":"source","name":"metrics"},"predicate":{"op":"bool","value":true}}),
    );
    assert!(!evaluate(&any, &inputs).unwrap().passed);
    inputs.remove("entities");
    assert!(evaluate(&any, &inputs).is_err());
    let mut bad = collections();
    bad.get_mut("metrics").unwrap().records[0]
        .fields
        .remove(&Field::Value);
    assert!(evaluate(&metric_assertion("1"), &bad).is_err());
    bad.get_mut("metrics").unwrap().records[0]
        .fields
        .insert(Field::Value, Scalar::String("0.8".into()));
    assert!(evaluate(&metric_assertion("1"), &bad).is_err());
}
#[test]
fn finite_composition_preserves_set_count_projection_and_string_semantics() {
    let projected = json!({"op":"map","input":{"op":"entities"},"as":"e","body":{"op":"field","name":"name","of":{"op":"var","name":"e"}}});
    let names = json!({"op":"distinct","input":projected});
    let input = collections();
    for expr in [
        json!({"op":"compare","left":{"op":"count","input":names},"relation":"eq","right":{"op":"number","value":2}}),
        json!({"op":"contains","input":projected,"value":{"op":"string","value":"a"}}),
        json!({"op":"subset","left":names,"right":projected}),
        json!({"op":"all","input":{"op":"filter","input":{"op":"entities"},"predicate":{"op":"matches","value":{"op":"field","name":"name"},"pattern":"^[ab]$"}},"predicate":{"op":"in","value":{"op":"field","name":"name"},"set":[{"type":"string","value":"a"},{"type":"string","value":"b"}]}}),
        json!({"op":"compare","left":{"op":"concat","args":[{"op":"string","value":"a"},{"op":"replace","value":{"op":"string","value":"cc"},"from":"c","to":"b"}]},"relation":"eq","right":{"op":"string","value":"abb"}}),
        json!({"op":"and","args":[{"op":"not","arg":{"op":"bool","value":false}},{"op":"or","args":[{"op":"bool","value":false},{"op":"bool","value":true}]}]}),
        json!({"op":"all","input":{"op":"source","name":"metrics"},"predicate":{"op":"in","value":{"op":"field","name":"value"},"set":[{"type":"decimal","value":{"coefficient":"8","scale":1}},{"type":"decimal","value":{"coefficient":"81","scale":2}}]}}),
    ] {
        assert!(
            evaluate(&expression(expr.clone()), &input).unwrap().passed,
            "{expr}"
        );
    }
}
#[test]
fn evaluation_budgets_and_large_distinct_sets_are_bounded() {
    let mut inputs = collections();
    inputs.get_mut("entities").unwrap().records = (0..100)
        .map(|n| Record::syntax(&n.to_string(), "", None, None))
        .collect();
    let names =
        json!({"op":"map","input":{"op":"entities"},"as":"e","body":{"op":"field","name":"name"}});
    let expr =
        expression(json!({"op":"subset","left":{"op":"distinct","input":names},"right":names}));
    assert!(evaluate(&expr, &inputs).unwrap().passed);
    assert!(evaluate_since(&expr, &inputs, Instant::now() - Duration::from_secs(31)).is_err());
    inputs.get_mut("entities").unwrap().records = vec![Record::syntax(
        "a",
        &"x".repeat(MAX_FACT_BYTES + 1),
        None,
        None,
    )];
    assert!(evaluate(&expr, &inputs).is_err());
}

#[test]
fn observation_pointers_use_assertion_and_report_truncated_paths_without_ranges() {
    let mut inputs = collections();
    inputs.get_mut("entities").unwrap().records = vec![Record::syntax(
        "long_path",
        "",
        Some(&"界".repeat(1025)),
        None,
    )];
    let assertion = expression(json!({"op":"all","input":{"op":"entities"},
        "predicate":{"op":"bool","value":false}}));
    let outcome = evaluate(&assertion, &inputs).unwrap();
    assert!(!outcome.passed);
    assert!(
        outcome
            .false_observations
            .iter()
            .all(|observation| observation.expression_path.starts_with("/assertion"))
    );
    let located = outcome
        .false_observations
        .iter()
        .find_map(|observation| observation.current.as_ref())
        .unwrap();
    assert_eq!(located.file.as_ref().unwrap().chars().count(), 1024);
    assert!(located.file_truncated);
    assert!(located.range.is_none());
}
