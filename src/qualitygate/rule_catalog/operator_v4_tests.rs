use super::*;
use serde_json::{Value, json};

fn expr(value: Value) -> Expr {
    serde_json::from_value(value).unwrap()
}
fn types() -> BTreeMap<String, SourceType> {
    BTreeMap::from([
        (
            "metrics".into(),
            SourceType::Metric(Dimension {
                schema: FactSchema::FunctionMetric,
                unit: "count".into(),
                measurement: "complexity-v1".into(),
            }),
        ),
        (
            "clones".into(),
            SourceType::ClonePair(Dimension {
                schema: FactSchema::ClonePair,
                unit: "ratio".into(),
                measurement: "similarity-v1".into(),
            }),
        ),
    ])
}
#[test]
fn fields_are_closed_and_numeric_dimensions_are_checked() {
    let valid = expr(
        json!({"op":"all","input":{"op":"source","name":"metrics"},"as":"m","predicate":{"op":"compare","left":{"op":"field","name":"value","of":{"op":"var","name":"m"}},"relation":"le","right":{"op":"decimal","value":{"coefficient":"101","scale":1}}}}),
    );
    let checked = valid.validate(&types()).unwrap();
    assert!(checked.required_fields["metrics"].contains(&Field::Value));
    let wrong = expr(
        json!({"op":"all","input":{"op":"source","name":"metrics"},"predicate":{"op":"matches","value":{"op":"field","name":"similarity"},"pattern":"x"}}),
    );
    assert!(
        wrong
            .validate(&types())
            .unwrap_err()
            .contains("unavailable")
    );
    let mismatched = expr(
        json!({"op":"all","input":{"op":"source","name":"metrics"},"as":"m","predicate":{"op":"all","input":{"op":"source","name":"clones"},"predicate":{"op":"compare","left":{"op":"field","name":"value","of":{"op":"var","name":"m"}},"relation":"le","right":{"op":"field","name":"similarity"}}}}),
    );
    assert!(
        mismatched
            .validate(&types())
            .unwrap_err()
            .contains("mismatch")
    );
    assert!(serde_json::from_value::<Expr>(json!({"op":"field","name":"arbitrary_json"})).is_err());
}
#[test]
fn lexical_scope_decimal_canonicality_and_expression_budgets_are_enforced() {
    for value in [
        json!({"op":"decimal","value":{"coefficient":"10","scale":1}}),
        json!({"op":"var","name":"unbound"}),
        json!({"op":"source","name":"missing"}),
        json!({"op":"and","args":[]}),
        json!({"op":"matches","value":{"op":"string","value":"x"},"pattern":"["}),
        json!({"op":"all","input":{"op":"entities"},"as":"x","predicate":{"op":"all","input":{"op":"entities"},"as":"x","predicate":{"op":"bool","value":true}}}),
    ] {
        assert!(expr(value).validate(&types()).is_err());
    }
    let mut deep = Expr::Bool { value: true };
    for _ in 0..18 {
        deep = Expr::Not {
            arg: Box::new(deep),
        };
    }
    assert!(deep.validate(&types()).is_err());
    let invalid = BTreeMap::from([("entities".into(), SourceType::Syntax)]);
    assert!(Expr::Bool { value: true }.validate(&invalid).is_err());
}
#[test]
fn source_selectors_require_explicit_scope_metric_contract_and_supported_version() {
    let value = json!({"producer":"scan","schema":"function_metric","schema_version":1,"project":"app","metric":"cc","unit":"count","measurement":"cc-v1"});
    let source: FactSource = serde_json::from_value(value.clone()).unwrap();
    source.validate().unwrap();
    for (key, invalid) in [
        ("schema_version", json!(2)),
        ("project", json!("")),
        ("metric", Value::Null),
        ("configuration", json!("debug")),
        ("similarity_floor", json!({"coefficient":"1","scale":1})),
    ] {
        let mut candidate = value.clone();
        candidate[key] = invalid;
        assert!(
            serde_json::from_value::<FactSource>(candidate)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

#[test]
fn computed_counts_and_fact_schemas_cannot_borrow_measured_units() {
    let mut types = types();
    let metric_dimension = Dimension {
        schema: FactSchema::FunctionMetric,
        unit: "ratio".into(),
        measurement: "same-descriptor".into(),
    };
    types.insert(
        "metrics".into(),
        SourceType::Metric(metric_dimension.clone()),
    );
    types.insert(
        "clones".into(),
        SourceType::ClonePair(Dimension {
            schema: FactSchema::ClonePair,
            ..metric_dimension.clone()
        }),
    );
    let count = json!({"op":"count","input":{"op":"entities"}});
    let metric = json!({"op":"field","name":"value"});
    for unit in ["percent", "ratio"] {
        let SourceType::Metric(dimension) = types.get_mut("metrics").unwrap() else {
            unreachable!()
        };
        dimension.unit = unit.into();
        for (left, right) in [
            (count.clone(), metric.clone()),
            (metric.clone(), count.clone()),
        ] {
            let compare = expr(json!({"op":"all","input":{"op":"source","name":"metrics"},
                "predicate":{"op":"compare","left":left,"relation":"le","right":right}}));
            assert!(
                compare.validate(&types).unwrap_err().contains("mismatch"),
                "{unit}"
            );
        }
    }
    let cross_schema = expr(
        json!({"op":"all","input":{"op":"source","name":"metrics"},"as":"metric",
        "predicate":{"op":"all","input":{"op":"source","name":"clones"},
            "predicate":{"op":"compare","left":{"op":"field","name":"value","of":{"op":"var","name":"metric"}},
                "relation":"eq","right":{"op":"field","name":"similarity"}}}}),
    );
    assert!(
        cross_schema
            .validate(&types)
            .unwrap_err()
            .contains("mismatch")
    );
    for literal in [
        json!({"op":"number","value":1}),
        json!({"op":"decimal","value":{"coefficient":"15","scale":1}}),
    ] {
        expr(json!({"op":"all","input":{"op":"source","name":"metrics"},
            "predicate":{"op":"compare","left":metric,"relation":"le","right":literal}}))
        .validate(&types)
        .unwrap();
        expr(json!({"op":"compare","left":count,"relation":"ge","right":literal}))
            .validate(&types)
            .unwrap();
    }
    expr(json!({"op":"compare","left":count,"relation":"eq",
        "right":{"op":"count","input":{"op":"source","name":"metrics"}}}))
    .validate(&types)
    .unwrap();
    let SourceType::Metric(dimension) = types.get_mut("metrics").unwrap() else {
        unreachable!()
    };
    dimension.schema = FactSchema::ClonePair;
    assert!(Expr::Bool { value: true }.validate(&types).is_err());
}
