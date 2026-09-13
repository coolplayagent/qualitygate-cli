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
fn packaged_rules_expose_archived_standard_inputs() {
    let config = Config {
        rulesets: RULESETS.iter().map(|name| (*name).into()).collect(),
        ..Config::default()
    };
    let catalog = catalog::Catalog::load(&config, std::iter::empty()).unwrap();
    assert!(catalog.entries.len() >= 10);
    assert!(catalog.entries.values().any(|entry| {
        entry
            .builtin
            .as_ref()
            .is_some_and(|rule| !rule.standard_refs.is_empty())
    }));
    assert!(catalog.entries.values().all(|entry| {
        entry.builtin.as_ref().is_some_and(|rule| {
            !rule.lifecycle_inputs.is_empty()
                && rule.standard_refs.iter().all(|reference| {
                    !reference.source_id.is_empty() && !reference.controls.is_empty()
                })
        })
    }));
}

fn standards_registry(source_id: &str) -> String {
    format!(
        r#"schema_version: 3
reviewed_on: 2026-09-13
purpose: Reviewed source archive.
source_policy:
  authority_order: [repository-policy, official-company]
  adoption_rule: External guidance needs adoption.
  freshness_rule: Recheck sources.
sources:
  - id: {source_id}
    organization: Example Organization
    title: Example source
    authority: official-company
    kind: guide
    url: https://example.com/source
    languages: [all]
    lifecycle_stages: [implementation]
    concerns: [coding]
    summary: Example source summary.
notes: [Source summaries are paraphrases.]
"#
    )
}

fn lifecycle_matrix(input: &str) -> String {
    format!(
        r#"schema_version: 1
updated: 2026-09-13
title: Test lifecycle inputs
policy:
  missing_required_evidence: incomplete
  enforced_input_rule: Only immutable evidence is enforceable.
  evidence_contract_rule: Other evidence needs a contract.
  conflict_rule: Read conflicts before adoption.
taxonomy:
  lifecycle_stages: [plan, architecture, implementation, review, static-analysis, verification, release, operations]
  languages: [all, java, python, rust, cpp, cuda, typescript, go]
  concerns: [coding, documentation, testing, architecture, dependencies, security, performance, reliability, reviewability, quality-gate, operations]
  enforcement: [deterministic-static, semantic-static, external-report, design-evidence, benchmark-evidence, operational-evidence]
  outcomes: [violation, warning, incomplete, advisory]
  statuses: [enforced, evidence-contract, planned]
inputs: [{{{input}}}]
"#
    )
}

#[test]
fn lifecycle_matrix_rejects_unarchived_input_sources() {
    let registry = standards_registry("known-source");
    let matrix = lifecycle_matrix(
        "id: input, status: enforced, rule_id: line-ending, lifecycle_stage: implementation, languages: [all], concerns: [coding], enforcement: deterministic-static, outcome: violation, source_ids: [missing-source], evidence: [diff], applicability: source, critical_adoption: scope",
    );
    assert!(catalog::validate_standard_inputs(&registry, &matrix).is_err());
}

#[test]
fn lifecycle_matrix_rejects_unverifiable_enforced_evidence() {
    let registry = standards_registry("known-source");
    let matrix = lifecycle_matrix(
        "id: input, status: enforced, rule_id: line-ending, lifecycle_stage: architecture, languages: [all], concerns: [security], enforcement: design-evidence, outcome: violation, source_ids: [known-source], evidence: [design-record], applicability: source, critical_adoption: scope",
    );
    assert!(catalog::validate_standard_inputs(&registry, &matrix).is_err());
}

#[test]
fn standards_registry_rejects_invalid_source_metadata() {
    let registry = standards_registry("known-source");
    let matrix = lifecycle_matrix(
        "id: input, status: planned, lifecycle_stage: architecture, languages: [rust], concerns: [architecture], enforcement: semantic-static, outcome: advisory, source_ids: [known-source], evidence: [public-api-diff], applicability: public API, critical_adoption: policy",
    );
    catalog::validate_standard_inputs(&registry, &matrix).unwrap();
    for invalid in [
        registry.replace("reviewed_on: 2026-09-13", "reviewed_on: 2026-02-30"),
        registry.replace("authority: official-company", "authority: unknown"),
        registry.replace(
            "url: https://example.com/source",
            "url: http://example.com/source",
        ),
        registry.replace("languages: [all]", "languages: [unknown]"),
        registry.replace(
            "    summary: Example source summary.",
            "    unexpected: true\n    summary: Example source summary.",
        ),
    ] {
        assert!(catalog::validate_standard_inputs(&invalid, &matrix).is_err());
    }
}

#[test]
fn lifecycle_matrix_rejects_status_and_taxonomy_mismatches() {
    let registry = standards_registry("known-source");
    let matrix = lifecycle_matrix(
        "id: input, status: evidence-contract, lifecycle_stage: architecture, languages: [all], concerns: [security], enforcement: design-evidence, outcome: incomplete, source_ids: [known-source], evidence: [design-record], applicability: source, critical_adoption: scope",
    );
    catalog::validate_standard_inputs(&registry, &matrix).unwrap();
    for invalid in [
        matrix.replace("outcome: incomplete", "outcome: warning"),
        matrix.replace(
            "statuses: [enforced, evidence-contract, planned]",
            "statuses: [enforced, evidence-contract]",
        ),
        matrix.replace("enforcement: design-evidence", "enforcement: unknown"),
        matrix.replace(
            "status: evidence-contract,",
            "status: evidence-contract, rule_id: line-ending,",
        ),
        matrix.replace(
            "critical_adoption: scope",
            "unexpected: true, critical_adoption: scope",
        ),
    ] {
        assert!(catalog::validate_standard_inputs(&registry, &invalid).is_err());
    }
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
fn module_boundaries_require_explicit_inventory_and_valid_direction_constraints() {
    use serde_json::json;
    let original =
        json!({"modules":["core","infra"], "forbidden":[{"from":"g:core", "to":"g:infra"}]});
    let validate = |parameters| {
        parse(serde_norway::to_string(&json!({"schema_version":1, "rulesets":["lang-java"], "rules":{"module-boundary":{"parameters":parameters}}})).unwrap().as_bytes())
    };
    assert!(validate(original.clone()).is_ok());
    for (field, value) in [
        ("modules", json!([])),
        ("modules", json!(["core", "core"])),
        ("modules", json!(["../outside"])),
        ("forbidden", json!([])),
        ("dependency_kind", json!("inferred")),
        ("unknown", json!(true)),
        ("forbidden", json!([{"from":"core", "to":"g:infra"}])),
        ("forbidden", json!([{"from":"g:core:1", "to":"g:infra"}])),
        ("forbidden", json!([{"from":"g:[", "to":"g:infra"}])),
        (
            "forbidden",
            json!([{"from":"g:core", "to":"g:infra", "scope":"compile"}]),
        ),
        (
            "forbidden",
            json!([{"from":"g:core", "to":"g:infra", "scopes":["import"]}]),
        ),
        (
            "forbidden",
            json!([{"from":"g:core", "to":"g:infra", "scopes":["test","test"]}]),
        ),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        assert!(validate(changed).is_err(), "{field}");
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
fn bytecode_usage_requires_complete_single_module_command_and_strict_policy() {
    use serde_json::json;
    let mut argv: Vec<String> = vec!["mvn".into()];
    argv.extend(
        project_rules::MAVEN_USAGE_ARGS
            .iter()
            .map(|arg| (*arg).into()),
    );
    argv.extend(["clean", "test-compile", project_rules::MAVEN_USAGE_GOAL].map(str::to_owned));
    let check = json!({"id":"facts","argv":argv,"projects":[{"root":".","effective_pom":"effective.xml","dependency_tree":"tree.json","dependency_usage":true}]});
    let config = json!({"schema_version":1,"rulesets":["lang-java"],"rules":{"used-undeclared":{"depends_on":["facts"],"parameters":{"modules":["."]}}},"checks":[check]});
    let validate = |value| parse(serde_norway::to_string(&value).unwrap().as_bytes());
    validate(config.clone()).unwrap();
    for (index, argument) in argv.iter().enumerate().skip(1) {
        let mut changed = config.clone();
        changed["checks"][0]["argv"]
            .as_array_mut()
            .unwrap()
            .remove(index);
        assert!(validate(changed).is_err(), "missing {argument}");
    }
    for extra in [
        "-Dverbose=true",
        "-Dmdep.analyze.excludedClasses=.*",
        "--define=mdep.analyze.excludedClasses=.*",
        "-Dmdep.analyze.excludedClasses",
        "--define",
        "--quiet",
        "--threads=2",
        "--file=other.xml",
        "--log-file=report.log",
        "--projects=other",
    ] {
        let mut changed = config.clone();
        changed["checks"][0]["argv"]
            .as_array_mut()
            .unwrap()
            .push(json!(extra));
        assert!(validate(changed).is_err(), "{extra}");
    }
    let mut cwd = config.clone();
    cwd["checks"][0]["cwd"] = json!("other");
    assert!(validate(cwd).is_err());
    for modules in [
        json!([]),
        json!([".", "."]),
        json!(["../escape"]),
        json!(["./module"]),
        json!([""]),
    ] {
        let mut changed = config.clone();
        changed["rules"]["used-undeclared"]["parameters"]["modules"] = modules;
        assert!(validate(changed).is_err());
    }
    let mut unknown = config;
    unknown["rules"]["used-undeclared"]["parameters"]["module"] = json!(".");
    assert!(validate(unknown).is_err());
}

#[test]
fn python_projects_preserve_maven_config_and_require_actual_confined_installations() {
    use serde_json::json;
    let check = json!({"id":"python-facts","argv":["python3","-E","-P","-m","pip","--isolated","install","--ignore-installed","--target","target/python","--report","target/pip.json",".[test]"],"tools":[{"id":"python","argv":["python3","-E","-P","--version"]},{"id":"pip","argv":["python3","-E","-P","-m","pip","--version"]}],"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"target/python","install_report":"target/pip.json","extras":["test"]}]});
    let config = |check| json!({"schema_version":1,"checks":[check]});
    let validate = |value| parse(serde_norway::to_string(&value).unwrap().as_bytes());
    let parsed = validate(config(check.clone())).unwrap();
    assert!(matches!(
        &parsed.checks[0].projects[0],
        ProjectSpec::Python(_)
    ));
    for (pointer, value) in [
        ("/projects/0/ecosystem", json!("unknown")),
        ("/projects/0/root", json!("module")),
        ("/projects/0/source_root", json!(".")),
        ("/projects/0/test_source_root", json!("src/tests")),
        ("/projects/0/install_target", json!("src/packages")),
        (
            "/projects/0/install_report",
            json!("target/python/report.json"),
        ),
        ("/projects/0/extras", json!(["test", "test"])),
        ("/projects/0/extras", json!(["TEST"])),
        ("/projects/0/extras", json!([])),
        ("/tools", json!([])),
    ] {
        let mut changed = check.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(validate(config(changed)).is_err(), "{pointer}");
    }
    for value in [
        "--dry-run",
        "--no-deps",
        "--user",
        "--prefix=/tmp/elsewhere",
        "--python-version=3.9",
        "other-package",
        "--ignore-installed",
    ] {
        let mut changed = check.clone();
        changed["argv"].as_array_mut().unwrap().push(json!(value));
        assert!(validate(config(changed)).is_err(), "{value}");
    }
    let maven = json!({"id":"maven","argv":["mvn"],"projects":[{"root":".","effective_pom":"effective.xml","dependency_tree":"tree.json"}]});
    let parsed = validate(config(maven)).unwrap();
    assert!(matches!(
        &parsed.checks[0].projects[0],
        ProjectSpec::Maven(_)
    ));
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
