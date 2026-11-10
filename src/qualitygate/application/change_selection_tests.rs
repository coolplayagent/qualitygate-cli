use super::*;
use serde_json::json;

fn plan() -> config::Plan {
    let config = config::parse(b"schema_version: 1\n").unwrap();
    let mut plan = config::Plan::build(&config, None, "full").unwrap();
    plan.commands = serde_json::from_value(json!([
        {"id":"c","argv":["cc"],"when_changed":{"languages":["c"]}},
        {"id":"cpp","argv":["c++"],"when_changed":{"languages":["cpp"]}},
        {"id":"rust","argv":["cargo","test"],"when_changed":{"languages":["rust"]}}
    ]))
    .unwrap();
    plan.order = vec!["c".into(), "cpp".into(), "rust".into()];
    plan
}

fn changed(root: &std::path::Path, name: &str, old: Option<&str>) -> Arc<Snapshot> {
    let mut snapshot = super::super::test_support::snapshot(root);
    snapshot.scope_evidence.mode = crate::domain::check_scope::CheckScope::Delivery;
    snapshot.files.insert(
        name.into(),
        crate::snapshot::File {
            bytes: b"changed".to_vec(),
            executable: false,
        },
    );
    if let Some(old) = old {
        snapshot.base_files.insert(
            old.into(),
            crate::snapshot::File {
                bytes: b"old".to_vec(),
                executable: false,
            },
        );
    }
    snapshot.changes.insert(
        name.into(),
        crate::snapshot::Change {
            kind: if old.is_some() { "renamed" } else { "modified" }.into(),
            old_path: old.map(str::to_owned),
            added_lines: [1].into(),
            removed_lines: [1].into(),
        },
    );
    Arc::new(snapshot)
}

#[tokio::test]
async fn header_changes_keep_both_c_consumers_and_renames_retain_old_language() {
    let root = tempfile::tempdir().unwrap();
    let plan = plan();
    let selected = select(&plan, &changed(root.path(), "api.h", None), &[], false)
        .await
        .unwrap();
    assert!(selected["c"].execute);
    assert!(selected["cpp"].execute);
    assert!(!selected["rust"].execute);
    let renamed = select(
        &plan,
        &changed(root.path(), "notes.txt", Some("source.rs")),
        &[],
        false,
    )
    .await
    .unwrap();
    assert!(renamed["rust"].execute);
    assert!(!renamed["c"].execute);
}

#[tokio::test]
async fn policy_uncertainty_shared_inputs_and_repository_scope_force_all_consumers() {
    let root = tempfile::tempdir().unwrap();
    let plan = plan();
    for (name, old, protected, invalid) in [
        ("notes.txt", None, vec![], true),
        ("Cargo.lock", None, vec![], false),
        ("policy.yaml", None, vec!["policy.yaml".into()], false),
        ("notes.txt", Some("Cargo.toml"), vec![], false),
    ] {
        let result = select(&plan, &changed(root.path(), name, old), &protected, invalid)
            .await
            .unwrap();
        assert!(result.values().all(|s| s.execute));
    }
    let mut repository = (*changed(root.path(), "notes.txt", None)).clone();
    repository.scope_evidence.mode = crate::domain::check_scope::CheckScope::Repository;
    assert!(
        select(&plan, &Arc::new(repository), &[], false)
            .await
            .unwrap()
            .values()
            .all(|s| s.execute)
    );
    assert!(
        select(
            &plan,
            &changed(root.path(), "notes.txt", None),
            &["[".into()],
            false
        )
        .await
        .is_err()
    );
    let mut unconditional = plan;
    for check in &mut unconditional.commands {
        check.when_changed = None;
    }
    assert!(
        select(
            &unconditional,
            &changed(root.path(), "notes.txt", None),
            &[],
            false
        )
        .await
        .unwrap()
        .values()
        .all(|s| s.execute)
    );
}
