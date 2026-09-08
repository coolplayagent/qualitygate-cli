use super::*;

#[tokio::test]
async fn rejects_missing_executable_and_empty_argv() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        capture(&[], root.path(), None, Duration::from_secs(1))
            .await
            .is_err()
    );
    assert!(
        capture(
            &["qualitygate-no-such-executable".into()],
            root.path(),
            None,
            Duration::from_secs(1)
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn output_budget_is_enforced() {
    let mut bytes = Vec::new();
    assert!(
        read_bounded(&vec![b'x'; MAX_OUTPUT_BYTES + 1][..], &mut bytes)
            .await
            .is_err()
    );
    bytes.clear();
    read_bounded(&b"small"[..], &mut bytes).await.unwrap();
    assert_eq!(bytes, b"small");
}

#[cfg(unix)]
#[tokio::test]
async fn captures_both_streams_input_and_exit_status() {
    let root = tempfile::tempdir().unwrap();
    let output = capture(
        &[
            "sh".into(),
            "-c".into(),
            "cat; printf problem >&2; exit 7".into(),
        ],
        root.path(),
        Some(b"input".to_vec()),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert_eq!(output.stdout, b"input");
    assert_eq!(output.stderr, b"problem");
    assert_eq!(output.exit_code, Some(7));
    assert!(!output.timed_out);
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_descendants_and_does_not_wait_for_inherited_pipes_forever() {
    let root = tempfile::tempdir().unwrap();
    let output = capture(
        &[
            "sh".into(),
            "-c".into(),
            "printf started; (sleep 1; touch escaped) & wait".into(),
        ],
        root.path(),
        None,
        Duration::from_millis(40),
    )
    .await
    .unwrap();
    assert!(output.timed_out);
    assert_eq!(output.stdout, b"started");
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(!root.path().join("escaped").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_the_future_kills_descendants() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_path_buf();
    let task = tokio::spawn(async move {
        capture(
            &[
                "sh".into(),
                "-c".into(),
                "touch ready; (sleep 1; touch escaped) & wait".into(),
            ],
            &path,
            None,
            Duration::from_secs(10),
        )
        .await
    });
    for _ in 0..100 {
        if root.path().join("ready").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(root.path().join("ready").exists());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(!root.path().join("escaped").exists());
}
