use super::*;
use crate::snapshot::{Change, File, Identity};
use serde_json::json;

fn snapshot(path: &str, text: &str) -> Snapshot {
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
        files: [(
            path.into(),
            File {
                bytes: text.as_bytes().into(),
                executable: false,
            },
        )]
        .into(),
        base_files: Default::default(),
        changes: [(
            path.into(),
            Change {
                kind: "added".into(),
                old_path: None,
                added_lines: (1..=text.lines().count()).collect(),
                removed_lines: Default::default(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn bounded_incomplete(result: &CheckResult) {
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert_eq!(result.verdict, None);
    assert!(result.diagnostics.len() <= MAX_DIAGNOSTICS);
    assert!(serde_json::to_vec(&result.diagnostics).unwrap().len() <= MAX_DIAGNOSTIC_BYTES);
    assert_eq!(result.metadata["diagnostic_output"]["complete"], false);
    assert_eq!(
        result.metadata["diagnostic_output"]["retained"],
        result.diagnostics.len()
    );
    assert_eq!(
        result.metadata["diagnostic_output"]["phase"],
        "rule_evaluation"
    );
}

#[test]
fn builtin_core_rejects_expanded_diagnostics_before_replay_binding() {
    // 18 KiB source previously yielded 12.9 MiB of completed diagnostic JSON.
    let input = snapshot("a.rs", &"x\n".repeat(9000));
    let mut setting = RuleSetting::default();
    setting.parameters.insert(
        "prohibited_patterns".into(),
        json!({"rust":["a?".repeat(256)]}),
    );
    let result = evaluate_as("review", "source-pattern", &setting, &input);
    bounded_incomplete(&result);
    assert!(!result.diagnostics.is_empty());
    assert!(result.diagnostics.len() < 9000);
}

#[test]
fn structural_comment_diagnostics_obey_the_shared_record_limit() {
    let input = snapshot("a.rs", &"// 中文\n".repeat(MAX_DIAGNOSTICS + 1));
    let mut setting = RuleSetting::default();
    setting
        .parameters
        .insert("language".into(), json!("english"));
    let result = evaluate("comment-language", &setting, &input);
    bounded_incomplete(&result);
    assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS);
}

#[test]
fn parallel_file_workers_share_one_budget_before_retaining_batches() {
    let mut input = snapshot("0.rs", &"x\n".repeat(1000));
    for index in 1..32 {
        input
            .files
            .insert(format!("{index}.rs"), input.files["0.rs"].clone());
        input
            .changes
            .insert(format!("{index}.rs"), input.changes["0.rs"].clone());
    }
    let mut setting = RuleSetting::default();
    setting
        .parameters
        .insert("languages".into(), json!(["rust"]));
    setting.parameters.insert("pattern".into(), json!("x"));
    let result = evaluate_as("review", "file-pattern", &setting, &input);
    bounded_incomplete(&result);
    // Failed worker batches do not become partial successful findings.
    assert!(result.diagnostics.is_empty());
}

#[test]
fn changed_line_intersection_keeps_multiline_comment_and_import_ranges() {
    let mut input = snapshot("a.rs", "/*\n中文\n*/\nuse std::{\n    io,\n    fs\n};\n");
    input.changes.get_mut("a.rs").unwrap().added_lines = [2, 5].into();
    let mut comments = RuleSetting::default();
    comments
        .parameters
        .insert("language".into(), json!("english"));
    let result = evaluate("comment-language", &comments, &input);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].range.as_ref().unwrap().start_line, 1);
    let mut imports = RuleSetting::default();
    imports
        .parameters
        .insert("forbidden_imports".into(), json!({"rust":["std"]}));
    let result = evaluate("import-boundary", &imports, &input);
    assert_eq!(result.diagnostics.len(), 1);
    input.changes.get_mut("a.rs").unwrap().added_lines = [8].into();
    assert!(
        evaluate("comment-language", &comments, &input)
            .diagnostics
            .is_empty()
    );
    assert!(
        evaluate("import-boundary", &imports, &input)
            .diagnostics
            .is_empty()
    );
}
