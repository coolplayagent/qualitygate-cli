use super::*;
use serde_json::{Value, json};

fn setting(parameters: Value) -> RuleSetting {
    serde_json::from_value(json!({"enabled":true,"parameters":parameters})).unwrap()
}

#[test]
fn malformed_parameter_values_cannot_silently_weaken_builtin_checks() {
    for (implementation, key, values) in [
        (
            "commit-message",
            "pattern",
            vec![json!(""), json!("x".repeat(513)), json!("["), json!(4)],
        ),
        (
            "test-naming",
            "patterns",
            vec![json!([]), json!({"rust":true}), json!({"rust":"["})],
        ),
        (
            "test-naming",
            "paths",
            vec![json!("src"), json!([1]), json!([""]), json!(["["])],
        ),
        (
            "comment-language",
            "exempt_patterns",
            vec![json!(["["]), json!([false])],
        ),
        (
            "test-naming",
            "languages",
            vec![json!(["unknown"]), json!([1])],
        ),
        (
            "diff-size",
            "max_added_lines",
            vec![json!(0), json!(-1), json!(1.5)],
        ),
        (
            "parameterized-tests",
            "minimum_similar",
            vec![json!(1), json!(false)],
        ),
        (
            "comment-language",
            "language",
            vec![json!("unknown"), json!(null)],
        ),
        (
            "ai-code-traceability",
            "provenance_scope",
            vec![json!("unknown"), json!(null)],
        ),
        (
            "ai-code-traceability",
            "marker",
            vec![
                json!({"type":"unsupported","name":"Marker"}),
                json!({"type":"comment","name":" "}),
                json!({"type":"comment","name":"Marker","fields":["bad field"]}),
            ],
        ),
        (
            "test-annotation-dependency",
            "annotation",
            vec![
                json!(""),
                json!("1Test"),
                json!("a.b"),
                json!("x".repeat(129)),
                json!(true),
            ],
        ),
        (
            "test-annotation-dependency",
            "group",
            vec![
                json!(""),
                json!("group/invalid"),
                json!("x".repeat(129)),
                json!(false),
            ],
        ),
    ] {
        for value in values {
            let mut parameters = json!({});
            parameters[key] = value.clone();
            assert!(
                validate(implementation, implementation, &[], &setting(parameters)).is_err(),
                "{implementation}.{key}={value}"
            );
        }
    }
    assert!(
        validate(
            "x",
            "line-ending",
            &[],
            &setting(json!({"misspelled":true}))
        )
        .is_err()
    );
    assert!(validate("x", "unknown", &[], &setting(json!({}))).is_err());
    for (implementation, parameters) in [
        ("commit-message", json!({"pattern":"^fix:"})),
        (
            "test-naming",
            json!({"patterns":{"rust":"^test_"},"paths":["src/**"],"languages":["rust"]}),
        ),
        (
            "comment-language",
            json!({"language":"bilingual","exempt_patterns":["license"]}),
        ),
        (
            "ai-code-traceability",
            json!({"provenance_scope":"ai_only","marker":{"type":"comment","name":"Generated","fields":["author"]}}),
        ),
        ("parameterized-tests", json!({"minimum_similar":2})),
    ] {
        validate(implementation, implementation, &[], &setting(parameters)).unwrap();
    }
}

#[test]
fn regex_maps_require_bounded_distinct_patterns_and_supported_languages() {
    for value in [
        json!({}),
        json!([]),
        json!({"ruby":["x"]}),
        json!({"rust":"x"}),
        json!({"rust":[]}),
        json!({"rust":[1]}),
        json!({"rust":[""]}),
        json!({"rust":["x","x"]}),
        json!({"rust":["["]}),
        json!({"rust":["x".repeat(513)]}),
        json!({"rust":vec!["x";33]}),
    ] {
        assert!(
            pattern_map("prohibited_patterns", &value, true).is_err(),
            "{value}"
        );
    }
    let entries: serde_json::Map<String, Value> = (0..33)
        .map(|i| (format!("lang{i}"), json!(["x"])))
        .collect();
    assert!(pattern_map("prohibited_patterns", &Value::Object(entries), true).is_err());
    assert!(pattern_map("forbidden_imports", &json!({"cpp":["x"]}), false).is_err());
    pattern_map(
        "prohibited_patterns",
        &json!({"c":["x"],"cpp":["y"],"all":["z"]}),
        true,
    )
    .unwrap();
}

#[test]
fn language_specific_rules_refuse_scope_changes_and_missing_required_contracts() {
    for (builtin, language) in [
        ("shell-fixture", "shell"),
        ("rust-fixture", "rust"),
        ("ts-fixture", "typescript"),
        ("go-fixture", "go"),
        ("py-fixture", "python"),
        ("cpp-fixture", "cpp"),
        ("java-fixture", "java"),
    ] {
        let mut parameters =
            json!({"languages":[language],"prohibited_patterns":{language:["forbidden"]}});
        validate(
            builtin,
            "source-pattern",
            &[language.into()],
            &setting(parameters.clone()),
        )
        .unwrap();
        parameters["languages"] = json!(["rust", "java"]);
        assert!(
            validate(
                builtin,
                "source-pattern",
                &[language.into()],
                &setting(parameters.clone())
            )
            .is_err()
        );
        parameters["languages"] = json!([language]);
        parameters["prohibited_patterns"] = json!({"all":["forbidden"]});
        assert!(
            validate(
                builtin,
                "source-pattern",
                &[language.into()],
                &setting(parameters)
            )
            .is_err()
        );
    }
    let both = json!({"languages":["c","cpp"],"prohibited_patterns":{"c":["x"],"cpp":["x"]}});
    validate("c-fixture", "source-pattern", &[], &setting(both.clone())).unwrap();
    for (key, value) in [
        ("languages", json!(["c"])),
        ("prohibited_patterns", json!({"c":["x"]})),
    ] {
        let mut changed = both.clone();
        changed[key] = value;
        assert!(validate("c-fixture", "source-pattern", &[], &setting(changed)).is_err());
    }
    for implementation in [
        "test-naming-strict",
        "test-annotation-dependency",
        "file-pattern",
        "native-syntax",
        "language-syntax",
        "shell-shebang",
        "shell-commented-code",
    ] {
        assert!(
            validate(
                implementation,
                implementation,
                &["java".into()],
                &setting(json!({"languages":["rust"]}))
            )
            .is_err()
        );
    }
    assert!(
        validate(
            "python-test-naming",
            "test-naming",
            &[],
            &setting(json!({"languages":["java"]}))
        )
        .is_err()
    );
    assert!(
        validate(
            "security-sensitive-api",
            "source-pattern",
            &["java".into()],
            &setting(json!({"languages":["rust"],"prohibited_patterns":{"all":["x"]}}))
        )
        .is_err()
    );
    assert!(
        validate(
            "security-sensitive-api",
            "source-pattern",
            &["java".into()],
            &setting(json!({"languages":["java"],"prohibited_patterns":{"rust":["x"]}}))
        )
        .is_err()
    );
    for (builtin, implementation, parameters) in [
        (
            "no-test-sleep",
            "source-pattern",
            json!({"prohibited_patterns":{"all":["sleep"]}}),
        ),
        ("source-pattern", "source-pattern", json!({})),
        ("import-boundary", "import-boundary", json!({})),
        (
            "test-annotation-dependency",
            "test-annotation-dependency",
            json!({"languages":["java"]}),
        ),
        ("java-files", "file-pattern", json!({"languages":["java"]})),
    ] {
        assert!(
            validate(
                builtin,
                implementation,
                &["java".into()],
                &setting(parameters)
            )
            .is_err()
        );
    }
}
