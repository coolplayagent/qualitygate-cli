mod common;
#[path = "common/repository.rs"]
mod repository;
#[path = "common/reviews.rs"]
mod reviews;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str =
    "# File contracts\nKeep instructions bounded and retain the owner and gate files.\n";

fn definition(assertions: Value) -> Value {
    json!({"id":"instructions","version":1,"source":{"document":"AGENTS.md","section":"File contracts","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "requires_capabilities":["files"],"applies_to":{"paths":["instructions/*.md"]},
        "when":{"entity":"file","change":"all"},"then":assertions,
        "fix":"Repair the instruction inventory and retain the reviewed owner and gate files"})
}

fn configure(root: &Path, assertions: Value) {
    std::fs::create_dir_all(root.join("qualitygate/rules")).unwrap();
    std::fs::create_dir_all(root.join("instructions")).unwrap();
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(
        root.join("qualitygate/rules/instructions.yaml"),
        serde_norway::to_string(&definition(assertions)).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\ncustom_rules: qualitygate/rules\nrules: {instructions: {}}\n",
    )
    .unwrap();
    reviews::record(root).unwrap();
}

fn check(root: &Path, code: i32, args: &[&str]) -> Value {
    report(&repository::check(root, args), code)
}

#[test]
fn full_file_contracts_detect_unchanged_debt_missing_files_and_staged_repairs() {
    let temp = fixture();
    let root = temp.path();
    configure(
        root,
        json!({"min_count":2,"max_lines":2,"max_total_words":5,"required_paths":["owner.rs","tests/owner.rs"]}),
    );
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("owner.rs"), "pub fn owns() {}\n").unwrap();
    std::fs::write(root.join("tests/owner.rs"), "#[test] fn ownership() {}\n").unwrap();
    std::fs::write(root.join("instructions/a.md"), "one two\nthree\nfour\n").unwrap();
    std::fs::write(root.join("instructions/b.md"), "five six\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "baseline instruction debt"]);
    // No instruction changed. Full inventory still measures both files.
    let failed = check(root, 1, &[]);
    let metadata = &failed["checks"][0]["metadata"]["file_inventory"];
    assert_eq!(metadata["files"], 2);
    assert_eq!(metadata["lines"], 4);
    assert_eq!(metadata["words"], 6);
    assert_eq!(
        metadata["snapshot_digest"],
        failed["snapshot"]["content_digest"]
    );
    assert_eq!(
        failed["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    std::fs::write(root.join("instructions/a.md"), "one two\nthree\n").unwrap();
    check(root, 0, &[]);
    check(root, 1, &["--staged"]);
    std::fs::remove_file(root.join("owner.rs")).unwrap();
    let missing = check(root, 1, &[]);
    assert_eq!(
        missing["checks"][0]["diagnostics"][0]["evidence"]["assertion"],
        "required_paths"
    );
    std::fs::write(root.join("owner.rs"), "pub fn owns() {}\n").unwrap();
    std::fs::remove_file(root.join("instructions/b.md")).unwrap();
    let missing = check(root, 1, &[]);
    assert_eq!(
        missing["checks"][0]["diagnostics"][0]["evidence"]["assertion"],
        "min_count"
    );
    let scoped = check(root, 1, &["--path", "hello.txt"]);
    assert_eq!(scoped["checks"][0]["matched_entities"], 0);
    assert_eq!(scoped["scope"], "path");
    std::fs::write(root.join("instructions/b.md"), "five six\n").unwrap();
    // Trusted policy still detects a candidate relaxing the word budget.
    configure(root, json!({"min_count":2,"max_total_words":500}));
    let stale = check(root, 2, &["--policy-ref", "HEAD"]);
    assert!(!stale["policy"]["changes"].as_array().unwrap().is_empty());
}

#[test]
fn cli_file_contracts_distinguish_complete_lookup_from_delivery_findings() {
    let temp = fixture();
    let root = temp.path();
    configure(root, json!({"max_lines":1,"required_paths":["owner.rs"]}));
    std::fs::write(root.join("instructions/a.md"), "historical\nexcess\n").unwrap();
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-qm", "required path absent before delivery"],
    );
    std::fs::write(root.join("hello.txt"), "changed delivery\n").unwrap();

    let args = [
        "check",
        "--worktree",
        "--profile",
        "full",
        "--format",
        "json",
    ];
    let delivered = report(&cli(root, &args), 0);
    assert_eq!(delivered["scope"], "delivery");
    assert_eq!(delivered["checks"][0]["metadata"]["delivery_filtered"], 1);
    assert_eq!(delivered["checks"][0]["matched_entities"], 0);
    assert!(
        delivered["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // The reusable core's repository scope still enforces both obligations.
    let whole = check(root, 1, &[]);
    for assertion in ["max_lines", "required_paths"] {
        assert!(
            whole["checks"][0]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["evidence"]["assertion"] == assertion)
        );
    }

    std::fs::write(root.join("owner.rs"), "pub fn owns() {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "required path exists in baseline"]);
    std::fs::remove_file(root.join("owner.rs")).unwrap();
    std::fs::write(root.join("hello.txt"), "next delivery\n").unwrap();
    let deleted = report(&cli(root, &args), 1);
    assert_eq!(deleted["checks"][0]["diagnostics"][0]["file"], "owner.rs");

    std::fs::write(root.join("instructions/a.md"), "changed\nexcess\nlines\n").unwrap();
    let changed = report(&cli(root, &args), 1);
    assert!(
        changed["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["evidence"]["assertion"] == "max_lines")
    );

    let mut scoped = args.to_vec();
    scoped.extend(["--path", "hello.txt"]);
    let feedback = report(&cli(root, &scoped), 0);
    assert_eq!(feedback["scope"], "path");
    assert_eq!(feedback["checks"][0]["metadata"]["delivery_filtered"], 1);
}

#[test]
fn file_contracts_reject_binary_input_and_measure_unicode_words_and_empty_files() {
    let temp = fixture();
    let root = temp.path();
    configure(
        root,
        json!({"min_count":1,"max_lines":1,"max_total_words":2}),
    );
    std::fs::write(root.join("instructions/a.md"), "说明\u{2003}规则\r\n").unwrap();
    let passed = check(root, 0, &[]);
    assert_eq!(
        passed["checks"][0]["metadata"]["file_inventory"]["words"],
        2
    );
    std::fs::write(root.join("instructions/a.md"), b"\xff\xfe").unwrap();
    let incomplete = check(root, 2, &[]);
    assert!(incomplete["checks"][0]["verdict"].is_null());
    std::fs::write(root.join("instructions/a.md"), "").unwrap();
    configure(
        root,
        json!({"min_count":1,"max_count":1,"max_lines":0,"max_total_words":0}),
    );
    let empty = check(root, 0, &[]);
    assert_eq!(empty["checks"][0]["metadata"]["file_inventory"]["lines"], 0);
}

#[test]
fn file_contract_schema_and_semantics_reject_ambiguous_or_unbounded_definitions() {
    use qualitygate::config::rule_schema;
    let valid = definition(
        json!({"min_count":1,"max_lines":10,"max_total_words":100,"required_paths":["src/owner.rs"]}),
    );
    assert!(rule_schema::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for assertions in [
        json!({"min_count":3,"max_count":2}),
        json!({"required_paths":["../outside"]}),
        json!({"required_paths":["/outside"]}),
        json!({"required_paths":["."]}),
        json!({"required_paths":["a","a"]}),
        json!({"required_paths":["src/*.rs"]}),
        json!({"required_paths":vec!["a";257]}),
        json!({"max_lines":-1}),
        json!({"max_total_words":"100"}),
    ] {
        assert!(
            rule_schema::parse(&serde_json::to_vec(&definition(assertions.clone())).unwrap())
                .is_err(),
            "{assertions}"
        );
    }
    for (entity, change) in [("test_method", "all"), ("file", "any"), ("file", "added")] {
        let mut invalid = valid.clone();
        invalid["when"] = json!({"entity":entity,"change":change});
        assert!(rule_schema::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}
