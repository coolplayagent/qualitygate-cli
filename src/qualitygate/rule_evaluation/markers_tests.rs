use super::*;

fn comment_marker() -> Marker {
    Marker {
        kind: "comment".into(),
        name: "AI-GENERATED".into(),
        fields: Vec::new(),
    }
}

#[test]
fn indexed_comment_binding_preserves_adjacency_and_marker_boundaries() {
    for (path, source, expected) in [
        (
            "test_sample.py",
            "# AI-GENERATED: author=robot\n\n# detail\n\ndef test_a(): pass\n# after\n",
            Some("# AI-GENERATED: author=robot\n# detail"),
        ),
        (
            "test_sample.py",
            "# AI-GENERATED\nx = 1\ndef test_a(): pass\n",
            None,
        ),
        (
            "test_sample.py",
            "# AI-GENERATED\n@mark\ndef test_a(): pass\n",
            Some("# AI-GENERATED"),
        ),
        (
            "test_sample.py",
            "# AI-GENERATED-other\ndef test_a(): pass\n",
            None,
        ),
        (
            "test_sample.py",
            "def test_a(): pass\n# AI-GENERATED\n",
            None,
        ),
        (
            "a.rs",
            "/* AI-GENERATED: author=robot\n * detail\n */\n\n#[test]\nfn a() {}\n",
            Some("/* AI-GENERATED: author=robot\n * detail\n */"),
        ),
        (
            "a.rs",
            "// AI-GENERATED\nfn helper() {}\n#[test]\nfn a() {}\n",
            None,
        ),
        (
            "a.test.ts",
            "// AI-GENERATED(author=robot)\n\u{2003}\ntest('a', () => {});\n",
            Some("// AI-GENERATED(author=robot)"),
        ),
    ] {
        let view = crate::adapters::syntax::parse(path, source.as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(view.tests.len(), 1, "{path}: {source}");
        let declaration = from_source(&comment_marker(), &view.tests[0], &view).unwrap();
        assert_eq!(declaration.as_deref(), expected, "{path}: {source}");
    }
}

#[test]
fn marker_comments_remain_bound_to_their_own_entities_in_large_files() {
    let source: String = (0..2000)
        .map(|index| format!("# AI-GENERATED: author=robot-{index}\ndef test_{index}(): pass\n\n"))
        .collect();
    let view = crate::adapters::syntax::parse("test_many.py", source.as_bytes())
        .unwrap()
        .unwrap();
    assert_eq!(view.tests.len(), 2000);
    for (index, entity) in view.tests.iter().enumerate() {
        assert_eq!(
            from_source(&comment_marker(), entity, &view).unwrap(),
            Some(format!("# AI-GENERATED: author=robot-{index}"))
        );
    }
}

#[test]
fn declared_fields_reject_quoted_impersonation_duplicates_and_truncated_values() {
    let parsed = fields(
        r#"description="author='attacker'" author = 'reviewer' model: gpt-6; task.id=42, _scope=unit / ignored trailing="unterminated"#,
    );
    assert_eq!(parsed.get("author").map(String::as_str), Some("reviewer"));
    assert_eq!(parsed.get("model").map(String::as_str), Some("gpt-6"));
    assert_eq!(parsed.get("task.id").map(String::as_str), Some("42"));
    assert_eq!(parsed.get("_scope").map(String::as_str), Some("unit"));
    assert_eq!(parsed.get("trailing").map(String::as_str), Some(""));
    for input in [
        "author=a author=b",
        "author='' author=b",
        "author=a author=''",
        "author=a author=\"unterminated",
    ] {
        assert_eq!(
            fields(input).get("author").map(String::as_str),
            Some(""),
            "{input}"
        );
    }
    assert!(fields("'author=attacker' incomplete:   ").is_empty());
    assert_eq!(
        fields(r#"description="quoted \" author=fake" author=real"#)["author"],
        "real"
    );
}

#[test]
fn annotation_binding_cannot_substitute_unverified_commit_or_unknown_marker_kinds() {
    let view = crate::adapters::syntax::parse(
        "test_sample.py",
        b"@generated(author='reviewer')\ndef test_sample(): pass\n",
    )
    .unwrap()
    .unwrap();
    let mut marker = Marker {
        kind: "annotation".into(),
        name: "generated".into(),
        fields: vec![],
    };
    assert_eq!(
        bound_declaration(
            &marker,
            &view.tests[0],
            &view,
            "test_sample.py",
            false,
            None
        )
        .unwrap()
        .as_deref(),
        Some("generated(author='reviewer')")
    );
    marker.kind = "git_trailer".into();
    assert!(
        bound_declaration(&marker, &view.tests[0], &view, "test_sample.py", true, None)
            .unwrap_err()
            .to_string()
            .contains("unavailable")
    );
    assert!(
        from_source(&marker, &view.tests[0], &view)
            .unwrap_err()
            .to_string()
            .contains("provenance is required")
    );
    marker.kind = "invented".into();
    assert!(
        from_source(&marker, &view.tests[0], &view)
            .unwrap_err()
            .to_string()
            .contains("Unsupported")
    );
}

#[test]
fn framework_discovery_distinguishes_annotations_signatures_and_non_test_calls() {
    for (path, source, names) in [
        (
            "Sample.java",
            "class Sample { @org.junit.jupiter.api.ParameterizedTest public void accepts(final String name, int... values) { /* note */ check(name); } void helper() {} }",
            vec!["accepts"],
        ),
        (
            "test_sample.py",
            "@framework.generated\ndef test_case(): pass\n@helper\ndef support(): pass\n",
            vec!["test_case"],
        ),
        (
            "sample.rs",
            "#[rstest::rstest]\n#[allow(unused)]\nfn scenario() {}\nfn helper() {}",
            vec!["scenario"],
        ),
        (
            "sample_test.go",
            "package sample\nimport \"testing\"\nfunc TestMain(m *testing.M) {}\nfunc Testlower(t *testing.T) {}\nfunc TestHelper() {}\nfunc TestCase(t *testing.T) {}",
            vec!["TestCase"],
        ),
        (
            "sample.test.ts",
            "test.each('case', () => {}); test(variable, () => {}); test(); other('unrelated');",
            vec!["case"],
        ),
        (
            "production.ts",
            "test('not a framework file', () => {});",
            vec![],
        ),
    ] {
        let view = crate::adapters::syntax::parse(path, source.as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(
            view.tests
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            names,
            "{path}"
        );
        if path.ends_with("java") {
            assert!(
                view.tests[0].symbol.contains("String,int"),
                "{}",
                view.tests[0].symbol
            );
        }
        if path.ends_with("test.ts") {
            assert_eq!(view.tests[0].annotations["parameterized"], "test.each");
        }
    }
}
