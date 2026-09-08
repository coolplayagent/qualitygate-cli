use super::*;

#[test]
fn rejects_unknown_fields_schemas_duplicates_and_empty_commands() {
    for yaml in [
        "schema_version: 2",
        "schema_version: 1\nrulez: {}",
        "schema_version: 1\nchecks: [{id: build}]",
        "schema_version: 1\nchecks: [{id: build, argv: [echo]}, {id: build, argv: [echo]}]",
        "schema_version: 1\nrules: {bad/id: {}}",
    ] {
        assert!(parse(yaml.as_bytes()).is_err(), "{yaml}");
    }
    assert!(parse(&vec![b' '; MAX_CONFIG_BYTES + 1]).is_err());
}

#[test]
fn validates_profiles_timeouts_thresholds_and_dependency_cycles() {
    for yaml in [
        "schema_version: 1\nrules: {format: {}}\nprofiles: {full: {include: []}}",
        "schema_version: 1\nprofiles: {typo: {include: []}}",
        "schema_version: 1\nprofiles: {quick: {include: [missing]}}",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], timeout_seconds: 0}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], depends_on: [x]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], depends_on: [missing]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], tools: [{id: tool, argv: [git, --version]}], reports: [{path: report, format: lcov, coverage_paths: ['src/**'], minimum_coverage: 101}]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], tools: [{id: tool, argv: [git, --version]}], reports: [{path: report, format: diagnostics, mode: new_diagnostics}]}]",
    ] {
        assert!(parse(yaml.as_bytes()).is_err(), "{yaml}");
    }
}

#[test]
fn init_detects_language_and_preserves_existing_configuration() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
    let config = init(root.path()).unwrap();
    assert_eq!(config.languages, ["rust"]);
    assert!(config.rules["line-ending"].required);
    let before = std::fs::read(root.path().join(CONFIG_FILE)).unwrap();
    init(root.path()).unwrap();
    assert_eq!(
        before,
        std::fs::read(root.path().join(CONFIG_FILE)).unwrap()
    );
    std::fs::write(root.path().join(CONFIG_FILE), "invalid").unwrap();
    assert!(init(root.path()).is_err());
    assert_eq!(
        std::fs::read_to_string(root.path().join(CONFIG_FILE)).unwrap(),
        "invalid"
    );
}

#[test]
fn valid_command_config_roundtrips_and_dependencies_need_not_be_ordered() {
    let yaml = "schema_version: 1\nchecks: [{id: test, argv: [cargo, test], depends_on: [build]}, {id: build, argv: [cargo, build]}]";
    let config = parse(yaml.as_bytes()).unwrap();
    assert_eq!(config.checks.len(), 2);
    parse(serde_norway::to_string(&config).unwrap().as_bytes()).unwrap();
}

#[test]
fn rule_and_command_dependencies_form_one_plan_with_profile_closure() {
    let yaml = "schema_version: 1\nrules: {line-ending: {depends_on: [facts]}}\nchecks: [{id: final, argv: [git, --version], depends_on: [line-ending]}, {id: facts, argv: [git, --version]}]\nprofiles: {quick: {include: [line-ending]}}";
    let config = parse(yaml.as_bytes()).unwrap();
    let full = Plan::build(&config, None, "full").unwrap();
    assert_eq!(full.order, ["facts", "line-ending", "final"]);
    let quick = Plan::build(&config, None, "quick").unwrap();
    assert_eq!(quick.order, ["facts", "line-ending"]);
    assert_eq!(quick.pending_delivery, ["final"]);
    for altered in [
        yaml.replace("depends_on: [facts]", "depends_on: [final]"),
        yaml.replace("depends_on: [facts]", "depends_on: [unknown]"),
        yaml.replace("depends_on: [facts]", "depends_on: [facts, facts]"),
    ] {
        assert!(parse(altered.as_bytes()).is_err());
    }
    let disabled = parse(
        yaml.replace(
            "line-ending: {depends_on",
            "line-ending: {enabled: false, depends_on",
        )
        .as_bytes(),
    )
    .unwrap();
    assert!(Plan::build(&disabled, None, "full").is_err());
}

#[test]
fn maven_facts_config_rejects_ambiguous_outputs_and_unsuccessful_producers() {
    use serde_json::json;
    let original = json!({"id":"facts", "argv":["mvn"], "projects":[{"root":".", "effective_pom":"target/effective.xml", "dependency_tree":"target/tree.json"}]});
    let parse_check = |check| {
        parse(
            serde_norway::to_string(&json!({"schema_version":1,"checks":[check]}))
                .unwrap()
                .as_bytes(),
        )
    };
    assert!(parse_check(original.clone()).is_ok());
    for (field, value) in [
        ("expected_exit_code", json!(7)),
        ("kind", json!("manual")),
        (
            "projects",
            json!([{"root":"../outside", "effective_pom":"x", "dependency_tree":"y"}]),
        ),
        (
            "projects",
            json!([{"root":".", "effective_pom":"x", "dependency_tree":"x"}]),
        ),
        (
            "reports",
            json!([{"format":"diagnostics", "path":"target/tree.json"}]),
        ),
        (
            "reports",
            json!([{"format":"diagnostics", "path":"diagnostics.json", "mode":"changed_lines"}]),
        ),
    ] {
        let mut invalid = original.clone();
        invalid[field] = value;
        assert!(parse_check(invalid).is_err());
    }
}

#[test]
fn report_provenance_exit_semantics_and_tool_inputs_are_validated() {
    use serde_json::json;
    let original = json!({
        "id":"static","argv":["analyzer"],"findings_exit_codes":[1],
        "tools":[{"id":"analyzer","argv":["analyzer","--version"],"inputs":["tool.jar"]}],
        "reports":[{"path":"report.json","format":"diagnostics","mode":"new_diagnostics","baseline":"report.json"}]
    });
    let config =
        |check| serde_norway::to_string(&json!({"schema_version":1,"checks":[check]})).unwrap();
    parse(config(original.clone()).as_bytes()).unwrap();
    for (field, value) in [
        ("tools", json!([{"id":"x","argv":[]}])),
        (
            "tools",
            json!([{"id":"x","argv":["tool"],"timeout_seconds":0}]),
        ),
        (
            "tools",
            json!([{"id":"x","argv":["tool"],"timeout_seconds":61}]),
        ),
        (
            "tools",
            json!([{"id":"x","argv":["tool"],"inputs":["../escape"]}]),
        ),
        (
            "tools",
            json!([{"id":"x","argv":["tool"]},{"id":"x","argv":["tool"]}]),
        ),
        ("findings_exit_codes", json!([1, 1])),
        ("findings_exit_codes", json!([0])),
        ("reports", json!([])),
        (
            "reports",
            json!([{"path":"report.json","format":"diagnostics","mode":"new_diagnostics","baseline":"../escape"}]),
        ),
    ] {
        let mut check = original.clone();
        check[field] = value;
        assert!(parse(config(check.clone()).as_bytes()).is_err(), "{check}");
    }
}

#[test]
fn misspelled_or_wrongly_typed_builtin_parameters_cannot_disable_assertions() {
    for rules in [
        "{line-ending: {parameters: {paths: ['*.txt']}}}",
        "{commit-message: {parameters: {pattern: 123}}}",
        "{diff-size: {parameters: {max_added_lines: 0}}}",
        "{test-naming: {parameters: {patten: '^good'}}}",
        "{test-naming: {parameters: {patterns: {java: '['}}}}",
        "{test-naming: {parameters: {paths: '*.java'}}}",
        "{test-naming: {parameters: {languages: [23]}}}",
        "{comment-language: {parameters: {language: german}}}",
        "{comment-language: {parameters: {exempt_patterns: [false]}}}",
        "{parameterized-tests: {parameters: {minimum_similar: 1}}}",
        "{ai-code-traceability: {parameters: {marker: {type: guessed, name: X}}}}",
        "{ai-code-traceability: {parameters: {provenance_scope: author_name}}}",
    ] {
        assert!(
            parse(format!("schema_version: 1\nrules: {rules}\n").as_bytes()).is_err(),
            "{rules}"
        );
    }
    for rulesets in ["[lang-typo]", "[core, core]"] {
        assert!(parse(format!("schema_version: 1\nrulesets: {rulesets}").as_bytes()).is_err());
    }
}
