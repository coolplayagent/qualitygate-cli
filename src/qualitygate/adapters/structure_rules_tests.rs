use super::*;
use crate::domain::{CheckResult, ExecutionStatus, Verdict};
use crate::snapshot::{Change as FileChange, File, Identity};

fn snapshot(path: &str, old: Option<&str>, current: &str) -> Snapshot {
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
        base_files: old
            .map(|text| [(path.into(), file(text))].into())
            .unwrap_or_default(),
        changes: [(
            path.into(),
            FileChange {
                kind: if old.is_some() { "modified" } else { "added" }.into(),
                old_path: old.map(|_| path.into()),
                removed_lines: Default::default(),
                added_lines: (1..=current.lines().count()).collect(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn run(id: &str, snapshot: &Snapshot, parameters: serde_json::Value) -> CheckResult {
    run_as(id, id, snapshot, parameters)
}

fn run_as(
    id: &str,
    implementation: &str,
    snapshot: &Snapshot,
    parameters: serde_json::Value,
) -> CheckResult {
    let setting = RuleSetting {
        parameters: serde_json::from_value(parameters).unwrap(),
        ..RuleSetting::default()
    };
    crate::adapters::rules::evaluate_as(id, implementation, &setting, snapshot)
}

#[test]
fn naming_checks_new_java_tests_and_ignores_existing_violations() {
    let old = "class T { @Test void old_bad() {} }";
    let current = "class T { @Test void old_bad() {} @Test void new_bad() { check(1); } }";
    let result = run(
        "test-naming",
        &snapshot("T.java", Some(old), current),
        serde_json::json!({}),
    );
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0].message.contains("new_bad"));
    let fixed = run(
        "test-naming",
        &snapshot(
            "T.java",
            None,
            "class T { @Test void should_work_when_ready() {} }",
        ),
        serde_json::json!({}),
    );
    assert_eq!(fixed.verdict, Some(Verdict::Pass));
}

#[test]
fn source_declarations_require_explicit_binding_and_nonempty_fields() {
    let input = snapshot(
        "T.java",
        None,
        "class T { @Test @AIGenerated(author=\"\") void test() {} }",
    );
    let parameters = serde_json::json!({"marker":{"type":"annotation","name":"AIGenerated","fields":["author"]}});
    assert_eq!(
        run("ai-code-traceability", &input, serde_json::json!({})).verdict,
        None
    );
    assert_eq!(
        run("ai-code-traceability", &input, parameters.clone()).verdict,
        Some(Verdict::Fail)
    );
    let fixed = snapshot(
        "T.java",
        None,
        "class T { @Test @AIGenerated(author=\"person\") void test() {} }",
    );
    assert_eq!(
        run("ai-code-traceability", &fixed, parameters).verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn configured_import_boundaries_only_match_added_syntax_imports() {
    let result = run_as(
        "import-boundary",
        "import-boundary",
        &snapshot(
            "src/lib.rs",
            None,
            "use crate::application::internal;\npub fn api() {}\n",
        ),
        serde_json::json!({"forbidden_imports":{"rust":["^use crate::application"]}}),
    );
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert_eq!(result.matched_entities, 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].range.as_ref().unwrap().start_line, 1);
    let incomplete = run_as(
        "import-boundary",
        "import-boundary",
        &snapshot("src/lib.rs", None, "pub fn api() {}\n"),
        serde_json::json!({}),
    );
    assert_eq!(incomplete.verdict, None);
}

#[test]
fn ai_scope_and_trailer_binding_without_verified_facts_are_incomplete() {
    let input = snapshot("test_x.py", None, "def test_x():\n    pass\n");
    let scope = run(
        "ai-code-traceability",
        &input,
        serde_json::json!({"marker":{"type":"comment","name":"@ai-generated"},"provenance_scope":"ai_only"}),
    );
    assert_eq!(scope.verdict, None);
    assert_eq!(
        run(
            "ai-code-traceability",
            &input,
            serde_json::json!({"marker":{"type":"git_trailer","name":"AI-generated"}})
        )
        .verdict,
        None
    );
}

#[test]
fn detects_similar_tests_and_allows_explicit_parameterized_cases() {
    let source = "def test_a():\n    assert add(1) == 2\ndef test_b():\n    assert add(2) == 3\ndef test_c():\n    assert add(3) == 4\n";
    let result = run(
        "parameterized-tests",
        &snapshot("test_x.py", None, source),
        serde_json::json!({}),
    );
    assert_eq!(result.diagnostics.len(), 1);
    let source =
        "@pytest.mark.parametrize('x', [1,2,3])\ndef test_x(x):\n    assert add(x) == x + 1\n";
    assert_eq!(
        run(
            "parameterized-tests",
            &snapshot("test_x.py", None, source),
            serde_json::json!({})
        )
        .verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn comment_language_uses_configured_exemptions_and_does_not_require_all_comments_to_be_prose() {
    let source = "# This explains the operation\n# TODO\nx = 1\n";
    let input = snapshot("x.py", None, source);
    let result = run(
        "comment-language",
        &input,
        serde_json::json!({"language":"chinese"}),
    );
    assert_eq!(result.diagnostics.len(), 1);
    let exempt = run(
        "comment-language",
        &input,
        serde_json::json!({"language":"chinese","exempt_patterns":["This explains"]}),
    );
    assert_eq!(exempt.verdict, Some(Verdict::Pass));
    assert_eq!(
        run(
            "comment-language",
            &input,
            serde_json::json!({"language":"bilingual"})
        )
        .verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn syntax_errors_and_invalid_rule_parameters_do_not_become_pass() {
    assert_eq!(
        run(
            "test-naming",
            &snapshot("T.java", None, "class T { broken("),
            serde_json::json!({})
        )
        .verdict,
        None
    );
    assert_eq!(
        run(
            "test-naming",
            &snapshot("test_x.py", None, "def test_x():\n    pass\n"),
            serde_json::json!({"pattern":"["})
        )
        .verdict,
        None
    );
    assert_eq!(
        run(
            "parameterized-tests",
            &snapshot("test_x.py", None, "x = 1\n"),
            serde_json::json!({"minimum_similar":1})
        )
        .verdict,
        None
    );
}

#[test]
fn removing_an_existing_required_annotation_is_a_violation() {
    let input = snapshot(
        "T.java",
        Some("class T { @Test @Generated(author=\"me\") void test() {} }"),
        "class T { @Test void test() {} }",
    );
    let result = run(
        "ai-code-traceability",
        &input,
        serde_json::json!({"marker":{"type":"annotation","name":"Generated","fields":["author"]}}),
    );
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert_eq!(result.matched_entities, 1);
}

#[test]
fn declared_builtin_languages_avoid_unrelated_syntax_without_overriding_policy() {
    let input = snapshot("broken.sh", None, "if then\n");
    let config = crate::config::parse(
        b"schema_version: 1\nrules: {test-naming: {}, parameterized-tests: {}}\n",
    )
    .unwrap();
    let catalog = crate::config::catalog::Catalog::load(&config, std::iter::empty()).unwrap();
    let effective = catalog.resolve(&config).unwrap();
    for id in ["test-naming", "parameterized-tests"] {
        let result = crate::adapters::rules::evaluate(id, &effective.rules[id], &input);
        assert_eq!(result.execution.status, ExecutionStatus::Completed);
        assert_eq!(result.matched_entities, 0);
    }
    let config = crate::config::parse(
        b"schema_version: 1\nrules: {test-naming: {parameters: {languages: [shell]}}}\n",
    )
    .unwrap();
    let catalog = crate::config::catalog::Catalog::load(&config, std::iter::empty()).unwrap();
    let effective = catalog.resolve(&config).unwrap();
    assert_eq!(
        effective.rules["test-naming"].parameters["languages"],
        serde_json::json!(["shell"])
    );
    let result =
        crate::adapters::rules::evaluate("test-naming", &effective.rules["test-naming"], &input);
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
}

#[test]
fn shared_syntax_facts_preserve_cross_file_moves_and_rule_diagnostics() {
    let source = "#[test]\nfn existing_test() { assert!(true); }\n";
    let mut input = snapshot("old.rs", Some(source), "");
    input.files.remove("old.rs");
    input.files.insert(
        "new.rs".into(),
        File {
            bytes: source.as_bytes().to_vec(),
            executable: false,
        },
    );
    input.changes = crate::snapshot::compare_files(&input.base_files, &input.files);
    input.path_filter = Some("new.rs".into());
    let cache = super::super::syntax::Cache::default();
    let facts = super::super::facts::RuleFacts {
        syntax: Some(&cache),
        ..Default::default()
    };
    for id in ["test-naming", "parameterized-tests"] {
        let setting = RuleSetting::default();
        let fresh = crate::adapters::rules::evaluate(id, &setting, &input);
        let reused =
            crate::adapters::rules::evaluate_with_context(id, id, &setting, &input, &facts);
        assert_eq!(
            serde_json::to_value(fresh).unwrap(),
            serde_json::to_value(reused).unwrap()
        );
    }
    input
        .files
        .get_mut("new.rs")
        .unwrap()
        .bytes
        .extend_from_slice(b"#[test]\nfn BadName() {}\n");
    input.changes = crate::snapshot::compare_files(&input.base_files, &input.files);
    let changed = crate::adapters::rules::evaluate_with_context(
        "test-naming",
        "test-naming",
        &RuleSetting::default(),
        &input,
        &facts,
    );
    assert_eq!(changed.verdict, Some(Verdict::Fail));
    assert_eq!(changed.diagnostics.len(), 1);
    assert!(changed.diagnostics[0].message.contains("BadName"));
}
