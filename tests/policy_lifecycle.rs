mod common;
#[path = "common/evolution.rs"]
mod evolution;

use evolution::*;
use qualitygate::config::{policy_candidates, policy_store::Store};

#[test]
fn semantic_lifecycle_changes_are_candidates_and_history_retains_definitions_and_evidence() {
    let fixture = Fixture::new();
    fixture.enable();
    let original =
        policy_candidates::candidate(&Store::open(fixture.root.path()).unwrap(), &fixture.id)
            .unwrap()
            .1
            .policy_digest;
    for (operation, expected, enabled, required, severity) in [
        ("demote", "demoted", true, false, "warning"),
        ("deprecate", "deprecated", true, false, "warning"),
        ("retire", "retired", false, false, "warning"),
        ("revalidate", "revalidate", true, false, "warning"),
        ("revoke", "revoked", false, false, "warning"),
    ] {
        let result = run(
            fixture.root.path(),
            &[
                "rules",
                operation,
                "line-ending",
                "--candidate",
                &fixture.id,
                "--actor",
                "generator",
                "--reason",
                "Independent lifecycle review required",
            ],
            0,
        );
        assert_eq!(result["revision"]["status"], "candidate");
        assert!(result["revision"]["approval_ref"].is_null());
        let store = Store::open(fixture.root.path()).unwrap();
        assert!(store.index.active_policy.is_none());
        let (_, config, frozen) = policy_candidates::load_version(
            &store,
            result["revision"]["policy_digest"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(config.rule_lifecycle["line-ending"].state).unwrap(),
            expected
        );
        let (config, catalog) = frozen.resolve(&config).unwrap();
        assert_eq!(config.rules["line-ending"].enabled, enabled);
        assert_eq!(config.rules["line-ending"].required, required);
        assert_eq!(
            serde_json::to_value(config.rules["line-ending"].severity).unwrap(),
            severity
        );
        assert!(catalog.entries.contains_key("line-ending"));
        assert_eq!(
            config.rule_lifecycle["line-ending"].evidence_refs,
            fixture.suite.motivating_evidence
        );
    }
    let store = Store::open(fixture.root.path()).unwrap();
    assert!(
        policy_candidates::load_version(&store, &original)
            .unwrap()
            .1
            .rules["line-ending"]
            .enabled
    );
    let history = run(fixture.root.path(), &["rules", "history", "line-ending"], 0);
    assert_eq!(history["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(history["candidates"].as_array().unwrap().len(), 1);
    // A retired candidate cannot weaken the protected negative-case oracle.
    assert_eq!(
        fixture.validate("4", 1)["evaluation"]["conclusion"],
        "block"
    );
    run(
        fixture.root.path(),
        &[
            "rules",
            "revalidate",
            "line-ending",
            "--candidate",
            &fixture.id,
            "--actor",
            "generator",
            "--reason",
            "cannot edit rejection",
        ],
        2,
    );
}
