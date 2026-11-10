use super::*;
use crate::snapshot::{File, Identity};
use serde_json::{Value, json};

fn snapshot(entries: &[(&str, &str)]) -> Snapshot {
    let files: BTreeMap<_, _> = entries
        .iter()
        .map(|(name, text)| {
            (
                (*name).into(),
                File {
                    bytes: text.as_bytes().to_vec(),
                    executable: false,
                },
            )
        })
        .collect();
    Snapshot {
        root: "/unit".into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: crate::snapshot::content_digest(&files),
            verification_digest: None,
            merge_request: None,
        },
        changes: crate::snapshot::compare_files(&BTreeMap::new(), &files),
        files,
        base_files: Default::default(),
        path_filter: None,
        commits: vec![],
        scope_evidence: Default::default(),
    }
}
fn rule(assertions: Value) -> CustomRule {
    serde_json::from_value(json!({"id":"scenario","version":1,"source":{"document":"AGENTS.md","section":"Tests","content_hash":crate::snapshot::digest(b"source")},
        "language":["python"],"requires_capabilities":["test_methods","comments"],"when":{"entity":"test_method","change":"added"},
        "then":assertions,"fix":"Repair the behavior contract"})).unwrap()
}

#[test]
fn naming_marker_and_collection_limits_report_independent_actionable_violations() {
    let snapshot = snapshot(&[(
        "tests/test_behavior.py",
        "def test_bad():\n    assert True\n\ndef test_other():\n    assert True\n",
    )]);
    let mut rule =
        rule(json!({"name_pattern":"^test_scenario_","require_marker":true,"max_count":1}));
    rule.binding = Some(
        serde_json::from_value(
            json!({"marker":{"type":"comment","name":"Generated","fields":["author"]}}),
        )
        .unwrap(),
    );
    rule.applies_to.provenance_scope = Some("all_added_tests".into());
    let failed = evaluate(&rule, &RuleSetting::default(), &snapshot);
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    for assertion in ["name_pattern", "require_marker"] {
        assert!(
            failed
                .diagnostics
                .iter()
                .any(|d| d.evidence["assertion"] == assertion),
            "{assertion}: {:?}",
            failed.diagnostics
        );
    }
    rule.then.name_pattern = Some("^test_".into());
    rule.then.require_marker = false;
    rule.binding = None;
    rule.applies_to.provenance_scope = None;
    rule.then.max_count = Some(2);
    assert_eq!(
        evaluate(&rule, &RuleSetting::default(), &snapshot).verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn file_inventory_checks_unchanged_files_and_retains_aggregate_measurements() {
    let mut snapshot = snapshot(&[
        ("README.md", "one two\nthree four\n"),
        ("docs/design.md", "five six\n"),
    ]);
    snapshot.changes.clear();
    let mut rule =
        rule(json!({"min_count":3,"max_lines":1,"max_total_words":5,"required_paths":["LICENSE"]}));
    rule.language.clear();
    rule.requires_capabilities = vec!["files".into()];
    rule.when.entity = "file".into();
    rule.when.change = Some("all".into());
    let failed = evaluate(&rule, &RuleSetting::default(), &snapshot);
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.metadata["file_inventory"]["files"], 2);
    assert_eq!(failed.metadata["file_inventory"]["lines"], 3);
    assert_eq!(failed.metadata["file_inventory"]["words"], 6);
    for assertion in [
        "min_count",
        "max_lines",
        "max_total_words",
        "required_paths",
    ] {
        assert!(
            failed
                .diagnostics
                .iter()
                .any(|d| d.evidence["assertion"] == assertion)
        );
    }
    rule.then.min_count = Some(2);
    rule.then.max_lines = Some(2);
    rule.then.max_total_words = Some(6);
    rule.then.required_paths = vec!["README.md".into()];
    assert_eq!(
        evaluate(&rule, &RuleSetting::default(), &snapshot).verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn legacy_and_named_source_expressions_evaluate_complete_selected_collections() {
    let mut snapshot = snapshot(&[
        ("src/a.rs", "fn a() {}"),
        ("tests/a.rs", "#[test] fn test_a() {}"),
    ]);
    let mut rule = rule(json!({}));
    rule.schema_version = 2;
    rule.language.clear();
    rule.requires_capabilities = vec!["files".into()];
    rule.when.entity = "file".into();
    rule.when.change = Some("all".into());
    for expected in [1, 3] {
        rule.expression=Some(serde_json::from_value(json!({"op":"compare","left":{"op":"count","input":{"op":"entities"}},"relation":"ge","right":{"op":"number","value":expected}})).unwrap());
        let result = evaluate(&rule, &RuleSetting::default(), &snapshot);
        assert_eq!(
            result.verdict,
            Some(if expected == 1 {
                Verdict::Pass
            } else {
                Verdict::Fail
            })
        );
        assert_eq!(result.metadata["expression"]["entities"], 2);
    }
    rule.schema_version = 3;
    rule.applies_to.paths = vec!["src/**".into()];
    rule.sources=serde_json::from_value(json!({"tests":{"when":{"entity":"file","change":"all"},"applies_to":{"paths":["tests/**"]},"requires_capabilities":["files"]}})).unwrap();
    rule.expression=Some(serde_json::from_value(json!({"op":"compare","left":{"op":"count","input":{"op":"source","name":"tests"}},"relation":"ge","right":{"op":"number","value":1}})).unwrap());
    snapshot.path_filter = Some("src".into());
    let passed = evaluate(&rule, &RuleSetting::default(), &snapshot);
    assert_eq!(passed.verdict, Some(Verdict::Pass));
    assert_eq!(passed.metadata["expression"]["sources"]["tests"], 1);
    snapshot.files.remove("tests/a.rs");
    let failed = evaluate(&rule, &RuleSetting::default(), &snapshot);
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.metadata["expression_inputs"]["complete"], true);
}

#[test]
fn unavailable_language_provenance_and_dependency_facts_are_gaps_not_empty_success() {
    let snapshot = snapshot(&[(
        "tests/test_behavior.py",
        "def test_behavior():\n    assert True\n",
    )]);
    for case in 0..6 {
        let mut rule = rule(json!({"min_count":1}));
        let mut setting = RuleSetting::default();
        match case {
            0 => rule.language = vec!["unknown".into()],
            1 => {
                rule.language = vec!["shell".into()];
                rule.requires_capabilities.push("annotations".into());
            }
            2 => rule
                .requires_capabilities
                .push("external_provenance".into()),
            3 => rule.applies_to.provenance_scope = Some("ai_only".into()),
            4 => rule
                .requires_capabilities
                .push("dependency_resolution".into()),
            5 => {
                setting.provenance =
                    Some(serde_json::from_value(json!({"evidence_file":"origin.json"})).unwrap())
            }
            _ => unreachable!(),
        }
        assert_eq!(
            evaluate(&rule, &setting, &snapshot).execution.status,
            ExecutionStatus::Blocked,
            "case {case}"
        );
    }
}

#[test]
fn manifest_only_changes_recheck_retained_marked_tests_and_require_bound_python_dependencies() {
    let mut input = snapshot(&[
        (
            "tests/test_behavior.py",
            "# Generated: author=reviewer\ndef test_behavior(): pass\n",
        ),
        ("pyproject.toml", "[project]\nname='sample'\n"),
        ("src/ignored.rs", "fn helper() {}"),
    ]);
    input.base_files = input.files.clone();
    input.changes.retain(|path, _| path == "pyproject.toml");
    let mut rule = rule(json!({"require_dependency":{"artifact":"pytest"}}));
    rule.binding = Some(
        serde_json::from_value(
            json!({"marker":{"type":"comment","name":"Generated","fields":["author"]}}),
        )
        .unwrap(),
    );
    rule.applies_to.paths = vec!["tests/**".into()];
    let mut project:ProjectFacts=serde_json::from_value(json!({"schema_version":1,"ecosystem":"python","root":".","manifest":"pyproject.toml","coordinate":"sample","source_root":"src","test_source_root":"tests","declared":[],"resolved":[],"producer_check":"resolve","snapshot_digest":input.identity.content_digest,"python":{"pip_version":"26.0","environment":{},"extras":[],"install_target":"installed","installed_metadata":{}}})).unwrap();
    let observe = |rule: &CustomRule, project: &ProjectFacts| {
        evaluate_with_context(
            rule,
            &RuleSetting::default(),
            &input,
            &crate::adapters::facts::RuleFacts {
                projects: std::slice::from_ref(project),
                ..Default::default()
            },
        )
    };
    let missing = observe(&rule, &project);
    assert_eq!(missing.verdict, Some(Verdict::Fail));
    assert_eq!(missing.metadata["dependency_scope"]["tests"], 1);
    assert_eq!(
        missing.diagnostics[0].evidence["assertion"],
        "require_dependency"
    );
    let dependency:crate::domain::DependencyFact=serde_json::from_value(json!({"group":"pypi","artifact":"pytest","version":"8.0","artifact_type":"distribution","classifier":"","scope":"environment"})).unwrap();
    project.declared = vec![dependency.clone()];
    project.resolved = vec![dependency];
    assert_eq!(observe(&rule, &project).verdict, Some(Verdict::Pass));
    rule.then.require_dependency.as_mut().unwrap().group = Some("maven".into());
    assert_eq!(
        observe(&rule, &project).execution.status,
        ExecutionStatus::Blocked
    );
    rule.then.require_dependency.as_mut().unwrap().group = None;
    rule.schema_version = 2;
    rule.requires_capabilities
        .push("dependency_resolution".into());
    for scenario in [0, 1, 2] {
        let mut invalid = project.clone();
        match scenario {
            0 => invalid.schema_version = 99,
            1 => invalid.ecosystem = "unknown".into(),
            _ => invalid.snapshot_digest = crate::snapshot::digest(b"stale"),
        }
        let result = observe(&rule, &invalid);
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
        assert!(
            result
                .execution
                .reason
                .unwrap()
                .contains("another snapshot")
        );
    }
    rule.binding.as_mut().unwrap().marker.kind = "git_trailer".into();
    assert!(
        observe(&rule, &project)
            .execution
            .reason
            .unwrap()
            .contains("Commit-to-entity")
    );
}
