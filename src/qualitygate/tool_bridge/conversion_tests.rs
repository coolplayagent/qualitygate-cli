use super::*;

fn context() -> serde_json::Value {
    serde_json::json!({
        "spec": {"adapter":"dryer", "schema":"clone_pair", "path":"report.edn",
            "tool_id":"dryer", "tool_version":crate::tool_evidence::contracts::DRYER_VERSION,
            "scope":{"project":"fixture"}, "paths":["src/lib.rs"], "crate_name":"fixture"},
        "producer":"clones", "snapshot_digest":crate::content_digest::sha256(b"unverified snapshot"),
        "artifact_digest":"unverified caller digest", "inventory":{"files":["src/lib.rs"],"entities":[]},
        "coverage":null, "execution":{"argv":["never-executed"],"exit_code":99,"stderr":"unverified receipt",
            "tool_id":"forged","tool_version":"forged","scope_files":[]}
    })
}

#[test]
fn valid_offline_conversion_binds_raw_bytes_but_never_accepts_caller_execution_claims() {
    let root = tempfile::tempdir().unwrap();
    let raw = root.path().join("report.edn");
    let contract = root.path().join("context.json");
    let bytes = b"{:candidates []}";
    std::fs::write(&raw, bytes).unwrap();
    std::fs::write(&contract, serde_json::to_vec(&context()).unwrap()).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&convert("dryer", &raw, &contract).unwrap()).unwrap();
    assert_eq!(value["complete"], false);
    assert_eq!(value["records"], serde_json::json!([]));
    assert_eq!(
        value["artifact_digest"],
        crate::content_digest::sha256(bytes)
    );
    assert!(
        convert("crapper", &raw, &contract)
            .unwrap_err()
            .to_string()
            .contains("differs")
    );
    assert!(
        convert("mutator", &raw, &contract)
            .unwrap_err()
            .to_string()
            .contains("Unsupported")
    );
    std::fs::write(&raw, b"{:candidates [").unwrap();
    assert!(convert("dryer", &raw, &contract).is_err());
    std::fs::write(&contract, b"{}").unwrap();
    assert!(
        convert("dryer", &raw, &contract)
            .unwrap_err()
            .to_string()
            .contains("context")
    );
}

#[test]
fn reads_regular_files_with_one_shared_budget_and_rejects_oversize_or_nonfiles() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("report.edn");
    std::fs::write(&file, b"[]").unwrap();
    assert_eq!(read_regular(&file, 2).unwrap(), b"[]");
    assert!(
        read_regular(&file, 1)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert!(
        read_regular(root.path(), 100)
            .unwrap_err()
            .to_string()
            .contains("regular file")
    );
    assert!(read_regular(&root.path().join("missing"), 100).is_err());
    let oversized = root.path().join("oversized.edn");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_INPUT_BYTES as u64 + 1)
        .unwrap();
    assert!(read_regular(&oversized, MAX_INPUT_BYTES).is_err());
}

#[cfg(unix)]
#[test]
fn nonregular_socket_input_is_rejected_before_opening() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(
        read_regular(&path, MAX_INPUT_BYTES)
            .unwrap_err()
            .to_string()
            .contains("regular file")
    );
}
