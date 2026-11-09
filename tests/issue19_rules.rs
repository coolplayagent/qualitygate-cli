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
        "delete this;\n",
        "owner_->release(this);\n",
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
fn cpp_header_rule_scans_only_added_headers_selected_by_paths() {
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
