use super::*;

const POLICY: &str = "schema_version: 1\nrules:\n  line-ending: {enabled: true, required: true}\nchecks:\n  - id: tests\n    kind: command\n    argv: [cargo, test]\n";

fn rows(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().into())
        .collect()
}

#[test]
fn discovery_without_policy_does_not_enable_rules_or_claim_mandatory_checks() {
    let root = tempfile::tempdir().unwrap();
    let inventory = list(root.path(), "qualitygate.yaml", Some("rust"), "builtin").unwrap();
    let rules = inventory["rules"].as_array().unwrap();
    assert!(!rules.is_empty());
    assert!(rules.iter().all(|row| row["enabled"] == false));
    assert!(rules.iter().all(|row| row["configuration"].is_null()));
    assert!(
        rules
            .iter()
            .all(|row| row["language"].as_array().unwrap().is_empty()
                || row["language"].as_array().unwrap().contains(&json!("rust")))
    );
    assert_eq!(inventory["mandatory"], json!([]));
    assert_eq!(inventory["mandatory_checks"], json!([]));
    assert!(inventory["policy_digest"].is_null());
    assert!(rows(&inventory, "rules").contains(&"line-ending".into()));
    assert!(!root.path().join("qualitygate.yaml").exists());
    assert!(list(root.path(), "missing.yaml", None, "all").is_err());
}

#[test]
fn filtered_context_keeps_mandatory_baseline_and_matches_frozen_input() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), POLICY).unwrap();
    let before = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
    let context = context(root.path(), "qualitygate.yaml", Some("security")).unwrap();
    assert!(context.get("rules").is_none());
    assert!(!rows(&context, "selected").contains(&"line-ending".into()));
    assert!(rows(&context, "mandatory").contains(&"line-ending".into()));
    assert_eq!(context["mandatory_checks"][0]["id"], "tests");
    let frozen = context_from_files(
        "qualitygate.yaml",
        [("qualitygate.yaml", POLICY.as_bytes())],
        Some("security"),
    )
    .unwrap();
    assert_eq!(context, frozen);
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        before
    );
    assert!(context_from_files("qualitygate.yaml", std::iter::empty(), None).is_err());
    assert!(
        context_from_files(
            "qualitygate.yaml",
            [("qualitygate.yaml", b"schema_version: 999".as_slice())],
            None
        )
        .is_err()
    );
}

#[test]
fn descriptions_and_category_counts_reflect_selection_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), POLICY).unwrap();
    let description = describe(root.path(), "qualitygate.yaml", "line-ending").unwrap();
    assert_eq!(description["enabled"], true);
    assert_eq!(description["implementation"], "line-ending");
    assert_eq!(description["source"], "builtin");
    assert!(description["definition"]["builtin"].is_object());
    assert!(describe(root.path(), "qualitygate.yaml", "unknown-rule").is_err());
    let summary = categories(root.path(), "qualitygate.yaml").unwrap();
    let core = summary["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "core")
        .unwrap();
    assert!(core["rule_count"].as_u64().unwrap() >= 3);
    assert!(core["enabled_count"].as_u64().unwrap() >= 1);
    assert_eq!(core["custom"], false);
    assert_eq!(summary["review_trust"], "local_candidate");
    let project = list(root.path(), "qualitygate.yaml", None, "project").unwrap();
    assert_eq!(project["rules"], json!([]));
    assert!(!project["mandatory"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_filters_and_explicit_missing_projects_are_errors_not_empty_success() {
    let root = tempfile::tempdir().unwrap();
    for language in ["", "Rust", "all", "rust/../java", "1rust", &"r".repeat(65)] {
        assert!(
            list(root.path(), "qualitygate.yaml", Some(language), "all")
                .unwrap_err()
                .to_string()
                .contains("Language")
        );
    }
    assert!(
        list(root.path(), "qualitygate.yaml", None, "unknown")
            .unwrap_err()
            .to_string()
            .contains("Unknown rule source")
    );
    assert!(
        list_filtered(
            root.path(),
            "qualitygate.yaml",
            None,
            "all",
            Some("unknown")
        )
        .unwrap_err()
        .to_string()
        .contains("Unknown category")
    );
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\ncustom_rules: absent\n",
    )
    .unwrap();
    assert!(list(root.path(), "qualitygate.yaml", None, "all").is_err());
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nunknown: true\n",
    )
    .unwrap();
    assert!(list(root.path(), "qualitygate.yaml", None, "builtin").is_err());
}

#[test]
fn selected_catalog_uses_explicit_packages_and_project_override_identity() {
    let root = tempfile::tempdir().unwrap();
    let policy = super::super::parse(POLICY.as_bytes()).unwrap();
    let inventory = Inventory::for_policy(root.path(), Some(policy.clone()), false).unwrap();
    let mut selected = policy;
    selected.rulesets = vec!["lang-java".into()];
    let catalog = inventory.selected(&selected);
    assert!(
        catalog
            .entries
            .values()
            .any(|entry| entry.package == "lang-java")
    );
    assert!(
        catalog
            .entries
            .values()
            .all(|entry| ["core", "shared", "lang-java"].contains(&entry.package.as_str()))
    );
    let mut other_origin = inventory.builtins.entries["line-ending"].clone();
    other_origin.origin = "other-source".into();
    assert!(
        inventory
            .configuration("line-ending", &other_origin)
            .is_none()
    );
    assert!(!inventory.enabled("line-ending", &other_origin));
    let no_definition = Entry {
        origin: "empty".into(),
        package: "project".into(),
        builtin: None,
        custom: None,
    };
    assert!(languages(&no_definition).is_empty());
}
