use super::*;

fn apply(root: &Path, mutation: Mutation) -> Result<Value> {
    update(root, Path::new("qualitygate.yaml"), mutation)
}

fn config(root: &Path) -> Config {
    super::super::parse(&std::fs::read(root.join("qualitygate.yaml")).unwrap()).unwrap()
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), "# preserve this comment on no-op\nschema_version: 1\nrules:\n  line-ending: {enabled: true}\nprofiles:\n  quick: {include: [line-ending]}\n  full: {include: [line-ending]}\n").unwrap();
    root
}

#[test]
fn enabling_is_idempotent_and_configuration_preserves_untouched_defaults() {
    let root = fixture();
    let original = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
    let noop = apply(root.path(), Mutation::Enable("line-ending".into())).unwrap();
    assert_eq!(noop["changed"], false);
    assert_eq!(noop["before_digest"], noop["after_digest"]);
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        original
    );
    let changed = apply(
        root.path(),
        Mutation::Configure {
            id: "diff-size".into(),
            parameters: vec!["max_added_lines=42".into()],
            severity: Some(crate::domain::Severity::Warning),
            required: Some(false),
        },
    )
    .unwrap();
    assert_eq!(changed["changed"], true);
    assert_ne!(changed["before_digest"], changed["after_digest"]);
    let selected = config(root.path());
    assert_eq!(selected.rules["diff-size"].parameters.len(), 1);
    assert_eq!(
        selected.rules["diff-size"].parameters["max_added_lines"],
        42
    );
    assert!(!selected.rules["diff-size"].enabled);
    assert!(!selected.rules["diff-size"].required);
    apply(root.path(), Mutation::Enable("diff-size".into())).unwrap();
    let selected = config(root.path());
    assert!(selected.rules["diff-size"].enabled);
    assert!(
        selected
            .profiles
            .values()
            .all(|profile| profile.include.contains(&"diff-size".into()))
    );
    apply(root.path(), Mutation::Disable("diff-size".into())).unwrap();
    assert!(!config(root.path()).rules["diff-size"].enabled);
    assert!(!root.path().join("qualitygate.yaml.lock").exists());
}

#[test]
fn category_rename_updates_multiple_memberships_and_forced_delete_keeps_other_labels() {
    let root = fixture();
    for name in ["team", "audit"] {
        let created = apply(
            root.path(),
            Mutation::Create {
                name: name.into(),
                description: format!("{name} review"),
            },
        )
        .unwrap();
        assert_eq!(created["result"]["custom"], true);
        apply(
            root.path(),
            Mutation::Assign {
                id: "line-ending".into(),
                category: name.into(),
            },
        )
        .unwrap();
    }
    apply(
        root.path(),
        Mutation::Rename {
            name: "team".into(),
            new_name: "team-new".into(),
        },
    )
    .unwrap();
    let membership = config(root.path()).rule_categories["line-ending"]
        .names()
        .to_vec();
    assert_eq!(membership, ["team-new", "audit"]);
    let original = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
    assert!(
        apply(
            root.path(),
            Mutation::Delete {
                name: "team-new".into(),
                force: false
            }
        )
        .unwrap_err()
        .to_string()
        .contains("explicit assignments")
    );
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        original
    );
    apply(
        root.path(),
        Mutation::Delete {
            name: "team-new".into(),
            force: true,
        },
    )
    .unwrap();
    assert_eq!(
        config(root.path()).rule_categories["line-ending"].names(),
        ["audit"]
    );
    apply(
        root.path(),
        Mutation::Unassign {
            id: "line-ending".into(),
            category: "audit".into(),
        },
    )
    .unwrap();
    assert!(
        !config(root.path())
            .rule_categories
            .contains_key("line-ending")
    );
    apply(
        root.path(),
        Mutation::Assign {
            id: "line-ending".into(),
            category: "audit".into(),
        },
    )
    .unwrap();
    let duplicate = apply(
        root.path(),
        Mutation::Assign {
            id: "line-ending".into(),
            category: "audit".into(),
        },
    )
    .unwrap();
    assert_eq!(duplicate["changed"], false);
    apply(
        root.path(),
        Mutation::Delete {
            name: "audit".into(),
            force: true,
        },
    )
    .unwrap();
    assert!(
        !config(root.path())
            .rule_categories
            .contains_key("line-ending")
    );
    assert!(config(root.path()).rules["line-ending"].enabled);
}

#[test]
fn invalid_mutations_and_contending_writer_preserve_exact_candidate_bytes() {
    let root = fixture();
    let original = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
    let mutations = [
        Mutation::Create {
            name: "core".into(),
            description: "duplicate".into(),
        },
        Mutation::Rename {
            name: "core".into(),
            new_name: "security".into(),
        },
        Mutation::Rename {
            name: "missing".into(),
            new_name: "fresh".into(),
        },
        Mutation::Delete {
            name: "missing".into(),
            force: true,
        },
        Mutation::Assign {
            id: "missing".into(),
            category: "core".into(),
        },
        Mutation::Assign {
            id: "line-ending".into(),
            category: "missing".into(),
        },
        Mutation::Unassign {
            id: "line-ending".into(),
            category: "core".into(),
        },
        Mutation::Enable("missing".into()),
        Mutation::Configure {
            id: "line-ending".into(),
            parameters: vec![],
            severity: None,
            required: None,
        },
        Mutation::Configure {
            id: "diff-size".into(),
            parameters: vec!["misspelled=2".into()],
            severity: None,
            required: None,
        },
    ];
    for mutation in mutations {
        assert!(apply(root.path(), mutation).is_err());
        assert_eq!(
            std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
            original
        );
        assert!(!root.path().join("qualitygate.yaml.lock").exists());
    }
    std::fs::write(root.path().join("qualitygate.yaml.lock"), b"other writer").unwrap();
    assert!(
        apply(root.path(), Mutation::Disable("line-ending".into()))
            .unwrap_err()
            .to_string()
            .contains("another writer")
    );
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml.lock")).unwrap(),
        b"other writer"
    );
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        original
    );
}

#[test]
fn enabling_unselected_language_rule_adds_package_and_explicit_profiles() {
    let root = fixture();
    let inventory = Inventory::for_policy(root.path(), Some(config(root.path())), true).unwrap();
    let id = inventory
        .builtins
        .entries
        .iter()
        .find(|(_, entry)| entry.package == "lang-java")
        .unwrap()
        .0
        .clone();
    apply(root.path(), Mutation::Enable(id.clone())).unwrap();
    let updated = config(root.path());
    assert!(updated.rulesets.contains(&"lang-java".into()));
    assert!(updated.rules[&id].enabled);
    assert!(
        updated
            .profiles
            .values()
            .all(|profile| profile.include.contains(&id))
    );
}
