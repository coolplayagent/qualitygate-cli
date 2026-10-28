use super::*;
use crate::snapshot::{File, Identity};
use std::collections::BTreeMap;

fn snapshot(current: &[&str], baseline: &[&str]) -> Snapshot {
    let files = |names: &[&str]| {
        names
            .iter()
            .map(|name| {
                (
                    (*name).into(),
                    File {
                        bytes: b"source\n".to_vec(),
                        executable: false,
                    },
                )
            })
            .collect()
    };
    Snapshot {
        scope_evidence: Default::default(),
        root: "/repo".into(),
        identity: Identity {
            verification_digest: None,
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
        },
        files: files(current),
        base_files: files(baseline),
        changes: BTreeMap::new(),
        path_filter: None,
        commits: vec![],
    }
}

#[test]
fn exact_paths_precede_suffixes_and_suffixes_require_unique_components() {
    let snapshot = snapshot(
        &[
            "a.rs",
            "src/a.rs",
            "src/duplicate.rs",
            "tests/duplicate.rs",
            "lib/子/β.rs",
            "lib/tail.rs",
        ],
        &[],
    );
    let mut sources = Sources::new(&snapshot, Path::new("/checked"));
    for (query, expected) in [
        ("a.rs", "a.rs"),
        ("./src/a.rs", "src/a.rs"),
        ("tail.rs", "lib/tail.rs"),
        ("子/β.rs", "lib/子/β.rs"),
        ("β.rs", "lib/子/β.rs"),
        ("src/duplicate.rs", "src/duplicate.rs"),
    ] {
        assert_eq!(sources.map(query, false).unwrap(), expected);
    }
    for query in [
        "duplicate.rs",
        "ail.rs",
        "A.rs",
        "missing.rs",
        "../a.rs",
        ".git/config",
        "/outside/a.rs",
    ] {
        assert!(sources.map(query, false).is_err(), "{query}");
    }
    assert_eq!(sources.map("a.rs", false).unwrap(), "a.rs");
}

#[test]
fn current_and_baseline_indexes_preserve_independent_roots_and_inventories() {
    let snapshot = snapshot(
        &["current/only.rs", "current/new.rs"],
        &["base/only.rs", "base/old.rs"],
    );
    let mut sources = Sources::new(&snapshot, Path::new("/checked"));
    for _ in 0..2 {
        assert_eq!(sources.map("only.rs", false).unwrap(), "current/only.rs");
        assert_eq!(sources.map("only.rs", true).unwrap(), "base/only.rs");
        assert_eq!(
            sources.map("file:///checked/base/old.rs", true).unwrap(),
            "base/old.rs"
        );
        assert_eq!(
            sources.map("/repo/current/new.rs", false).unwrap(),
            "current/new.rs"
        );
        assert!(sources.map("old.rs", false).is_err());
        assert!(sources.map("new.rs", true).is_err());
    }
}

#[test]
fn indexed_suffix_results_match_exhaustive_resolution_across_path_shapes() {
    let mut names: Vec<_> = (0..200)
        .map(|i| format!("module/{i}/item-{}.rs", i % 19))
        .collect();
    names.extend(["子/α.rs", "別/β.rs", "outer/子/α.rs", "item-0.rs"].map(str::to_owned));
    let snapshot = snapshot(&names.iter().map(String::as_str).collect::<Vec<_>>(), &[]);
    let mut sources = Sources::new(&snapshot, Path::new("/checked"));
    for name in &names {
        for offset in std::iter::once(0).chain(name.match_indices('/').map(|(i, _)| i + 1)) {
            let query = &name[offset..];
            let candidates: Vec<_> = if snapshot.files.contains_key(query) {
                vec![query]
            } else {
                let suffix = format!("/{query}");
                names
                    .iter()
                    .filter(|name| name.ends_with(&suffix))
                    .map(String::as_str)
                    .collect()
            };
            let actual = sources.map(query, false);
            match candidates.as_slice() {
                [expected] => assert_eq!(actual.unwrap(), *expected, "{query}"),
                _ => assert!(actual.is_err(), "{query}"),
            }
        }
    }
}
