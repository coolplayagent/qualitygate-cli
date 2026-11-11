use super::*;
use crate::snapshot::{Change, File, Identity, Snapshot};

fn snapshot(path: &str, base: &str, current: &str, added_lines: &[usize]) -> Snapshot {
    let file = |text: &str| File {
        bytes: text.as_bytes().to_vec(),
        executable: false,
    };
    Snapshot {
        scope_evidence: Default::default(),
        root: ".".into(),
        identity: Identity {
            verification_digest: None,
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
                removed_lines: Default::default(),
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
fn packaged_review_signals_retain_actionable_evidence_without_claiming_semantic_proof() {
    let cases = [
        (
            include_str!(
                "../../../skills/qualitygate-cli/references/rules/lang-cpp/cpp-disabled-test.yaml"
            ),
            "test.cpp",
            "// Reason: instability; issue: TEST-42; replacement: retries.\nTEST(Cache, DISABLED_ready) {}\n",
            2,
            "replacement coverage",
        ),
        (
            include_str!(
                "../../../skills/qualitygate-cli/references/rules/lang-c/c-ignored-atomic-cas.yaml"
            ),
            "state.c",
            "(void) atomic_compare_exchange_strong_explicit(&value, &expected, desired, success, failure);\n",
            1,
            "success and failure",
        ),
        (
            include_str!(
                "../../../skills/qualitygate-cli/references/rules/shared/workaround-marker.yaml"
            ),
            "lib.rs",
            "// workaround: issue BUG-42; owner maintainer; remove after corrected release.\n",
            1,
            "removal plan",
        ),
    ];
    for (yaml, path, source, line, expected_guidance) in cases {
        let rule: crate::config::catalog::Builtin = serde_norway::from_str(yaml).unwrap();
        let mut input = snapshot(path, "", source, &[line]);
        let result = evaluate_as(&rule.id, &rule.implementation, &rule.defaults, &input);
        assert_eq!(result.execution.status, ExecutionStatus::Completed);
        assert_eq!(result.verdict, Some(Verdict::Fail));
        assert!(!result.required);
        assert_eq!(result.severity, Severity::Warning);
        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.file.as_deref(), Some(path));
        assert_eq!(diagnostic.range.as_ref().unwrap().start_line, line);
        assert!(diagnostic.message.contains("review signal, not proof"));
        assert!(diagnostic.fix.contains(expected_guidance));
        assert!(diagnostic.evidence["pattern"].is_string());

        // Optionality does not turn unavailable source evidence into a pass.
        input.files.clear();
        let unavailable = evaluate_as(&rule.id, &rule.implementation, &rule.defaults, &input);
        assert_eq!(unavailable.execution.status, ExecutionStatus::Blocked);
        assert_eq!(unavailable.verdict, None);
    }
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

#[test]
fn source_pattern_scope_handles_deletions_unselected_paths_and_missing_or_binary_inputs() {
    use serde_json::json;
    let selected = setting(
        json!({"prohibited_patterns":{"rust":["unsafe"]},"paths":["src/**"],"languages":["rust"]}),
    );
    for path in ["tests/test.rs", "src/readme.txt"] {
        let result = evaluate_as(
            "sensitive",
            "source-pattern",
            &selected,
            &snapshot(path, "", "unsafe", &[1]),
        );
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.execution.status, ExecutionStatus::Completed);
    }
    let mut input = snapshot("src/lib.rs", "old", "unsafe", &[1]);
    input.files.clear();
    assert_eq!(
        evaluate_as("sensitive", "source-pattern", &selected, &input)
            .execution
            .status,
        ExecutionStatus::Blocked
    );
    input.changes.get_mut("src/lib.rs").unwrap().kind = "deleted".into();
    assert_eq!(
        evaluate_as("sensitive", "source-pattern", &selected, &input)
            .execution
            .status,
        ExecutionStatus::Completed
    );
    let mut input = snapshot("src/lib.rs", "old", "unsafe", &[1]);
    input.files.get_mut("src/lib.rs").unwrap().bytes = vec![0xff];
    assert_eq!(
        evaluate_as("sensitive", "source-pattern", &selected, &input)
            .execution
            .status,
        ExecutionStatus::Blocked
    );
    for params in [
        json!({"prohibited_patterns":{"unknown":["unsafe"]}}),
        json!({"prohibited_patterns":{"rust":[]}}),
        json!({"prohibited_patterns":{"rust":["same","same"]}}),
        json!({"prohibited_patterns":{"rust":["["]}}),
        json!({"prohibited_patterns":{"rust":["unsafe"]},"paths":["["]}),
    ] {
        let result = evaluate_as(
            "sensitive",
            "source-pattern",
            &setting(params),
            &snapshot("src/lib.rs", "", "unsafe", &[1]),
        );
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    }
    let result = evaluate_as(
        "missing",
        "unknown-adapter",
        &RuleSetting::default(),
        &snapshot("src/lib.rs", "", "unsafe", &[1]),
    );
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    let provenance = RuleSetting {
        provenance: Some(serde_json::from_value(json!({"evidence_file":"origin.json"})).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        evaluate_as(
            "line-ending",
            "line-ending",
            &provenance,
            &snapshot("src/lib.rs", "", "new", &[1])
        )
        .execution
        .status,
        ExecutionStatus::Blocked
    );
}
