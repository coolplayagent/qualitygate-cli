use super::*;
use serde_json::json;

#[tokio::test]
async fn project_rule_needs_current_source_review_and_historical_policy_keeps_its_definition() {
    let yaml = "schema_version: 1\ncustom_rules: qualitygate/rules\nrules:\n  behavior-names: {}\n";
    let root = test_support::repository("schema_version: 1\n").await;
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nTests must describe behavior.\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join("qualitygate/rules")).unwrap();
    let source = config::rule_authoring::source(root.path(), "AGENTS.md", "Rules").unwrap();
    let rule = json!({"id":"behavior-names","version":1,"source":source,"language":["rust"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},"then":{"name_pattern":"^test_"},"fix":"Use scenario names"});
    let rule_bytes = serde_json::to_vec(&rule).unwrap();
    std::fs::write(
        root.path().join("qualitygate/rules/behavior-names.yaml"),
        &rule_bytes,
    )
    .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), yaml).unwrap();
    std::fs::write(
        root.path().join("source.rs"),
        "#[test]\nfn test_observable_behavior() { assert_eq!(2 + 2, 4); }\n",
    )
    .unwrap();
    let options = test_support::options(root.path());
    let unreviewed = check(options.clone()).await.unwrap();
    assert_eq!(unreviewed.gate.decision, Decision::Incomplete);
    assert!(
        unreviewed.checks[0]
            .execution
            .reason
            .as_ref()
            .unwrap()
            .contains("source review")
    );
    let mut config = config::parse(yaml.as_bytes()).unwrap();
    let catalog = config::catalog::Catalog::load(
        &config,
        [(
            "qualitygate/rules/behavior-names.yaml",
            rule_bytes.as_slice(),
        )],
    )
    .unwrap();
    let resolved = catalog.resolve(&config).unwrap();
    let binding = config::source_reviews::binding(
        "behavior-names",
        &catalog.entries["behavior-names"],
        &resolved.rules["behavior-names"],
    )
    .unwrap()
    .unwrap();
    config
        .source_reviews
        .insert("behavior-names".into(), test_support::adoption(&binding));
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    let reviewed = check(options.clone()).await.unwrap();
    assert_eq!(reviewed.gate.decision, Decision::Pass);
    let preflight = initialization_preflight(root.path(), &config, "qualitygate.yaml")
        .await
        .unwrap();
    assert!(preflight.complete);
    snapshot::run_git(root.path(), &["add", "."], None)
        .await
        .unwrap();
    snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "reviewed policy",
        ],
        None,
    )
    .await
    .unwrap();
    let context = rule_context::read(
        root.path().into(),
        "qualitygate.yaml".into(),
        None,
        Some("HEAD".into()),
    )
    .await
    .unwrap();
    assert_eq!(context["review_trust"], "caller_supplied_ref");
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nDifferent normative requirement.\n",
    )
    .unwrap();
    let changed = check(options.clone()).await.unwrap();
    assert_eq!(changed.gate.decision, Decision::Incomplete);
    assert!(
        changed.checks[0]
            .execution
            .reason
            .as_ref()
            .unwrap()
            .contains("source")
    );
    let mut historical = options;
    historical.policy_ref = Some("HEAD".into());
    let result = check(historical).await.unwrap();
    assert_eq!(result.policy.trust, "caller_supplied_ref");
    assert_eq!(result.gate.decision, Decision::Incomplete);
}

#[tokio::test]
async fn historical_policy_detects_task_and_verification_asset_replacement_before_execution() {
    let yaml = "schema_version: 1\nverification_assets: [checks/**]\nchecks:\n  - id: unit\n    argv: [git, --version]\n";
    let root = test_support::repository(yaml).await;
    std::fs::create_dir_all(root.path().join("checks")).unwrap();
    std::fs::write(root.path().join("checks/input.txt"), "reviewed input").unwrap();
    let task = "schema_version: 1\ntask_id: release\nacceptance:\n  - id: behavior\n    description: Verify observable behavior\n    verification:\n      check_id: behavior\n      argv: [git, --version]\n";
    std::fs::write(root.path().join("task.yaml"), task).unwrap();
    snapshot::run_git(root.path(), &["add", "."], None)
        .await
        .unwrap();
    snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "reviewed task",
        ],
        None,
    )
    .await
    .unwrap();
    let mut options = test_support::options(root.path());
    options.task = Some("task.yaml".into());
    options.policy_ref = Some("HEAD".into());
    let before = check(options.clone()).await.unwrap();
    assert!(before.gate.complete);
    std::fs::write(
        root.path().join("task.yaml"),
        task.replace("Verify observable behavior", "Changed task claim"),
    )
    .unwrap();
    std::fs::write(root.path().join("checks/input.txt"), "different input").unwrap();
    let after = check(options).await.unwrap();
    assert_eq!(after.gate.decision, Decision::Incomplete);
    assert_eq!(after.policy.changes, vec!["checks/input.txt", "task.yaml"]);
    assert!(
        after
            .checks
            .iter()
            .all(|c| c.execution.started_at_ms.is_none())
    );
}
