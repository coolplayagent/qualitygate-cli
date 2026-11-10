use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    run_git(root.path(), &["init", "-q", "--template="], None)
        .await
        .unwrap();
    run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ],
        None,
    )
    .await
    .unwrap();
    root
}

#[tokio::test]
async fn provider_heads_require_matching_origin_and_complete_local_ancestry() {
    let root = repository().await;
    let root = root.path();
    let base = resolve_commit(root, "HEAD").await.unwrap();
    std::fs::write(root.join("change.txt"), "new behavior").unwrap();
    run_git(root, &["add", "."], None).await.unwrap();
    run_git(
        root,
        &[
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "change",
        ],
        None,
    )
    .await
    .unwrap();
    let head = resolve_commit(root, "HEAD").await.unwrap();
    run_git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/other/repo.git",
        ],
        None,
    )
    .await
    .unwrap();
    assert!(
        resolve(root, "https://example.invalid/team/repo/pull/1", None)
            .await
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
    run_git(
        root,
        &[
            "remote",
            "set-url",
            "origin",
            "http://127.0.0.1/team/repo.git",
        ],
        None,
    )
    .await
    .unwrap();
    for shallow in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body=serde_json::json!({"number":1,"head":{"ref":"feature","sha":head},"base":{"ref":"main","sha":base,"repo":{"full_name":"team/repo"}}}).to_string();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 8192];
            let received = stream.read(&mut request).await.unwrap();
            assert!(received > 0);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        if shallow {
            std::fs::write(root.join(".git/shallow"), format!("{base}\n")).unwrap();
        }
        let result = resolve(
            root,
            &format!("http://{address}/team/repo/pull/1"),
            Some(&format!("http://{address}/")),
        )
        .await;
        server.await.unwrap();
        if shallow {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("complete Git ancestry")
            );
        } else {
            let result = result.unwrap();
            assert_eq!(result.merge_base, base);
            assert_eq!(result.source_head, head);
            assert_eq!(result.api_response_digests.len(), 1);
            std::fs::write(root.join("change.txt"), "uncommitted mutation").unwrap();
            let captured = crate::snapshot::capture_resolved(
                root,
                &crate::snapshot::Selection::MergeRequest {
                    url: result.url.clone(),
                    api_base: None,
                },
                &crate::snapshot::CaptureOptions::default(),
                Some(result.clone()),
            )
            .await
            .unwrap();
            assert_eq!(captured.identity.mode, "mr");
            assert_eq!(captured.identity.base, base);
            assert_eq!(captured.identity.head, head);
            assert_eq!(captured.files["change.txt"].bytes, b"new behavior");
            assert_eq!(
                captured.identity.merge_request.unwrap().source_head,
                result.source_head
            );
        }
    }
}

#[tokio::test]
async fn fetching_missing_objects_does_not_move_local_branch_or_write_fetch_head() {
    let remote = repository().await;
    let local = tempfile::tempdir().unwrap();
    run_git(local.path(), &["init", "-q", "--template="], None)
        .await
        .unwrap();
    run_git(
        local.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
        None,
    )
    .await
    .unwrap();
    let head = resolve_commit(remote.path(), "HEAD").await.unwrap();
    ensure_objects(local.path(), &head, &head).await.unwrap();
    assert!(!local.path().join(".git/FETCH_HEAD").exists());
    assert!(resolve_commit(local.path(), "HEAD").await.is_err());
    assert_eq!(resolve_commit(local.path(), &head).await.unwrap(), head);
    assert!(
        ensure_objects(remote.path(), "HEAD", "HEAD")
            .await
            .unwrap_err()
            .to_string()
            .contains("identities")
    );
}
