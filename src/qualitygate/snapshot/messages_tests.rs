use super::*;
use crate::snapshot::run_git;

async fn fixture() -> (tempfile::TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "-q"], None).await.unwrap();
    let tree = run_git(root.path(), &["mktree"], Some(Vec::new()))
        .await
        .unwrap();
    (root, String::from_utf8(tree).unwrap().trim().into())
}

async fn commit(
    root: &Path,
    tree: &str,
    parent: Option<&str>,
    encoding: Option<&str>,
    message: &[u8],
) -> String {
    let mut raw = format!("tree {tree}\n");
    if let Some(parent) = parent {
        raw.push_str(&format!("parent {parent}\n"));
    }
    raw.push_str("author Audit <audit@example.invalid> 1700000000 +0000\ncommitter Audit <audit@example.invalid> 1700000000 +0000\n");
    if let Some(encoding) = encoding {
        raw.push_str(&format!("encoding {encoding}\n"));
    }
    raw.push('\n');
    let mut raw = raw.into_bytes();
    raw.extend_from_slice(message);
    let oid = run_git(
        root,
        &[
            "hash-object",
            "--literally",
            "-t",
            "commit",
            "-w",
            "--stdin",
        ],
        Some(raw),
    )
    .await
    .unwrap();
    String::from_utf8(oid).unwrap().trim().into()
}

#[tokio::test]
async fn batched_messages_preserve_git_order_encoding_and_newlines() {
    let (root, tree) = fixture().await;
    let base = commit(root.path(), &tree, None, None, b"base\n").await;
    let mut head = base.clone();
    for (encoding, message) in [
        (None, b"plain".as_slice()),
        (None, b"many\n\n"),
        (None, b"visible\0hidden\n"),
        (Some("ISO-8859-1"), b"caf\xe9\n"),
        (None, b""),
    ] {
        head = commit(root.path(), &tree, Some(&head), encoding, message).await;
    }
    // Delimiter framing must not depend on a caller's non-UTF-8 pretty output.
    run_git(
        root.path(),
        &["config", "i18n.logOutputEncoding", "UTF-16"],
        None,
    )
    .await
    .unwrap();
    let range = format!("{base}..{head}");
    let listing = run_git(root.path(), &["rev-list", &range], None)
        .await
        .unwrap();
    let mut expected = Vec::new();
    for oid in std::str::from_utf8(&listing).unwrap().lines() {
        let message = run_git(
            root.path(),
            &["show", "-s", "--encoding=UTF-8", "--format=%B", oid],
            None,
        )
        .await
        .unwrap();
        expected.push((oid.into(), String::from_utf8(message).unwrap()));
    }
    assert_eq!(capture(root.path(), &base, &head).await.unwrap(), expected);
    assert!(capture(root.path(), &head, &head).await.unwrap().is_empty());
    assert_eq!(expected[2].1, "visible\n");
}

#[tokio::test]
async fn aggregate_commit_messages_reject_over_budget_without_partial_history() {
    let (root, tree) = fixture().await;
    let base = commit(root.path(), &tree, None, None, b"base").await;
    let mut head = base.clone();
    for _ in 0..3 {
        head = commit(
            root.path(),
            &tree,
            Some(&head),
            None,
            &vec![b'x'; 3 * 1024 * 1024],
        )
        .await;
    }
    assert!(
        capture(root.path(), &base, &head)
            .await
            .unwrap_err()
            .to_string()
            .contains("8 MiB")
    );
}

#[test]
fn framing_count_and_aggregate_limits_reject_incomplete_evidence() {
    let oid = "a".repeat(40);
    let record = format!("{oid}\0one\0");
    assert_eq!(parse(record.as_bytes(), 4).unwrap()[0].1, "one\n");
    assert!(parse(record.as_bytes(), 3).is_err());
    assert!(parse(record.repeat(1001).as_bytes(), MAX_MESSAGE_BYTES).is_err());
    assert_eq!(
        parse(record.repeat(1000).as_bytes(), MAX_MESSAGE_BYTES)
            .unwrap()
            .len(),
        1000
    );
    for bytes in [
        b"bad\0message\0".to_vec(),
        format!("{oid}\0message").into_bytes(),
        format!("{oid}\0").into_bytes(),
        [format!("{oid}\0").into_bytes(), vec![255, 0]].concat(),
    ] {
        assert!(parse(&bytes, MAX_MESSAGE_BYTES).is_err());
    }
    assert!(parse(&[], 0).unwrap().is_empty());
}
