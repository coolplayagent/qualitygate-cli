mod common;

use common::{cli, fixture, git, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "cpp-disabled-test",
        "sample.cpp",
        "TEST(Cache, DISABLED_should_return_when_ready) {}\n",
        "TEST(Cache, should_return_when_ready) {}\n",
        "replacement coverage",
    ),
    (
        "c-ignored-atomic-cas",
        "sample.c",
        "(void) atomic_compare_exchange_weak(&value, &expected, desired);\n",
        "if (!atomic_compare_exchange_weak(&value, &expected, desired)) retry();\n",
        "success and failure",
    ),
    (
        "workaround-marker",
        "sample.rs",
        "// temporary fix pending root-cause analysis\n",
        "// Root cause resolved; regression coverage exercises the corrected path.\n",
        "removal plan",
    ),
];

fn configure(root: &Path, id: &str, overrides: &str) {
    fs::write(
        root.join("qualitygate.yaml"),
        format!("schema_version: 1\nrulesets: [lang-c, lang-cpp]\nrules:\n  {id}: {overrides}\n"),
    )
    .unwrap();
}

fn check(root: &Path) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        0,
    )
}

fn rule<'a>(result: &'a Value, id: &str) -> &'a Value {
    result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn review_signals_are_discoverable_but_opt_in_and_optional_warnings() {
    let repository = fixture();
    let result = report(
        &cli(
            repository.path(),
            &["rules", "list", "--source", "builtin", "--format", "json"],
        ),
        0,
    );
    for (id, _, _, _, _) in CASES {
        let entry = result["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap();
        assert_eq!(entry["enabled"], false, "{id}");
        let defaults = &entry["definition"]["builtin"]["defaults"];
        assert_eq!(defaults["required"], false, "{id}");
        assert_eq!(defaults["severity"], "warning", "{id}");
    }
}

#[test]
fn signals_preserve_evidence_and_guidance_without_blocking_then_disappear_after_repair() {
    for (id, path, bad, good, guidance) in CASES {
        let repository = fixture();
        let root = repository.path();
        configure(root, id, "{}");
        fs::write(root.join(path), bad).unwrap();
        let failed = check(root);
        assert_eq!(failed["gate"]["complete"], true);
        assert_eq!(failed["gate"]["decision"], "pass");
        let finding = rule(&failed, id);
        assert_eq!(finding["required"], false);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["verdict"], "fail");
        assert_eq!(finding["execution"]["status"], "completed");
        assert_eq!(finding["diagnostics"].as_array().unwrap().len(), 1);
        let diagnostic = &finding["diagnostics"][0];
        assert_eq!(diagnostic["file"], *path);
        assert_eq!(diagnostic["range"]["start_line"], 1);
        assert!(
            diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("review signal")
        );
        assert!(
            diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("not proof")
        );
        assert!(diagnostic["fix"].as_str().unwrap().contains(guidance));

        fs::write(root.join(path), good).unwrap();
        let repaired = check(root);
        assert_eq!(rule(&repaired, id)["verdict"], "pass");
        assert!(
            rule(&repaired, id)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_ne!(
            failed["snapshot"]["content_digest"],
            repaired["snapshot"]["content_digest"]
        );
    }
}

#[test]
fn tracking_text_does_not_hide_disabled_tests_or_workaround_signals() {
    let repository = fixture();
    let root = repository.path();
    configure(root, "cpp-disabled-test", "{}");
    fs::write(
        root.join("tracked.cpp"),
        "// Reason: unstable service; tracking: TEST-42; replacement: should_retry.\nTEST(Cache, DISABLED_should_return_when_ready) {}\n",
    )
    .unwrap();
    let result = check(root);
    assert_eq!(
        rule(&result, "cpp-disabled-test")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        rule(&result, "cpp-disabled-test")["diagnostics"][0]["range"]["start_line"],
        2
    );

    configure(root, "workaround-marker", "{}");
    fs::write(
        root.join("tracked.cpp"),
        "// workaround: root cause BUG-42; owner: maintainer; remove after corrected release.\n",
    )
    .unwrap();
    assert_eq!(rule(&check(root), "workaround-marker")["verdict"], "fail");
}

#[test]
fn cas_signals_cover_c11_variants_in_c_family_but_not_checked_results_or_other_languages() {
    let repository = fixture();
    let root = repository.path();
    configure(root, "c-ignored-atomic-cas", "{}");
    let bad = ["weak", "strong", "weak_explicit", "strong_explicit"]
        .iter()
        .map(|variant| {
            format!("( void ) atomic_compare_exchange_{variant} (&value, &expected, desired);\n")
        })
        .collect::<String>();
    for path in ["a.c", "b.cpp", "header.h", "ignored.rs", "ignored.txt"] {
        fs::write(root.join(path), &bad).unwrap();
    }
    let result = check(root);
    let diagnostics = rule(&result, "c-ignored-atomic-cas")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 12);
    assert!(diagnostics.iter().all(|diagnostic| {
        ["c", "cpp"].contains(&diagnostic["evidence"]["language"].as_str().unwrap())
    }));

    for path in ["a.c", "b.cpp", "header.h"] {
        fs::write(
            root.join(path),
            "if (atomic_compare_exchange_weak(&value, &expected, desired)) done();\nbool changed = atomic_compare_exchange_strong_explicit(&value, &expected, desired, success, failure);\n(void) unrelated_atomic_compare_exchange_weak(&value);\n",
        )
        .unwrap();
    }
    assert_eq!(
        rule(&check(root), "c-ignored-atomic-cas")["verdict"],
        "pass"
    );
}

#[test]
fn workaround_markers_cover_english_and_chinese_in_source_but_not_documents() {
    let repository = fixture();
    let root = repository.path();
    configure(root, "workaround-marker", "{}");
    let markers = "// WORKAROUND\n// temporary   fix\n// 暂时规避\n// 临时方案\n// 临时绕过\n";
    fs::write(root.join("sample.rs"), markers).unwrap();
    fs::write(root.join("notes.md"), markers).unwrap();
    let result = check(root);
    let diagnostics = rule(&result, "workaround-marker")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 5);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["file"] == "sample.rs")
    );
    fs::write(root.join("sample.rs"), "let workaround_count = 0;\n").unwrap();
    assert_eq!(rule(&check(root), "workaround-marker")["verdict"], "pass");
}

#[test]
fn unchanged_markers_and_unselected_paths_do_not_create_findings() {
    let repository = fixture();
    let root = repository.path();
    configure(
        root,
        "cpp-disabled-test",
        "{parameters: {paths: ['src/**']}}",
    );
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/tests.cpp"), "TEST(Cache, DISABLED_old) {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "baseline marker"]);
    fs::write(
        root.join("src/tests.cpp"),
        "TEST(Cache, DISABLED_old) {}\nTEST(Cache, enabled_new) {}\n",
    )
    .unwrap();
    fs::write(
        root.join("unselected.cpp"),
        "TEST(Cache, DISABLED_new) {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/c_header.h"),
        "TEST(Cache, DISABLED_header) {}\n",
    )
    .unwrap();
    assert_eq!(rule(&check(root), "cpp-disabled-test")["verdict"], "pass");
    fs::write(
        root.join("src/tests.cpp"),
        "TEST(Cache, DISABLED_old) {}\nTEST(Cache, DISABLED_new) {}\n",
    )
    .unwrap();
    let result = check(root);
    let diagnostics = rule(&result, "cpp-disabled-test")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start_line"], 2);
}

#[test]
fn configuration_can_select_project_cas_symbols_but_rejects_invalid_scope_and_patterns() {
    let repository = fixture();
    let root = repository.path();
    configure(
        root,
        "c-ignored-atomic-cas",
        "{parameters: {prohibited_patterns: {c: ['\\(void\\) project_cas\\('], cpp: ['\\(void\\) project_cas\\(']}}}",
    );
    fs::write(root.join("sample.c"), "(void) project_cas(&value);\n").unwrap();
    assert_eq!(
        rule(&check(root), "c-ignored-atomic-cas")["verdict"],
        "fail"
    );
    for (id, invalid) in [
        ("cpp-disabled-test", "{parameters: {languages: [c]}}"),
        ("c-ignored-atomic-cas", "{parameters: {languages: [c]}}"),
        (
            "c-ignored-atomic-cas",
            "{parameters: {prohibited_patterns: {c: ['cas']}}}",
        ),
        (
            "workaround-marker",
            "{parameters: {prohibited_patterns: {all: ['[']}}}",
        ),
    ] {
        configure(root, id, invalid);
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{id}"
        );
    }
}
