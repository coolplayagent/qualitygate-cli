use super::*;
use serde_json::json;

fn plan(checks: serde_json::Value) -> Plan {
    let config = serde_json::from_value(json!({"schema_version":1,"checks":checks})).unwrap();
    Plan::build(&config, None, "full").unwrap()
}

fn changed(path: &str, language: Option<&str>) -> ChangedInput {
    ChangedInput {
        path: path.into(),
        language: language.map(str::to_owned),
    }
}

#[test]
fn selectors_intersect_but_declared_inputs_trigger_independently() {
    let plan = plan(json!([
        {"id":"rust","argv":["git","--version"],"when_changed":{"languages":["rust"],"paths":["service/**"],"inputs":["shared/**"]}},
        {"id":"assets","argv":["git","--version"],"when_changed":{"inputs":["assets/**"]}},
        {"id":"always","argv":["git","--version"]}
    ]));
    for (path, language, rust, assets) in [
        ("service/a.rs", Some("rust"), true, false),
        ("other/a.rs", Some("rust"), false, false),
        ("service/a.py", Some("python"), false, false),
        ("shared/data.txt", None, true, false),
        ("assets/icon.txt", None, false, true),
    ] {
        let selected = select(&plan, &[changed(path, language)], None).unwrap();
        assert_eq!(selected["rust"].execute, rust);
        assert_eq!(selected["assets"].execute, assets);
        assert!(selected["always"].execute);
    }
    let selected = select(&plan, &[], Some("Repository verification")).unwrap();
    assert!(
        selected
            .values()
            .all(|s| s.execute && s.reason == "Repository verification")
    );
}

#[test]
fn dependency_closure_keeps_producers_and_does_not_activate_unrelated_consumers() {
    let plan = plan(json!([
        {"id":"producer","argv":["git","--version"],"when_changed":{"languages":["java"]}},
        {"id":"middle","argv":["git","--version"],"when_changed":{"languages":["java"]},"depends_on":["producer"]},
        {"id":"consumer","argv":["git","--version"],"depends_on":["middle"]},
        {"id":"unrelated","argv":["git","--version"],"when_changed":{"languages":["java"]},"depends_on":["producer"]}
    ]));
    let selected = select(&plan, &[], None).unwrap();
    assert_eq!(selected["producer"].required_by.as_deref(), Some("middle"));
    assert_eq!(selected["middle"].required_by.as_deref(), Some("consumer"));
    assert!(!selected["unrelated"].execute);
}

#[test]
fn selector_validation_is_strict_and_bounded() {
    for selector in [
        json!({}),
        json!({"languages":["ruby"]}),
        json!({"languages":["rust","rust"]}),
        json!({"paths":["../src/**"]}),
        json!({"inputs":["/tmp/a"]}),
        json!({"paths":["[invalid"]}),
        json!({"paths": vec!["src/**";257]}),
    ] {
        let check: CommandCheck =
            serde_json::from_value(json!({"id":"a","argv":["git"],"when_changed":selector}))
                .unwrap();
        assert!(validate(&check).is_err());
    }
    for language in [
        "java",
        "python",
        "rust",
        "typescript",
        "go",
        "shell",
        "c",
        "cpp",
    ] {
        let check = serde_json::from_value(
            json!({"id":"a","argv":["git"],"when_changed":{"languages":[language]}}),
        )
        .unwrap();
        validate(&check).unwrap();
    }
    let check = serde_json::from_value(
        json!({"id":"a","kind":"manual","when_changed":{"paths":["src/**"]}}),
    )
    .unwrap();
    assert!(validate(&check).is_err());
}

#[test]
fn common_manifest_lock_and_build_files_are_global_triggers() {
    for path in [
        "Cargo.lock",
        "crates/a/Cargo.toml",
        "crates/a/build.rs",
        "a/.cargo/config.toml",
        ".cargo/config",
        "frontend/package.json",
        "frontend/pnpm-lock.yaml",
        "frontend/tsconfig.build.json",
        "java/pom.xml",
        "gradle/libs.versions.toml",
        "backend/requirements-dev.txt",
        "pyproject.toml",
        "go.work.sum",
        "build/rules.bzl",
        "src/BUILD.bazel",
        "lib/CMakeLists.txt",
        "lib/config.cmake",
    ] {
        assert!(shared_build_input(path), "{path}");
    }
    for path in [
        "readme.md",
        "src/main.rs",
        "docs/Cargo.toml.example",
        "package.json.txt",
    ] {
        assert!(!shared_build_input(path), "{path}");
    }
}
