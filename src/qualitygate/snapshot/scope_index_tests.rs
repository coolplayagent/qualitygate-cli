use super::*;
use crate::domain::check_scope::{CheckScope, ScopeEvidence};

fn change(old: Option<&str>, lines: &[usize]) -> Change {
    Change {
        kind: "modified".into(),
        old_path: old.map(str::to_owned),
        added_lines: lines.iter().copied().collect(),
        removed_lines: Default::default(),
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        scope_evidence: ScopeEvidence {
            mode: CheckScope::Delivery,
            ..Default::default()
        },
        root: "/unused".into(),
        identity: super::super::Identity {
            mode: "diff".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: String::new(),
            verification_digest: None,
            merge_request: None,
        },
        files: BTreeMap::new(),
        base_files: BTreeMap::new(),
        commits: Vec::new(),
        path_filter: None,
        changes: BTreeMap::from([
            ("src/added.rs".into(), change(None, &[1, 2])),
            (
                "src/changed.rs".into(),
                change(Some("src/changed.rs"), &[4]),
            ),
            ("src/renamed.rs".into(), change(Some("old/name.rs"), &[])),
            ("src/deleted.rs".into(), change(Some("src/deleted.rs"), &[])),
            ("src/binary.rs".into(), change(Some("src/binary.rs"), &[])),
            ("src/mode.rs".into(), change(Some("src/mode.rs"), &[])),
            ("a/alias.rs".into(), change(Some("shared.rs"), &[10])),
            ("b/alias.rs".into(), change(Some("shared.rs"), &[20])),
            ("c/alias.rs".into(), change(Some("src/changed.rs"), &[30])),
        ]),
    }
}

#[test]
fn indexed_scope_preserves_feedback_alias_order_and_current_key_priority() {
    let mut snapshot = snapshot();
    for mode in [CheckScope::Delivery, CheckScope::Repository] {
        snapshot.scope_evidence.mode = mode;
        for filter in [
            None,
            Some("src"),
            Some("src/changed.rs"),
            Some("old"),
            Some("missing"),
        ] {
            snapshot.path_filter = filter.map(str::to_owned);
            let scope = snapshot.scope_index();
            for path in [
                "src/added.rs",
                "src/changed.rs",
                "src/renamed.rs",
                "old/name.rs",
                "src/deleted.rs",
                "src/binary.rs",
                "src/mode.rs",
                "shared.rs",
                "missing",
                "src-sibling/file.rs",
            ] {
                assert_eq!(
                    scope.includes(path),
                    snapshot.includes(path),
                    "{mode:?} {filter:?} {path}"
                );
                for range in [
                    None,
                    Some(Range {
                        start_line: 1,
                        end_line: 2,
                    }),
                    Some(Range {
                        start_line: 4,
                        end_line: 4,
                    }),
                    Some(Range {
                        start_line: 10,
                        end_line: 10,
                    }),
                    Some(Range {
                        start_line: 20,
                        end_line: 30,
                    }),
                ] {
                    assert_eq!(
                        scope.selects_diagnostic(Some(path), range.as_ref()),
                        snapshot.selects_diagnostic(Some(path), range.as_ref()),
                        "{mode:?} {filter:?} {path} {range:?}"
                    );
                }
            }
            assert!(scope.selects_diagnostic(
                None,
                Some(&Range {
                    start_line: 1,
                    end_line: 1
                })
            ));
        }
    }
    snapshot.scope_evidence.mode = CheckScope::Delivery;
    snapshot.path_filter = None;
    let scope = snapshot.scope_index();
    assert!(scope.selects_diagnostic(
        Some("shared.rs"),
        Some(&Range {
            start_line: 10,
            end_line: 10
        })
    ));
    assert!(!scope.selects_diagnostic(
        Some("shared.rs"),
        Some(&Range {
            start_line: 20,
            end_line: 20
        })
    ));
    assert!(!scope.selects_diagnostic(
        Some("src/changed.rs"),
        Some(&Range {
            start_line: 30,
            end_line: 30
        })
    ));
}

#[test]
fn rebuilding_after_public_snapshot_edits_cannot_reuse_stale_aliases() {
    let mut snapshot = snapshot();
    assert!(snapshot.scope_index().includes("old/name.rs"));
    snapshot.changes.clear();
    assert!(!snapshot.scope_index().includes("old/name.rs"));
    snapshot
        .changes
        .insert("new.rs".into(), change(Some("new-old.rs"), &[7]));
    let scope = snapshot.scope_index();
    assert!(scope.includes("new-old.rs"));
    assert!(!scope.includes("old/name.rs"));
    assert_eq!(scope.old_paths.len(), 1);
    snapshot.scope_evidence.mode = CheckScope::Repository;
    let scope = snapshot.scope_index();
    assert!(scope.old_paths.is_empty());
    assert!(scope.includes("unchanged.rs"));
}

#[test]
fn bulk_scope_index_matches_single_lookups_in_large_inventory() {
    let mut snapshot = snapshot();
    snapshot.changes = (0..2000)
        .map(|i| {
            (
                format!("new/{i}.rs"),
                change(Some(&format!("old/{i}.rs")), &[1]),
            )
        })
        .collect();
    let paths: Vec<_> = (0..5000).map(|i| format!("unchanged/{i}.rs")).collect();
    let started = std::time::Instant::now();
    let expected: Vec<_> = paths.iter().map(|path| snapshot.includes(path)).collect();
    let reference = started.elapsed();
    let started = std::time::Instant::now();
    let scope = snapshot.scope_index();
    let actual: Vec<_> = paths.iter().map(|path| scope.includes(path)).collect();
    assert_eq!(expected, actual);
    assert!(actual.iter().all(|selected| !selected));
    assert_eq!(scope.old_paths.len(), snapshot.changes.len());
    eprintln!(
        "scope lookup smoke benchmark (5000 paths / 2000 changes): single={reference:?}, indexed={:?}",
        started.elapsed()
    );
}
