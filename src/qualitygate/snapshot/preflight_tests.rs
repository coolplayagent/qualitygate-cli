use super::*;

async fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("source.rs"), b"fn source() {}\n").unwrap();
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
    root
}

fn sparse(root: &Path, name: &str, bytes: u64) {
    std::fs::File::create(root.join(name))
        .unwrap()
        .set_len(bytes)
        .unwrap();
}

#[tokio::test]
async fn metadata_preflight_recommends_capacity_without_reading_large_inputs() {
    let root = repository().await;
    let clean = inspect(root.path()).await.unwrap();
    assert!(clean.complete);
    assert_eq!(clean.oversized_file_count, 0);
    assert_eq!(clean.recommended_max_file_mib, None);
    sparse(root.path(), "large.rs", DEFAULT_FILE_BYTES as u64 + 1);
    let report = inspect(root.path()).await.unwrap();
    assert_eq!(report.oversized_file_count, 1);
    assert_eq!(report.oversized_files[0].path, "large.rs");
    assert_eq!(report.recommended_max_file_mib, Some(3));
    assert!(report.next_steps[0].contains("--snapshot-max-file-mib 3"));
    sparse(root.path(), "large.rs", MAX_FILE_BYTES as u64 + 1);
    let report = inspect(root.path()).await.unwrap();
    assert_eq!(report.recommended_max_file_mib, None);
    assert!(report.next_steps[0].contains("above the supported 8 MiB"));
    let excluded = inspect_excluding(root.path(), &["large.rs".into()])
        .await
        .unwrap();
    assert_eq!(excluded.oversized_file_count, 0);
    assert_eq!(excluded.excluded_paths, ["large.rs"]);
    assert!(
        inspect_policy(root.path(), &["large.rs".into()], &["large.rs".into()])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn preflight_retains_head_size_after_worktree_shrink_and_bounds_details() {
    let root = repository().await;
    sparse(root.path(), "history.rs", DEFAULT_FILE_BYTES as u64 + 7);
    run_git(root.path(), &["add", "history.rs"], None)
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
            "large baseline",
        ],
        None,
    )
    .await
    .unwrap();
    std::fs::write(root.path().join("history.rs"), b"small").unwrap();
    std::fs::remove_file(root.path().join("source.rs")).unwrap();
    for index in 0..101 {
        sparse(
            root.path(),
            &format!("file-{index:03}.rs"),
            DEFAULT_FILE_BYTES as u64 + 1,
        );
    }
    let report = inspect(root.path()).await.unwrap();
    assert_eq!(report.oversized_file_count, 102);
    assert!(report.details_truncated);
    assert_eq!(report.oversized_files.len(), 100);
    let report = inspect_excluding(root.path(), &["file-*.rs".into()])
        .await
        .unwrap();
    assert_eq!(report.excluded_file_count, 101);
    assert_eq!(report.excluded_paths.len(), 100);
    assert_eq!(
        report.oversized_files[0].bytes,
        DEFAULT_FILE_BYTES as u64 + 7
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_are_reported_as_unsupported_and_budget_is_not_success() {
    let root = repository().await;
    std::os::unix::fs::symlink("source.rs", root.path().join("link.rs")).unwrap();
    run_git(root.path(), &["add", "link.rs"], None)
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
            "link",
        ],
        None,
    )
    .await
    .unwrap();
    let report = inspect(root.path()).await.unwrap();
    assert!(report.unsupported_entry_count > 0);
    assert!(
        report
            .unsupported_entries
            .iter()
            .any(|s| s.contains("Git mode 120000"))
    );
    assert!(report.next_steps[0].contains("Unsupported"));
    assert!(check_budget(MAX_FILES - 1, Instant::now() + Duration::from_secs(1)).is_ok());
    assert!(check_budget(MAX_FILES, Instant::now() + Duration::from_secs(1)).is_err());
    assert!(check_budget(0, Instant::now() - Duration::from_secs(1)).is_err());
    let missing = tempfile::tempdir().unwrap();
    assert!(inspect(missing.path()).await.is_err());
}
