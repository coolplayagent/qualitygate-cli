use super::*;

#[cfg(unix)]
#[path = "portable.rs"]
mod portable;

fn rule(expression: Value) -> Value {
    let mut value = candidate(expression);
    value["schema_version"] = json!(3);
    value["language"] = json!([]);
    value["requires_capabilities"] = json!(["files"]);
    value["when"] = json!({"entity":"file","change":"any"});
    value["applies_to"] = json!({"paths":["src/**/*.rs"]});
    value["sources"] = json!({"tests":{"when":{"entity":"file","change":"all"},"applies_to":{"paths":["tests/**/*.rs"]},"requires_capabilities":["files"]}});
    value
}
fn correspondence() -> Value {
    let expected = json!({"op":"map","input":{"op":"entities"},"as":"source","body":{"op":"replace","value":{"op":"replace","value":{"op":"field","name":"path","of":{"op":"var","name":"source"}},"from":"src/","to":"tests/"},"from":".rs","to":"_test.rs"}});
    json!({"op":"subset","left":{"op":"distinct","input":expected},"right":{"op":"map","input":{"op":"source","name":"tests"},"as":"test","body":{"op":"field","name":"path"}}})
}
fn install(root: &Path, value: &Value) {
    std::fs::write(
        root.join("qualitygate/rules/test-contract.yaml"),
        serde_norway::to_string(value).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
}
#[test]
fn v3_multisource_rules_include_unchanged_tests_and_bind_selected_snapshots() {
    let dir = setup();
    let root = dir.path();
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(root.join("tests/a_test.rs"), "#[test] fn test_a() {}\n").unwrap();
    install(root, &rule(correspondence()));
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "baseline"]);
    std::fs::write(root.join("src/a.rs"), "fn a() { }\n").unwrap();
    let pass = check(root, 0);
    let expression = &pass["checks"][0]["metadata"]["expression"];
    assert_eq!(expression["sources"]["tests"], 1);
    assert_eq!(expression["sources"]["entities"], 1);
    let scoped = report(
        &cli(root, &["check", "--path", "src", "--format", "json"]),
        0,
    );
    assert_eq!(
        scoped["checks"][0]["metadata"]["expression"]["sources"]["tests"],
        1
    );
    git(root, &["add", "src/a.rs"]);
    let staged = report(&cli(root, &["check", "--staged", "--format", "json"]), 0);
    std::fs::remove_file(root.join("tests/a_test.rs")).unwrap();
    report(&cli(root, &["check", "--staged", "--format", "json"]), 0);
    let failed = check(root, 1);
    assert_ne!(
        staged["snapshot"]["content_digest"],
        failed["snapshot"]["content_digest"]
    );
    assert_eq!(
        failed["checks"][0]["metadata"]["expression"]["sources"]["tests"],
        0
    );
    std::fs::create_dir_all(root.join("src/sub")).unwrap();
    std::fs::write(root.join("src/sub/a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(root.join("tests/a_test.rs"), "#[test] fn test_a() {}\n").unwrap();
    check(root, 1); // Same basename in another directory is not a corresponding test.
    std::fs::create_dir(root.join("tests/sub")).unwrap();
    std::fs::write(root.join("tests/sub/a_test.rs"), "#[test] fn test_a() {}\n").unwrap();
    check(root, 0);
    std::fs::rename(root.join("src/sub/a.rs"), root.join("src/sub/b.rs")).unwrap();
    check(root, 1);
    std::fs::rename(
        root.join("tests/sub/a_test.rs"),
        root.join("tests/sub/b_test.rs"),
    )
    .unwrap();
    check(root, 0);
    std::fs::write(root.join("AGENTS.md"), "# Tests\nChanged requirement\n").unwrap();
    check(root, 2);
}

#[test]
fn v3_file_correspondence_scales_without_relaxing_shared_budgets() {
    let dir = setup();
    let root = dir.path();
    std::fs::create_dir(root.join("tests")).unwrap();
    install(root, &rule(correspondence()));
    for index in 0..1000 {
        std::fs::write(root.join(format!("src/f{index}.rs")), "fn f() {}\n").unwrap();
        std::fs::write(
            root.join(format!("tests/f{index}_test.rs")),
            "#[test] fn test_f() {}\n",
        )
        .unwrap();
    }
    let passed = check(root, 0);
    let expression = &passed["checks"][0]["metadata"]["expression"];
    assert_eq!(expression["sources"]["entities"], 1000);
    assert_eq!(expression["sources"]["tests"], 1000);
    assert!(expression["operations"].as_u64().unwrap() < 100_000);
    assert!(expression["scanned_bytes"].as_u64().unwrap() < 32 * 1024 * 1024);
    assert!(expression["materialized_bytes"].as_u64().unwrap() < 32 * 1024 * 1024);

    std::fs::remove_file(root.join("tests/f999_test.rs")).unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["gate"]["complete"], true);
    assert_eq!(
        failed["checks"][0]["metadata"]["expression"]["passed"],
        false
    );
    assert_eq!(
        failed["checks"][0]["metadata"]["expression"]["sources"]["tests"],
        999
    );
    std::fs::write(root.join("tests/f999_test.rs"), "#[test] fn test_f() {}\n").unwrap();
    check(root, 0);
}
#[test]
fn v3_authoring_schemas_and_mixed_packages_preserve_legacy_protocols() {
    let dir = setup();
    let root = dir.path();
    let value = rule(correspondence());
    let schema = report(&cli(root, &["schema", "operator-rule-v3"]), 0);
    assert_eq!(
        schema,
        serde_json::from_str::<Value>(include_str!(
            "../../skills/qualitygate-cli/references/schemas/operator-rule-v3.schema.json"
        ))
        .unwrap()
    );
    assert!(
        jsonschema::draft202012::new(&schema)
            .unwrap()
            .is_valid(&value)
    );
    write_candidate(root, &value);
    let validation = report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        0,
    );
    assert_eq!(
        validation["files"][0]["schema_id"],
        "urn:qualitygate:project-rule:3"
    );
    let mut new = value.clone();
    new["id"] = json!("correspondence");
    write_candidate(root, &new);
    let generated = report(
        &cli(
            root,
            &[
                "rules",
                "generate",
                "--input",
                "candidate.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(generated["schema_id"], "urn:qualitygate:project-rule:3");
    let mixed = report(
        &cli(
            root,
            &["rules", "validate", "qualitygate/rules", "--format", "json"],
        ),
        0,
    );
    assert_eq!(mixed["files"].as_array().unwrap().len(), 2);
    let family = report(&cli(root, &["schema", "project-rule-family"]), 0);
    assert_eq!(
        mixed["schema_digest"],
        qualitygate::snapshot::digest(&serde_json::to_vec(&family).unwrap())
    );
    for version in [1, 2] {
        let mut invalid = value.clone();
        invalid["schema_version"] = json!(version);
        write_candidate(root, &invalid);
        report(
            &cli(
                root,
                &["rules", "validate", "candidate.yaml", "--format", "json"],
            ),
            1,
        );
    }
    for (pointer, bad) in [
        ("/sources/tests/when/entity", json!("test_method")),
        ("/sources/tests/requires_capabilities", json!([])),
        ("/expression/left/input/as", json!("bad name")),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(pointer).unwrap() = bad;
        write_candidate(root, &invalid);
        report(
            &cli(
                root,
                &["rules", "validate", "candidate.yaml", "--format", "json"],
            ),
            1,
        );
    }
    let capabilities = report(&cli(root, &["capabilities", "--format", "json"]), 0);
    assert!(
        capabilities["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("rules.operators.v3"))
    );
}
#[test]
fn v3_unused_sources_require_complete_parsing_and_project_evidence() {
    let dir = setup();
    let root = dir.path();
    let mut value = rule(json!({"op":"bool","value":true}));
    value["sources"]["syntax"] = json!({"language":["rust"],"when":{"entity":"test_method"},"requires_capabilities":["test_methods"],"applies_to":{"paths":["src/**"]}});
    install(root, &value);
    std::fs::write(root.join("src/a.rs"), "#[test] fn broken( {\n").unwrap();
    let incomplete = check(root, 2);
    assert!(incomplete["checks"][0]["verdict"].is_null());
    std::fs::write(root.join("src/a.rs"), "#[test] fn test_a() {}\n").unwrap();
    check(root, 0);
    value["sources"]["syntax"]["requires_capabilities"] =
        json!(["test_methods", "dependency_resolution"]);
    install(root, &value);
    assert!(check(root, 2).to_string().contains("dependency_resolution"));
}
#[test]
fn v3_exhaustion_preserves_fixed_violations_and_policy_changes_need_new_review() {
    let dir = setup();
    let root = dir.path();
    let nested = json!({"op":"all","input":{"op":"entities"},"predicate":{"op":"all","input":{"op":"entities"},"predicate":{"op":"bool","value":true}}});
    let mut value = rule(nested);
    value["then"] = json!({"min_count":400});
    install(root, &value);
    for index in 0..300 {
        std::fs::write(root.join(format!("src/f{index}.rs")), "fn f() {}\n").unwrap();
    }
    let incomplete = check(root, 2);
    let result = &incomplete["checks"][0];
    assert!(
        result["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("operations")
    );
    assert_eq!(
        result["diagnostics"][0]["evidence"]["assertion"],
        "min_count"
    );
    value["expression"] = json!({"op":"bool","value":true});
    value["then"] = json!({});
    std::fs::write(
        root.join("qualitygate/rules/test-contract.yaml"),
        serde_norway::to_string(&value).unwrap(),
    )
    .unwrap();
    check(root, 2);
    reviews::record(root).unwrap();
    check(root, 0);
}

#[test]
fn packaged_correspondence_example_is_source_bound_and_executable() {
    let dir = fixture();
    let root = dir.path();
    let document = "skills/qualitygate-cli/references/operators-v3.md";
    std::fs::create_dir_all(root.join("skills/qualitygate-cli/references")).unwrap();
    std::fs::write(
        root.join(document),
        include_str!("../../skills/qualitygate-cli/references/operators-v3.md"),
    )
    .unwrap();
    std::fs::write(
        root.join(".gitattributes"),
        include_str!("../../.gitattributes"),
    )
    .unwrap();
    // Reproduce Windows-style checkout conversion on every platform. The
    // packaged source hash remains a byte-exact contract, including line endings.
    git(root, &["config", "core.autocrlf", "true"]);
    git(root, &["add", ".gitattributes", document]);
    std::fs::remove_file(root.join(document)).unwrap();
    git(root, &["checkout", "--", document]);
    assert_eq!(
        std::fs::read(root.join(document)).unwrap(),
        include_bytes!("../../skills/qualitygate-cli/references/operators-v3.md")
    );
    let example =
        include_str!("../../skills/qualitygate-cli/references/examples/operators-v3.yaml");
    std::fs::write(root.join("candidate.yaml"), example).unwrap();
    let value: Value = serde_norway::from_str(example).unwrap();
    let bound = report(
        &cli(
            root,
            &[
                "rules",
                "source",
                "--document",
                document,
                "--section",
                "File correspondence",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(value["source"], bound["source"]);
    report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        0,
    );
    report(
        &cli(
            root,
            &[
                "rules",
                "generate",
                "--input",
                "candidate.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: []\ncustom_rules: qualitygate/rules\nrules: {source-test-correspondence: {}}\n").unwrap();
    reviews::record(root).unwrap();
    std::fs::create_dir_all(root.join("src/nested")).unwrap();
    std::fs::write(root.join("src/nested/widget.rs"), "fn widget() {}\n").unwrap();
    check(root, 1);
    std::fs::create_dir_all(root.join("tests/nested")).unwrap();
    std::fs::write(
        root.join("tests/nested/widget_test.rs"),
        "#[test] fn widget_test() {}\n",
    )
    .unwrap();
    check(root, 0);
}

#[test]
fn v3_acquisition_exhaustion_keeps_prior_violations_and_never_claims_complete_inputs() {
    let dir = setup();
    let root = dir.path();
    let mut value = rule(json!({"op":"bool","value":true}));
    value["sources"] = json!({});
    for index in 0..16 {
        value["sources"][format!("source{index}")] = json!({
            "when":{"entity":"file","change":"all"},
            "applies_to":{"paths":["src/**/*.rs"]},
            "requires_capabilities":["files"]
        });
    }
    value["then"] = json!({"min_count":2});
    install(root, &value);
    std::fs::write(root.join("src/a.rs"), vec![b'a'; 2 * 1024 * 1024]).unwrap();
    let incomplete = check(root, 2);
    let result = &incomplete["checks"][0];
    assert_eq!(incomplete["gate"]["complete"], false);
    assert!(
        result["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("32 MiB facts")
    );
    assert_eq!(
        result["diagnostics"][0]["evidence"]["assertion"],
        "min_count"
    );
    assert!(result["metadata"].get("expression_inputs").is_none());
    // Every source is still visited when the root expression is a constant.
    value["sources"] = json!({});
    value["then"] = json!({});
    install(root, &value);
    check(root, 0);
}
