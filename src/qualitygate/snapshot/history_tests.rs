use super::*;

fn file(bytes: &[u8]) -> File {
    File {
        bytes: bytes.into(),
        executable: false,
    }
}

#[test]
fn interning_bounds_unique_bytes_and_checks_the_shared_deadline() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let source = BTreeMap::from([("a".into(), file(b"same")), ("b".into(), file(b"same"))]);
    let first = intern(source.clone(), BTreeMap::new(), deadline, 4).unwrap();
    assert_eq!(first.added_bytes, 4);
    assert!(Arc::ptr_eq(&first.files["a"], &first.files["b"]));
    let reused = intern(source.clone(), first.pool, deadline, 0).unwrap();
    assert_eq!(reused.added_bytes, 0);
    assert!(intern(source.clone(), BTreeMap::new(), deadline, 3).is_err());
    assert!(intern(source, BTreeMap::new(), Instant::now(), usize::MAX).is_err());
    assert!(remaining(Instant::now()).is_err());
}

#[tokio::test]
async fn declaration_history_rejects_an_expired_deadline_before_git_execution() {
    let snapshot = Snapshot {
        scope_evidence: Default::default(),
        root: "/unavailable".into(),
        identity: crate::domain::SnapshotIdentity {
            mode: "diff".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: String::new(),
            verification_digest: None,
            merge_request: None,
        },
        files: BTreeMap::new(),
        base_files: BTreeMap::new(),
        changes: BTreeMap::new(),
        path_filter: None,
        commits: Vec::new(),
    };
    let error = capture_before(&snapshot, &Default::default(), Instant::now())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("60-second"));
    let error = super::super::git::run_git_before(
        snapshot.root.as_path(),
        &["--version"],
        None,
        Instant::now(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("timed out"));
}

#[tokio::test]
async fn history_deadline_covers_pending_work_and_final_completion() {
    let completed = std::sync::atomic::AtomicBool::new(false);
    let result = before_deadline(Instant::now() + Duration::from_millis(5), async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        completed.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .await;
    assert!(result.unwrap_err().to_string().contains("60-second"));
    assert!(!completed.load(std::sync::atomic::Ordering::Relaxed));
    let result = before_deadline(Instant::now() + Duration::from_millis(1), async {
        std::thread::sleep(Duration::from_millis(5));
        Ok(())
    })
    .await;
    assert!(
        result.is_err(),
        "Late final completion must not publish history"
    );
}

#[test]
fn history_decoding_charges_before_expansion_and_preserves_messages_and_trailers() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let (parents, message, input) = decode_commit(
        b"tree tree\nparent a\nparent b\n\nsubject\n\nKey: value\n".to_vec(),
        deadline,
    )
    .unwrap();
    assert_eq!(parents, ["a", "b"]);
    assert_eq!(message, "subject\n\nKey: value\n");
    assert_eq!(input, message.as_bytes());
    let trailers = decode_trailers(b"Key: first\nKEY: second\n", deadline).unwrap();
    assert_eq!(trailers["key"], ["first", "second"]);
    assert!(decode_commit(b"malformed".to_vec(), deadline).is_err());
    assert!(decode_commit(vec![255], deadline).is_err());
    assert!(decode_trailers(b"malformed", deadline).is_err());
    assert!(decode_trailers(&[255], deadline).is_err());
    assert!(decode_commit(b"tree t\n\nm".to_vec(), Instant::now()).is_err());
    assert!(decode_trailers(b"Key: v", Instant::now()).is_err());
    let mut bytes = MAX_HISTORY_BYTES - 1;
    charge(&mut bytes, 1).unwrap();
    assert!(charge(&mut bytes, 1).is_err());
    assert!(charge(&mut bytes, usize::MAX).is_err());
}

#[tokio::test]
async fn captured_history_pins_parents_reuses_blobs_and_parses_only_input_trailers() {
    let root = tempfile::tempdir().unwrap();
    super::super::run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("unchanged.rs"), b"shared\n").unwrap();
    for (name, message) in [
        ("first.rs", "first\n\nAgent-Run: run-1\nAgent-Run: run-2"),
        ("second.rs", "second\n\nReviewed-By: human"),
    ] {
        std::fs::write(root.path().join(name), name).unwrap();
        super::super::run_git(root.path(), &["add", "."], None)
            .await
            .unwrap();
        super::super::run_git(
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
                message,
            ],
            None,
        )
        .await
        .unwrap();
    }
    let snapshot = super::super::capture_with_options(
        root.path(),
        &super::super::Selection::Diff {
            base: "HEAD^".into(),
            head: "HEAD".into(),
        },
        &Default::default(),
    )
    .await
    .unwrap();
    let history = capture(&snapshot).await.unwrap();
    assert_eq!(history.commits.len(), 2);
    let first = &history.commits[0];
    let second = &history.commits[1];
    assert!(first.parents.is_empty());
    assert_eq!(second.parents.as_slice(), std::slice::from_ref(&first.oid));
    assert_eq!(first.trailers["agent-run"], ["run-1", "run-2"]);
    assert_eq!(second.trailers["reviewed-by"], ["human"]);
    assert!(Arc::ptr_eq(
        &first.files["unchanged.rs"],
        &second.files["unchanged.rs"]
    ));
    assert!(!first.files.contains_key("second.rs"));
    assert_eq!(second.content_digest, snapshot.identity.content_digest);
    let filtered = capture_with_options(
        &snapshot,
        &super::super::CaptureOptions {
            exclude: vec!["unchanged.rs".into()],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        filtered
            .commits
            .iter()
            .all(|commit| !commit.files.contains_key("unchanged.rs"))
    );
    assert_ne!(filtered.commits[1].content_digest, second.content_digest);
}
