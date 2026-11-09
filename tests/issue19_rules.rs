mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "cpp-no-realloc",
        "warning",
        "value = realloc(ptr, n);\n",
        "value = std::vector<int>(n);\n",
    ),
    (
        "cpp-no-alloca",
        "warning",
        "buf = alloca(size);\n",
        "buf = std::vector<char>(size);\n",
    ),
    (
        "cpp-no-unsafe-memfunc",
        "error",
        "strcpy(dst, src);\n",
        "std::copy(src.begin(), src.end(), dst.begin());\n",
    ),
    (
        "cpp-throw-by-value",
        "warning",
        "throw new Error();\n",
        "throw Error();\n",
    ),
    (
        "cpp-catch-by-reference",
        "warning",
        "catch (Error e) { handle(e); }\n",
        "catch (const Error& e) { handle(e); }\n",
    ),
    (
        "cpp-no-direct-mutex",
        "warning",
        "mutex.lock();\n",
        "std::lock_guard<std::mutex> guard(mutex);\n",
    ),
    (
        "cpp-no-std-move-local-return",
        "warning",
        "return std::move(value);\n",
        "return value;\n",
    ),
    (
        "cpp-no-unsafe-rand",
        "warning",
        "int n = rand();\n",
        "int n = secure_random();\n",
    ),
    (
        "cpp-no-throw-spec",
        "warning",
        "void process() throw() {}\n",
        "void process() noexcept {}\n",
    ),
    (
        "cpp-no-reinterpret-cast",
        "warning",
        "auto value = reinterpret_cast<int>(pointer);\n",
        "auto value = static_cast<int>(pointer);\n",
    ),
    (
        "cpp-no-const-cast",
        "warning",
        "auto value = const_cast<char *>(text);\n",
        "auto value = text;\n",
    ),
    (
        "cpp-no-delete-this",
        "warning",
        "void release() { delete this; }\n",
        "void release() { owner_->release(this); }\n",
    ),
    (
        "cpp-prefer-make-smart-pointer",
        "warning",
        "std::unique_ptr<Widget> widget(new Widget());\n",
        "auto widget = std::make_unique<Widget>();\n",
    ),
    (
        "cpp-use-nullptr",
        "warning",
        "Widget *widget = NULL;\n",
        "Widget *widget = nullptr;\n",
    ),
    (
        "cpp-no-macro-constant",
        "warning",
        "#define MAX_ITEMS 16\n",
        "constexpr int kMaxItems = 16;\n",
    ),
];

fn check(root: &Path, expected: i32) -> Value {
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
        expected,
    )
}

fn rule<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn cpp_rules_are_opt_in_discoverable_and_reject_language_drift() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &[
                "rules",
                "list",
                "--source",
                "builtin",
                "--language",
                "cpp",
                "--format",
                "json",
            ],
        ),
        0,
    );
    for (id, severity, _, _) in CASES {
        let entry = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap();
        assert_eq!(
            entry["definition"]["builtin"]["language"],
            serde_json::json!(["cpp"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "lang-cpp");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-realloc: {parameters: {languages: [c]}}}\n",
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-realloc: {parameters: {prohibited_patterns: {c: ['realloc']}}}}\n",
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-realloc: {parameters: {prohibited_patterns: {cpp: ['[']}}}}\n",
    ] {
        fs::write(root.join("qualitygate.yaml"), invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
fn clang_sarif_reference_is_valid_and_requires_a_real_baseline() {
    let repository = fixture();
    let root = repository.path();
    let source =
        include_str!("../skills/qualitygate-cli/references/clang-static-analyzer-ratchet.yaml");
    fs::write(root.join("qualitygate.yaml"), source).unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(0)
    );
    fs::write(
        root.join("qualitygate.yaml"),
        format!(
            "{}\n",
            source
                .lines()
                .filter(|line| !line.trim_start().starts_with("baseline:"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn cpp_source_signals_find_only_selected_changed_lines_then_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-cpp]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        fs::write(root.join("src/sample.cpp"), bad).unwrap();
        fs::write(root.join("src/other.c"), bad).unwrap();
        let failed = check(root, if *severity == "error" { 1 } else { 0 });
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
        assert_eq!(
            rule(&failed, id)["diagnostics"][0]["evidence"]["language"],
            "cpp"
        );
        fs::write(root.join("src/sample.cpp"), good).unwrap();
        assert!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id}"
        );
    }
}

#[test]
fn cpp_header_rule_scans_headers_selected_by_paths() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("include")).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-using-namespace-in-header: {}}\n",
    )
    .unwrap();
    fs::write(root.join("include/sample.h"), "using namespace std;\n").unwrap();
    fs::write(root.join("src/sample.cpp"), "using namespace std;\n").unwrap();
    let report = check(root, 0);
    let diagnostics = rule(&report, "cpp-no-using-namespace-in-header")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["file"], "include/sample.h");
    fs::write(root.join("include/sample.h"), "using std::string;\n").unwrap();
    assert!(
        rule(&check(root, 0), "cpp-no-using-namespace-in-header")["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cpp_smart_pointer_review_covers_braced_and_parenthesized_construction() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-prefer-make-smart-pointer: {}}\n",
    )
    .unwrap();
    for source in [
        "std::unique_ptr<Widget> widget{new Widget()};",
        "auto widget = std::shared_ptr<Widget>{new Widget()};",
        "std::shared_ptr<Widget> widget(new Widget());",
        "auto widget = std::unique_ptr<Widget>(new Widget());",
    ] {
        fs::write(root.join("sample.cpp"), source).unwrap();
        assert_eq!(
            rule(&check(root, 0), "cpp-prefer-make-smart-pointer")["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "{source}"
        );
    }
    for source in [
        "auto widget = std::make_unique<Widget>();",
        "std::unique_ptr<Widget> widget{factory()};",
        "std::unique_ptr<Widget> widget; auto other = new Widget();",
    ] {
        fs::write(root.join("sample.cpp"), source).unwrap();
        assert!(
            rule(&check(root, 0), "cpp-prefer-make-smart-pointer")["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{source}"
        );
    }
}

#[test]
fn cpp_review_signals_ignore_comments_and_string_literals() {
    let repository = fixture();
    let root = repository.path();
    for (id, _, bad, _) in &CASES[9..] {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-cpp]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        let source = format!("/*\n{bad}*/\nconst char *example = R\"sample(\n{bad})sample\";\n");
        fs::write(root.join("sample.cpp"), source).unwrap();
        assert!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id} must not diagnose documentation text"
        );
    }
}

#[test]
fn native_header_rule_reviews_modified_headers_and_only_changed_syntax() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-using-namespace-in-header: {}}\n",
    )
    .unwrap();
    fs::write(
        root.join("api.h"),
        "using namespace legacy;\nint old_value;\n",
    )
    .unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "baseline header"]);
    fs::write(
        root.join("api.h"),
        "using namespace legacy;\nusing\nnamespace std;\n",
    )
    .unwrap();
    let result = check(root, 0);
    let diagnostics = rule(&result, "cpp-no-using-namespace-in-header")["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start_line"], 2);
    assert_eq!(diagnostics[0]["range"]["end_line"], 3);
    assert_eq!(diagnostics[0]["evidence"]["node_kind"], "using_declaration");
    fs::write(
        root.join("api.h"),
        "using namespace legacy;\nint new_value;\n",
    )
    .unwrap();
    assert!(
        rule(&check(root, 0), "cpp-no-using-namespace-in-header")["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn native_parse_errors_are_incomplete_and_path_exclusions_do_not_parse() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-const-cast: {}}\n",
    )
    .unwrap();
    fs::write(root.join("broken.cpp"), "void f( { const_cast<int*>(p);").unwrap();
    let result = check(root, 2);
    let finding = rule(&result, "cpp-no-const-cast");
    assert_ne!(finding["execution"]["status"], "completed");
    assert!(finding["verdict"].is_null());
    fs::write(root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-const-cast: {parameters: {paths: ['include/**']}}}\n").unwrap();
    let result = check(root, 0);
    assert_eq!(
        rule(&result, "cpp-no-const-cast")["execution"]["status"],
        "completed"
    );
}

#[test]
fn native_rules_reject_regex_overrides_and_language_scope_drift() {
    let repository = fixture();
    let root = repository.path();
    for parameters in [
        "{languages: [c]}",
        "{languages: []}",
        "{prohibited_patterns: {cpp: ['anything']}}",
        "{paths: ['[']}",
    ] {
        fs::write(root.join("qualitygate.yaml"), format!(
            "schema_version: 1\nrulesets: [lang-cpp]\nrules: {{cpp-no-const-cast: {{parameters: {parameters}}}}}\n"
        )).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2),
            "{parameters}"
        );
    }
}

#[test]
fn native_syntax_ignores_comment_only_changes_and_tracks_constructor_type_changes() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-cpp]\nrules: {cpp-no-const-cast: {}, cpp-prefer-make-smart-pointer: {}}\n").unwrap();
    fs::write(
        root.join("sample.cpp"),
        "auto x = const_cast<\nint*>(p);\nauto y = factory(\nnew Widget());\n",
    )
    .unwrap();
    common::git(root, &["add", "."]);
    common::git(root, &["commit", "-qm", "baseline syntax"]);
    fs::write(root.join("sample.cpp"), "auto x = const_cast<\n// rationale only\nint*>(p);\nauto y = std::unique_ptr<Widget>(\nnew Widget());\n").unwrap();
    let result = check(root, 0);
    assert!(
        rule(&result, "cpp-no-const-cast")["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        rule(&result, "cpp-prefer-make-smart-pointer")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
