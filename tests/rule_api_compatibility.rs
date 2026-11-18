//! Public rule paths shared by the Cargo library and the Bazel aggregate.

use qualitygate::{adapters, config, domain, rule_catalog, rule_evaluation, snapshot};
use std::collections::BTreeMap;

#[test]
fn established_rule_entry_points_use_the_same_public_types() {
    type Builtin = fn(&str, &config::RuleSetting, &snapshot::Snapshot) -> domain::CheckResult;
    type Custom =
        fn(&config::CustomRule, &config::RuleSetting, &snapshot::Snapshot) -> domain::CheckResult;

    let _: [Builtin; 2] = [
        adapters::rules::evaluate,
        rule_evaluation::builtin_rules::evaluate,
    ];
    let _: [Custom; 2] = [
        adapters::custom_rules::evaluate,
        rule_evaluation::custom_rules::evaluate,
    ];
    let _: adapters::facts::RuleFacts<'_> = rule_evaluation::evaluation_facts::RuleFacts::default();

    let expression: rule_catalog::operator::Expr = domain::operator::Expr::Bool { value: true };
    let facts: adapters::operators::Facts<'_> = rule_evaluation::operators_v2::Facts {
        entities: &[],
        complete: true,
    };
    assert!(
        adapters::operators::evaluate(&expression, facts)
            .unwrap()
            .passed
    );

    let expression: rule_catalog::operator_v3::Expr =
        domain::operator_v3::Expr::Bool { value: true };
    let primary: adapters::operators_v3::Collection<'_> =
        rule_evaluation::operators_v3::Collection {
            entities: Vec::new(),
            complete: true,
        };
    let collections = BTreeMap::from([("entities".to_owned(), primary)]);
    assert!(
        adapters::operators_v3::evaluate(&expression, &collections)
            .unwrap()
            .passed
    );
}

#[test]
fn established_diagnostic_paths_preserve_fingerprints() {
    let expected = snapshot::digest(b"sample\0src/lib.rs\0entity");
    for diagnostic in [
        adapters::rules::diagnostic,
        rule_evaluation::builtin_rules::diagnostic,
    ] {
        let result = diagnostic(
            "sample",
            Some("src/lib.rs"),
            None,
            "message".into(),
            serde_json::Value::Null,
            "fix",
            "entity",
        );
        assert_eq!(result.fingerprint, expected);
        assert_eq!(result.id, format!("sample:{}", &expected[7..23]));
    }
}
