use super::*;

#[tokio::test]
async fn exact_index_and_refs_keep_sizes_separate_from_dirty_worktree_and_policy() {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("source.rs"), b"base").unwrap();
    run_git(root.path(), &["add", "source.rs"], None)
        .await
        .unwrap();
    run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit Test",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
        None,
    )
    .await
    .unwrap();
    std::fs::write(root.path().join("source.rs"), b"staged content").unwrap();
    run_git(root.path(), &["add", "source.rs"], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("source.rs"), b"different worktree content").unwrap();
    let options = CaptureOptions::default();
    let staged = inspect(root.path(), &Selection::Staged, &options, &[], Some("HEAD"))
        .await
        .unwrap();
    assert_eq!(staged.len(), 3);
    assert_eq!(staged[0].total_bytes, 4);
    assert_eq!(staged[1].total_bytes, 14);
    assert_eq!(staged[2].total_bytes, 4);
    let committed = inspect(
        root.path(),
        &Selection::Diff {
            base: "HEAD".into(),
            head: "HEAD".into(),
        },
        &options,
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(committed[1].total_bytes, 4);
    std::fs::remove_file(root.path().join("source.rs")).unwrap();
    let deleted = worktree(
        root.path(),
        Instant::now() + std::time::Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert_eq!(deleted[0].identity, "deleted");
    assert_eq!(deleted[0].bytes, 0);
    assert!(
        inspect(
            root.path(),
            &Selection::Path {
                path: "src".into(),
                base: "HEAD".into()
            },
            &options,
            &[],
            None
        )
        .await
        .is_err()
    );
    assert!(index(root.path(), Instant::now()).await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn index_conflicts_and_nonregular_worktree_entries_remain_unsupported() {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    let oid = String::from_utf8(
        run_git(
            root.path(),
            &["hash-object", "-w", "--stdin"],
            Some(b"data".to_vec()),
        )
        .await
        .unwrap(),
    )
    .unwrap()
    .trim()
    .to_string();
    let input = format!(
        "100644 {oid} 1\tconflicted.rs\n100644 {oid} 2\tconflicted.rs\n100644 {oid} 3\tconflicted.rs\n"
    );
    run_git(
        root.path(),
        &["update-index", "--index-info"],
        Some(input.into_bytes()),
    )
    .await
    .unwrap();
    let entries = index(
        root.path(),
        Instant::now() + std::time::Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 3);
    assert!(
        entries
            .iter()
            .all(|e| e.unsupported.as_deref() == Some("Unresolved index conflict"))
    );
    std::os::unix::fs::symlink("outside", root.path().join("link")).unwrap();
    std::fs::create_dir(root.path().join("conflicted.rs")).unwrap();
    let entries = worktree(
        root.path(),
        Instant::now() + std::time::Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(
        entries
            .iter()
            .find(|e| e.path == "link")
            .unwrap()
            .unsupported
            .is_some()
    );
    assert!(
        entries
            .iter()
            .find(|e| e.path == "conflicted.rs")
            .unwrap()
            .unsupported
            .is_some()
    );
}
