use super::*;
use crate::config::selfcheck_policy::EvolutionFixture;

fn catalog() -> Catalog {
    Catalog::load(
        &Config {
            rulesets: config::RULESETS.iter().map(|s| (*s).into()).collect(),
            ..Default::default()
        },
        std::iter::empty(),
    )
    .unwrap()
}

#[test]
fn in_memory_rule_report_and_policy_scenarios_agree_with_independent_goldens() {
    let catalog = catalog();
    let cases = config::selfcheck::load().unwrap();
    validate_coverage(&cases, &catalog).unwrap();
    let mut count = 0;
    for case in cases {
        // The unit boundary is the in-memory evaluator dispatch. Native Git,
        // process, archive and workflow cases remain in the selfcheck integration gate.
        if matches!(
            &case.fixture.input,
            Input::Snapshot { .. } | Input::Runner { .. }
        ) || matches!(&case.fixture.input, Input::Evolution { scenario }
                if matches!(scenario.as_ref(), EvolutionFixture::Candidate { .. } | EvolutionFixture::Workflow { .. }))
        {
            continue;
        }
        let observed = observe(&case, &catalog)
            .unwrap_or_else(|error| panic!("{}/{}: {error:#}", case.suite, case.fixture.id));
        let mismatches = compare_golden(&case.golden, &observed);
        assert!(
            mismatches.is_empty(),
            "{}/{}: {mismatches:?}",
            case.suite,
            case.fixture.id
        );
        count += 1;
    }
    assert!(
        count > 800,
        "the independent scenario inventory unexpectedly shrank: {count}"
    );
}

#[test]
fn deleting_a_negative_oracle_is_detected_before_running_a_selfcheck() {
    let catalog = catalog();
    let mut cases = config::selfcheck::load().unwrap();
    cases.retain(|case| {
        !(case.suite == "minimal"
            && case.fixture.rule == "line-ending"
            && case.golden.get("/verdict") == Some(&json!("fail")))
    });
    let error = validate_coverage(&cases, &catalog).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("line-ending lacks independent compliant/violating goldens in minimal")
    );
}

#[test]
fn builtin_observations_preserve_selected_definition_versions_for_every_execution_outcome() {
    let mut catalog = catalog();
    let cases = config::selfcheck::load().unwrap();
    let selected = [
        "issue16-sql-format-placeholder-pass-minimal",
        "issue16-sql-string-format-fail-minimal",
        "issue16-sql-malformed-incomplete-minimal",
    ]
    .map(|id| cases.iter().find(|case| case.fixture.id == id).unwrap());
    for version in [2, 7] {
        catalog
            .entries
            .get_mut("py-sql-string-format")
            .unwrap()
            .builtin
            .as_mut()
            .unwrap()
            .version = version;
        for case in selected {
            let observed = observe(case, &catalog).unwrap();
            assert_eq!(observed["rule_version"], version, "{}", case.fixture.id);
            assert!(
                compare_golden(&case.golden, &observed).is_empty(),
                "{}",
                case.fixture.id
            );
        }
    }
}

#[tokio::test]
async fn empty_filter_is_incomplete_and_small_selected_suite_retains_its_claim_boundary() {
    let missing = run(Some("does-not-exist".into()), None).await;
    assert!(!missing.complete);
    assert_eq!(missing.decision, Decision::Incomplete);
    assert!(missing.fixtures.is_empty());
    assert!(
        missing
            .errors
            .iter()
            .any(|s| s.contains("No fixtures match"))
    );
    assert!(missing.verification.verified_shapes.is_empty());
    let selected = run(Some("minimal".into()), Some("line-ending".into())).await;
    assert!(selected.complete);
    assert_eq!(selected.decision, Decision::Pass);
    assert!(!selected.fixtures.is_empty());
    assert_eq!(
        selected.verification.verified_shapes.len(),
        selected.fixtures.len()
    );
    assert!(
        selected
            .fixtures
            .iter()
            .all(|case| case.suite == "minimal" && case.rule == "line-ending")
    );
    assert!(
        selected
            .verification
            .known_limits
            .iter()
            .any(|limit| limit.contains("partial regression"))
    );
}
