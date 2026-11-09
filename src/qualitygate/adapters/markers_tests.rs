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
        let view = super::super::syntax::parse(path, source.as_bytes())
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
    let view = super::super::syntax::parse("test_many.py", source.as_bytes())
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
