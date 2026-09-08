use super::*;

async fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "-q"], None).await.unwrap();
    run_git(root.path(), &["config", "user.name", "Fixture"], None)
        .await
        .unwrap();
    run_git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
        None,
    )
    .await
    .unwrap();
    std::fs::write(root.path().join("file.txt"), "old\n").unwrap();
    run_git(root.path(), &["add", "."], None).await.unwrap();
    run_git(root.path(), &["commit", "-qm", "initial"], None)
        .await
        .unwrap();
    root
}

#[tokio::test]
async fn staged_snapshot_uses_index_bytes_and_worktree_includes_untracked_files() {
    let root = repository().await;
    std::fs::write(root.path().join("file.txt"), "staged\n").unwrap();
    run_git(root.path(), &["add", "."], None).await.unwrap();
    std::fs::write(root.path().join("file.txt"), "working\n").unwrap();
    std::fs::write(root.path().join("new file.txt"), "untracked\n").unwrap();
    let staged = capture(root.path(), &Selection::Staged).await.unwrap();
    assert_eq!(staged.files["file.txt"].bytes, b"staged\n");
    assert!(!staged.files.contains_key("new file.txt"));
    let working = capture(
        root.path(),
        &Selection::Worktree {
            base: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(working.files["file.txt"].bytes, b"working\n");
    assert!(working.files.contains_key("new file.txt"));
    assert_ne!(
        staged.identity.content_digest,
        working.identity.content_digest
    );
    let isolated = materialize(&staged).await.unwrap();
    assert_eq!(
        std::fs::read(isolated.path().join("file.txt")).unwrap(),
        b"staged\n"
    );
}

#[tokio::test]
async fn commit_snapshots_ignore_dirty_files_and_collect_messages() {
    let root = repository().await;
    let base = resolve_commit(root.path(), "HEAD").await.unwrap();
    std::fs::write(root.path().join("file.txt"), "committed\n").unwrap();
    run_git(root.path(), &["commit", "-qam", "[ID]fix: change"], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("file.txt"), "dirty\n").unwrap();
    let snapshot = capture(
        root.path(),
        &Selection::Diff {
            base,
            head: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(snapshot.files["file.txt"].bytes, b"committed\n");
    assert!(snapshot.commits[0].1.contains("[ID]fix: change"));
    assert_eq!(snapshot.changes["file.txt"].added_lines, [1].into());
    assert!(resolve_commit(root.path(), "--help").await.is_err());
    assert!(resolve_commit(root.path(), "missing-ref").await.is_err());
}

#[test]
fn rename_is_not_new_content_and_digest_includes_path_and_mode() {
    let file = File {
        bytes: b"same\n".to_vec(),
        executable: false,
    };
    let base: BTreeMap<_, _> = [("old".into(), file.clone())].into();
    let mut head: BTreeMap<_, _> = [("new".into(), file)].into();
    let changed = changes::compare(&base, &head);
    assert_eq!(changed["new"].kind, "renamed");
    assert!(changed["new"].added_lines.is_empty());
    assert_eq!(changed.len(), 1);
    assert_ne!(content_digest(&base), content_digest(&head));
    let before = content_digest(&head);
    head.get_mut("new").unwrap().executable = true;
    assert_ne!(before, content_digest(&head));
}

#[tokio::test]
async fn deleted_files_and_path_boundaries_are_explicit() {
    let root = repository().await;
    std::fs::remove_file(root.path().join("file.txt")).unwrap();
    let snapshot = capture(
        root.path(),
        &Selection::Path {
            path: "src".into(),
            base: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(snapshot.changes["file.txt"].kind, "deleted");
    assert!(snapshot.includes("src/lib.rs"));
    assert!(!snapshot.includes("src-other/lib.rs"));
}
