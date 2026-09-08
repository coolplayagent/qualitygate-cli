use super::*;
use test_server::{Server, json};

#[tokio::test]
async fn provider_response_body_is_subject_to_total_deadline() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 1024];
        assert!(stream.read(&mut bytes).await.unwrap() > 0);
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = stream.write_all(b"{}").await;
    });
    let error = Http::with_deadline(Duration::from_millis(100))
        .unwrap()
        .get(Url::parse(&format!("http://{address}/")).unwrap(), None)
        .await
        .err()
        .unwrap();
    assert!(
        error
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout),
        "{error:#}"
    );
    server.await.unwrap();
}

#[test]
fn mr_urls_identify_supported_providers_and_match_target_origins() {
    let github =
        merge_request::Request::parse("https://github.com/team/repo/pull/12#discussion", None)
            .unwrap();
    assert_eq!(github.repository, "team/repo");
    assert_eq!(github.number, 12);
    assert_eq!(github.url.as_str(), "https://github.com/team/repo/pull/12");
    for origin in [
        "git@github.com:Team/Repo.git",
        "ssh://git@github.com/team/repo.git",
        "https://github.com/team/repo.git",
    ] {
        assert!(github.matches_remote(origin).unwrap());
    }
    assert!(
        !github
            .matches_remote("https://github.com/team/other.git")
            .unwrap()
    );
    assert!(
        !github
            .matches_remote("https://other.invalid/team/repo.git")
            .unwrap()
    );
    assert!(
        github
            .matches_remote("https://token@github.com/team/repo.git")
            .is_err()
    );
    assert!(github.matches_remote("/tmp/repo").is_err());
    let gitlab = merge_request::Request::parse(
        "https://gitlab.example/group/sub/repo/-/merge_requests/3",
        None,
    )
    .unwrap();
    assert_eq!(gitlab.provider, "gitlab");
    assert_eq!(gitlab.repository, "group/sub/repo");
    for bad in [
        "https://github.com/team/repo/issues/3",
        "https://github.com/team/repo/pull/0",
        "https://github.com/team/repo/pull/no",
        "http://example.com/team/repo/pull/1",
        "https://secret@github.com/team/repo/pull/1",
        "https://github.com/team/a%20b/pull/1",
    ] {
        assert!(merge_request::Request::parse(bad, None).is_err(), "{bad}");
    }
    assert!(
        merge_request::Request::parse(
            "https://github.com/team/repo/pull/1",
            Some("https://attacker.invalid/")
        )
        .is_err()
    );
    assert!(
        merge_request::Request::parse(
            "https://github.com/team/repo/pull/1",
            Some("https://api.github.com/?key=bad")
        )
        .is_err()
    );
    validate_url(&Url::parse("http://[::1]:8000/").unwrap()).unwrap();
}

#[tokio::test]
async fn provider_http_bounds_responses_and_never_follows_redirects_or_echoes_error_bodies() {
    let server = Server::new(vec![
        json(serde_json::json!({"ok":true})),
        "HTTP/1.1 302 Found\r\nLocation: https://attacker.invalid/\r\nContent-Length: 0\r\n\r\n"
            .into(),
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 12\r\n\r\nsecret-token".into(),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_RESPONSE_BYTES + 1
        ),
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nno!".into(),
    ]);
    let url = Url::parse(&server.base).unwrap();
    let http = Http::new().unwrap();
    let response = http.get(url.clone(), None).await.unwrap();
    assert_eq!(response.value["ok"], true);
    assert!(response.digest.starts_with("sha256:"));
    for text in ["302", "403", "exceeds", "valid JSON"] {
        let error = http.get(url.clone(), None).await.err().unwrap().to_string();
        assert!(error.contains(text), "{error}");
        assert!(!error.contains("secret-token"));
    }
    assert!(http.get(url, Some("secret-token".into())).await.is_err());
    assert_eq!(server.requests.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn chunked_response_budgets_apply_without_a_content_length() {
    let body = "x".repeat(MAX_RESPONSE_BYTES + 1);
    let server = Server::new(vec![format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
        body.len()
    )]);
    let error = Http::new()
        .unwrap()
        .get(Url::parse(&server.base).unwrap(), None)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("exceeds"));
}

#[tokio::test]
async fn github_and_gitlab_metadata_validate_identities_and_use_current_target_heads() {
    let source = "a".repeat(40);
    let target = "b".repeat(40);
    let server = Server::new(vec![
        json(
            serde_json::json!({"number":2,"head":{"ref":"topic","sha":source},"base":{"ref":"main","sha":target,"repo":{"full_name":"team/repo"}}}),
        ),
        json(
            serde_json::json!({"iid":3,"project_id":7,"target_project_id":7,"source_branch":"topic","target_branch":"release/main","sha":source,"diff_refs":{"head_sha":source,"base_sha":"c".repeat(40),"start_sha":"d".repeat(40)}}),
        ),
        json(serde_json::json!({"name":"release/main","commit":{"id":target}})),
    ]);
    let github = merge_request::Request::parse(
        &format!("{}team/repo/pull/2", server.base),
        Some(&server.base),
    )
    .unwrap()
    .resolve()
    .await
    .unwrap();
    assert_eq!(github.source_head, source);
    assert_eq!(github.target_head, target);
    let gitlab = merge_request::Request::parse(
        &format!("{}team/sub/repo/-/merge_requests/3", server.base),
        None,
    )
    .unwrap()
    .resolve()
    .await
    .unwrap();
    assert_eq!(gitlab.target_head, target);
    assert_eq!(gitlab.api_response_digests.len(), 2);
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /repos/team/repo/pulls/2 "));
    assert!(requests[1].starts_with("GET /api/v4/projects/team%2Fsub%2Frepo/merge_requests/3 "));
    assert!(requests[2].starts_with("GET /api/v4/projects/7/repository/branches/release%2Fmain "));
}

#[tokio::test]
async fn provider_mismatches_missing_refs_and_stale_diff_refs_are_errors() {
    for response in [
        serde_json::json!({}),
        serde_json::json!({"number":99,"base":{"repo":{"full_name":"team/repo"}}}),
        serde_json::json!({"number":2,"base":{"repo":{"full_name":"other/repo"}}}),
        serde_json::json!({"number":2,"head":{"ref":"topic","sha":"not-an-oid"},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"full_name":"team/repo"}}}),
    ] {
        let server = Server::new(vec![json(response)]);
        assert!(
            merge_request::Request::parse(
                &format!("{}team/repo/pull/2", server.base),
                Some(&server.base)
            )
            .unwrap()
            .resolve()
            .await
            .is_err()
        );
    }
    let server = Server::new(vec![json(
        serde_json::json!({"iid":3,"source_branch":"topic","target_branch":"main","sha":"a".repeat(40),"diff_refs":{"head_sha":"b".repeat(40)}}),
    )]);
    let error = merge_request::Request::parse(
        &format!("{}team/repo/-/merge_requests/3", server.base),
        None,
    )
    .unwrap()
    .resolve()
    .await
    .unwrap_err();
    assert!(error.to_string().contains("stale"));
}
