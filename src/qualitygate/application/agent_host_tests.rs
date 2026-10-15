use super::*;
#[tokio::test]
async fn expired_deadline_is_typed_and_prevents_process_launch() {
    let deadline = Instant::now() - Duration::from_millis(1);
    let error = command(
        &["missing-agent-binary".into()],
        Path::new("."),
        None,
        deadline,
    )
    .await
    .unwrap_err();
    assert!(error.is::<TimeBudgetExpired>());
}
#[test]
fn budget_errors_keep_pre_agent_attempts_distinct_from_other_execution_errors() {
    let mut ledger = json!({"attempts":[{"status":"running"}]});
    record_execution_error(&mut ledger, &TimeBudgetExpired.into());
    assert_eq!(ledger["termination"], "time_budget");
    assert_eq!(
        ledger["attempts"][0]["status"],
        "budget_exhausted_before_agent"
    );
    let mut initial = json!({"attempts":[]});
    record_execution_error(&mut initial, &TimeBudgetExpired.into());
    assert_eq!(initial["termination"], "time_budget");
    assert_eq!(initial["attempts"].as_array().unwrap().len(), 0);
    record_execution_error(&mut initial, &anyhow::anyhow!("invalid report"));
    assert_eq!(initial["termination"], "execution_incomplete");
}
#[test]
fn changed_or_repeated_diagnostics_do_not_reset_the_no_progress_budget() {
    let before = BTreeSet::from(["a".into(), "b".into()]);
    assert!(progress(&before, &BTreeSet::from(["a".into()])));
    assert!(!progress(&before, &before));
    assert!(!progress(&before, &BTreeSet::from(["new".into()])));
    assert!(!progress(&BTreeSet::new(), &BTreeSet::new()));
}
#[test]
fn files_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input");
    assert!(read(&path, 10).is_err());
    std::fs::write(&path, b"12345678901").unwrap();
    assert!(read(&path, 10).is_err());
    assert_eq!(read(&path, 11).unwrap().len(), 11);
}

#[test]
fn read_only_proposals_cannot_redirect_or_overwrite_changed_inputs() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    let path = Path::new("src/lib.rs");
    std::fs::write(root.path().join(path), "old").unwrap();
    for name in [
        "../outside",
        "src/../task.yaml",
        ".git/config",
        "src/missing.rs",
    ] {
        assert!(replacement_target(root.path(), Path::new(name)).is_err());
    }
    for proposal in [
        json!({"path":"src/other.rs","content":"new"}),
        json!({"path":"src/lib.rs","content":"new","unrequested":true}),
        json!({"path":"src/lib.rs","content":"x".repeat(1024 * 1024 + 1)}),
    ] {
        assert!(apply_replacement(root.path(), path, b"old", &proposal.to_string()).is_err());
    }
    let proposal = json!({"path":"src/lib.rs","content":"new"}).to_string();
    assert!(apply_replacement(root.path(), path, b"stale", &proposal).is_err());
    let applied = apply_replacement(root.path(), path, b"old", &proposal).unwrap();
    assert_ne!(applied["before"], applied["after"]);
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), b"new");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("lib.rs", root.path().join("src/link.rs")).unwrap();
        assert!(replacement_target(root.path(), Path::new("src/link.rs")).is_err());
    }
}
