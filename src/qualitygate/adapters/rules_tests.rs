use super::*;
use crate::snapshot::{Change, File, Identity, Snapshot};

fn snapshot(path: &str, base: &str, current: &str, added_lines: &[usize]) -> Snapshot {
    let file = |text: &str| File {
        bytes: text.as_bytes().to_vec(),
        executable: false,
    };
    Snapshot {
        root: ".".into(),
        identity: Identity {
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
        },
        files: [(path.into(), file(current))].into(),
        base_files: [(path.into(), file(base))].into(),
        changes: [(
            path.into(),
            Change {
                kind: "modified".into(),
                old_path: Some(path.into()),
                added_lines: added_lines.iter().copied().collect(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn setting(value: serde_json::Value) -> RuleSetting {
    RuleSetting {
        severity: Severity::Warning,
        parameters: serde_json::from_value(value).unwrap(),
        ..RuleSetting::default()
    }
}

#[test]
fn source_patterns_only_report_added_source_lines() {
    let source = "fn retained() { unsafe { retained(); } }\nfn added() { unsafe { added(); } }\n";
    let result = evaluate_as(
        "security-sensitive-api",
        "source-pattern",
        &setting(serde_json::json!({"prohibited_patterns":{"rust":["\\bunsafe\\s*\\{"]}})),
        &snapshot("src/lib.rs", source, source, &[2]),
    );
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert_eq!(result.matched_entities, 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].range.as_ref().unwrap().start_line, 2);
    assert_eq!(result.severity, Severity::Warning);
}

#[test]
fn source_pattern_configuration_gaps_remain_incomplete() {
    let result = evaluate_as(
        "security-sensitive-api",
        "source-pattern",
        &setting(serde_json::json!({"prohibited_patterns":{}})),
        &snapshot("src/lib.rs", "", "fn added() {}\n", &[1]),
    );
    assert_eq!(result.verdict, None);
    assert!(
        result
            .execution
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("requires 1..32"))
    );
}
