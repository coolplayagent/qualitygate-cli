use super::*;

#[test]
fn parses_framework_tests_and_annotations_in_java_python_rust_go_and_typescript() {
    let cases = [
        (
            "OrderTest.java",
            "class OrderTest { @Test @AIGenerated(author=\"me\") void should_work_when_valid() { assertTrue(true); } }",
            "should_work_when_valid",
        ),
        (
            "test_order.py",
            "@pytest.mark.ai_generated(author='me')\ndef test_order():\n    assert True\n",
            "test_order",
        ),
        (
            "order.rs",
            "#[test]\nfn order_is_valid() { assert!(true); }",
            "order_is_valid",
        ),
        (
            "order_test.go",
            "package orders\nimport \"testing\"\nfunc TestOrder(t *testing.T) {}",
            "TestOrder",
        ),
        (
            "order.test.ts",
            "test('creates an order', () => { expect(1).toBe(1); });",
            "creates an order",
        ),
    ];
    for (path, source, expected) in cases {
        let view = parse(path, source.as_bytes()).unwrap().unwrap();
        assert_eq!(view.tests.len(), 1, "{path}: {view:?}");
        assert_eq!(view.tests[0].name, expected);
    }
}

#[test]
fn extracts_comments_imports_and_rejects_broken_syntax() {
    let view = parse(
        "a.py",
        b"import requests\n# explanation\ndef helper():\n    pass\n",
    )
    .unwrap()
    .unwrap();
    assert_eq!(view.comments[0].range.start_line, 2);
    assert_eq!(view.imports[0].text, "import requests");
    assert!(view.tests.is_empty());
    assert!(parse("bad.java", b"class Broken { void x( }").is_err());
    assert!(parse("unknown.txt", b"plain text").unwrap().is_none());
    assert!(parse("invalid.py", &[255]).is_err());
    assert!(
        parse("run.sh", b"#!/bin/sh\n# note\necho ok\n")
            .unwrap()
            .unwrap()
            .comments
            .len()
            >= 2
    );
}

#[test]
fn body_identity_survives_method_rename_but_shape_detects_similar_test_scenarios() {
    let before = parse("test_a.py", b"def test_old():\n    assert add(1, 2) == 3\n")
        .unwrap()
        .unwrap();
    let renamed = parse("test_a.py", b"def test_new():\n    assert add(1, 2) == 3\n")
        .unwrap()
        .unwrap();
    let similar = parse(
        "test_a.py",
        b"def test_other():\n    assert add(3, 4) == 7\n",
    )
    .unwrap()
    .unwrap();
    assert_eq!(before.tests[0].body_digest, renamed.tests[0].body_digest);
    assert_ne!(before.tests[0].body_digest, similar.tests[0].body_digest);
    assert_eq!(before.tests[0].shape_digest, similar.tests[0].shape_digest);
}
