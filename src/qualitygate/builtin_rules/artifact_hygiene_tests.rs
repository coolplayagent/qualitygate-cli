use super::*;
use crate::domain::{CheckResult, ExecutionStatus, Severity, Verdict};
use crate::snapshot::{Change, File, Identity};

fn snapshot() -> Snapshot {
    Snapshot {
        root: ".".into(),
        scope_evidence: Default::default(),
        identity: Identity {
            verification_digest: None,
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "fixture".into(),
        },
        files: Default::default(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn insert(snapshot: &mut Snapshot, path: &str, bytes: Vec<u8>, kind: &str) {
    snapshot.files.insert(
        path.into(),
        File {
            bytes,
            executable: false,
        },
    );
    snapshot.changes.insert(
        path.into(),
        Change {
            kind: kind.into(),
            old_path: None,
            added_lines: Default::default(),
            removed_lines: Default::default(),
        },
    );
}

fn setting(parameters: serde_json::Value) -> RuleSetting {
    RuleSetting {
        parameters: serde_json::from_value(parameters).unwrap(),
        ..RuleSetting::default()
    }
}

fn run(input: &Snapshot, parameters: serde_json::Value) -> CheckResult {
    crate::builtin_rules::dispatch::evaluate_as(
        "no-committed-binaries",
        "artifact-hygiene",
        &setting(parameters),
        input,
    )
}

#[test]
fn suffix_and_content_findings_are_file_level_and_use_raw_byte_lengths() {
    let mut input = snapshot();
    for (path, bytes) in [
        ("lib/tool.SO", &b"\0\xFF"[..]),
        ("src/data.bin", &b"hi\0"[..]),
        ("src/no-suffix", &b"\xFF"[..]),
        ("vendor.so/file.rs", "fn café() {}".as_bytes()),
        ("lib/tool.so.1", &b"text"[..]),
        ("lib/.so", &b"text"[..]),
    ] {
        insert(&mut input, path, bytes.to_vec(), "added");
    }
    let result = run(
        &input,
        json!({"binary_extensions":[".so"],"scan_content":true}),
    );
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert_eq!(result.matched_entities, 6);
    let diagnostics: Vec<_> = result
        .diagnostics
        .iter()
        .map(|finding| {
            assert!(finding.range.is_none());
            (finding.file.as_deref().unwrap(), finding.evidence.clone())
        })
        .collect();
    assert_eq!(
        diagnostics,
        [
            (
                "lib/.so",
                json!({"kind":"binary_extension","extension":".so","bytes":4})
            ),
            (
                "lib/tool.SO",
                json!({"kind":"binary_extension","extension":".so","bytes":2})
            ),
            (
                "src/data.bin",
                json!({"kind":"binary_content","extension":".bin","bytes":3})
            ),
            (
                "src/no-suffix",
                json!({"kind":"binary_content","extension":null,"bytes":1})
            ),
        ]
    );
}

#[test]
fn only_selected_added_paths_outside_rule_exemptions_are_checked() {
    let mut input = snapshot();
    input.path_filter = Some("src".into());
    for (path, kind) in [
        ("outside.so", "added"),
        ("src/fixtures/media.so", "added"),
        ("src/old.so", "modified"),
        ("src/moved.so", "renamed"),
        ("src/deleted.so", "deleted"),
        ("src/new.so", "added"),
    ] {
        insert(&mut input, path, vec![0], kind);
    }
    let result = run(
        &input,
        json!({
            "binary_extensions":[".so"], "scan_content":true, "allow_paths":["src/fixtures/**"]
        }),
    );
    assert_eq!(result.matched_entities, 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].file.as_deref(), Some("src/new.so"));
}

#[test]
fn contents_outside_prefix_and_utf8_characters_spanning_it_are_not_binary() {
    let mut input = snapshot();
    let mut text = vec![b'a'; PREFIX_BYTES - 1];
    text.extend_from_slice("界".as_bytes());
    insert(&mut input, "valid", text.clone(), "added");
    text.truncate(PREFIX_BYTES);
    insert(&mut input, "invalid-at-eof", text, "added");
    let mut late = vec![b'a'; PREFIX_BYTES];
    late.push(0);
    insert(&mut input, "late-signal", late, "added");
    let result = run(&input, json!({"scan_content":true}));
    assert_eq!(result.matched_entities, 3);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].file.as_deref(),
        Some("invalid-at-eof")
    );
}

#[test]
fn size_threshold_is_strict_and_independent_of_binary_detection() {
    let mut input = snapshot();
    insert(&mut input, "equal.so", vec![0, 0], "added");
    insert(&mut input, "large", vec![0, 0, 0], "added");
    let size_only = run(&input, json!({"max_added_bytes":2,"scan_content":false}));
    assert_eq!(size_only.diagnostics.len(), 1);
    assert_eq!(
        size_only.diagnostics[0].evidence,
        json!({
            "kind":"oversized_added_file","extension":null,"bytes":3,"limit_bytes":2
        })
    );
    let both = run(&input, json!({"max_added_bytes":2,"scan_content":true}));
    assert_eq!(both.diagnostics.len(), 3);
    assert_eq!(both.diagnostics[1].file.as_deref(), Some("large"));
    assert_ne!(
        both.diagnostics[1].fingerprint,
        both.diagnostics[2].fingerprint
    );
}

#[test]
fn exceeding_either_artifact_budget_preserves_findings_and_blocks_completion() {
    for (limit, bytes_per_file, reason) in [
        (MAX_ADDED_FILES, 1, "added file budget"),
        (
            MAX_SCANNED_BYTES / PREFIX_BYTES,
            PREFIX_BYTES,
            "byte content scan budget",
        ),
    ] {
        let mut input = snapshot();
        insert(&mut input, "first.so", vec![0], "added");
        let text = vec![b'x'; bytes_per_file];
        for index in 1..limit {
            insert(&mut input, &format!("s{index:05}"), text.clone(), "added");
        }
        if bytes_per_file == PREFIX_BYTES {
            // The suffix finding above is not charged to the content budget.
            insert(&mut input, "s-final", text.clone(), "added");
        }
        let parameters = json!({"binary_extensions":[".so"],"scan_content":true});
        let exact = run(&input, parameters.clone());
        assert_eq!(exact.verdict, Some(Verdict::Fail));
        assert_eq!(exact.diagnostics.len(), 1);
        insert(&mut input, "z-over-budget", text, "added");
        let blocked = run(&input, parameters);
        assert_eq!(blocked.execution.status, ExecutionStatus::Blocked);
        assert_eq!(blocked.verdict, None);
        assert_eq!(blocked.matched_entities, exact.matched_entities);
        assert_eq!(blocked.diagnostics.len(), 1);
        assert!(blocked.execution.reason.unwrap().contains(reason));
    }
}

#[test]
fn expired_deadline_or_missing_file_cannot_yield_usable_evidence() {
    let mut input = snapshot();
    insert(&mut input, "input.bin", vec![0], "added");
    let mut output = BuiltinResult::new(CheckResult::pending(
        "no-committed-binaries",
        true,
        Severity::Error,
    ));
    output.expire_deadline();
    let parameters = json!({"scan_content":true});
    assert!(
        evaluate(&mut output, &setting(parameters.clone()), &input)
            .unwrap_err()
            .to_string()
            .contains("Rule analysis exceeded")
    );
    assert!(output.diagnostics.is_empty());
    input.files.clear();
    let result = run(&input, parameters);
    assert_eq!(result.verdict, None);
    assert!(
        result
            .execution
            .reason
            .unwrap()
            .contains("missing from the selected snapshot")
    );
    assert_eq!(run(&input, json!({})).verdict, None);
}
