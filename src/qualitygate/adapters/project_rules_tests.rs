use super::*;
use crate::snapshot::{File, Identity};
use serde_json::{Value, json};

fn fixture() -> (Snapshot, Vec<ProjectFacts>) {
    let dependency = |artifact: &str| DependencyFact {
        group: "g".into(),
        artifact: artifact.into(),
        version: "1".into(),
        artifact_type: "jar".into(),
        classifier: "".into(),
        scope: "compile".into(),
    };
    let project = |root: &str| ProjectFacts {
        schema_version: 1,
        ecosystem: "maven".into(),
        root: root.into(),
        manifest: format!("{root}/pom.xml"),
        coordinate: format!("g:{root}:1"),
        source_root: format!("{root}/src/main/java"),
        test_source_root: format!("{root}/src/test/java"),
        declared: vec![],
        resolved: vec![],
        dependency_usage: None,
        python: None,
        producer_check: format!("facts-{root}"),
        snapshot_digest: "digest".into(),
    };
    let mut core = project("core");
    core.declared.push(dependency("infra"));
    core.resolved.extend([
        dependency("infra"),
        dependency("transport"),
        dependency("transport"),
    ]);
    let snapshot = Snapshot {
        root: ".".into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        files: ["core/pom.xml", "infra/pom.xml"]
            .map(|path| {
                (
                    path.into(),
                    File {
                        bytes: b"<project/>".to_vec(),
                        executable: false,
                    },
                )
            })
            .into(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    };
    (snapshot, vec![core, project("infra")])
}

fn run(snapshot: &Snapshot, facts: &[ProjectFacts], policy: Value) -> CheckResult {
    let setting = RuleSetting {
        parameters: serde_json::from_value(policy).unwrap(),
        ..RuleSetting::default()
    };
    super::super::rules::evaluate_with_projects(
        "module-boundary",
        "module-boundary",
        &setting,
        snapshot,
        facts,
    )
}

fn policy() -> Value {
    json!({"modules":["core","infra"], "forbidden":[{"from":"g:core", "to":"g:infra"}]})
}

#[test]
fn boundaries_enforce_direction_and_scope_even_without_source_changes() {
    let (snapshot, facts) = fixture();
    let failed = run(&snapshot, &facts, policy());
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.matched_entities, 1);
    assert_eq!(failed.metadata["increment_mode"], "full");
    assert_eq!(failed.diagnostics[0].file.as_deref(), Some("core/pom.xml"));
    assert_eq!(failed.diagnostics[0].evidence["to"], "g:infra");
    assert_eq!(failed.diagnostics[0].evidence["producer"], "facts-core");
    let mut reverse = policy();
    reverse["forbidden"] = json!([{"from":"g:infra", "to":"g:core"}]);
    assert_eq!(run(&snapshot, &facts, reverse).verdict, Some(Verdict::Pass));
    let mut scope = policy();
    scope["forbidden"][0]["scopes"] = json!(["test"]);
    assert_eq!(run(&snapshot, &facts, scope).verdict, Some(Verdict::Pass));
    let mut repaired = facts.clone();
    repaired[0].declared.clear();
    assert_eq!(
        run(&snapshot, &repaired, policy()).verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn resolved_classpath_checks_transitive_dependencies_without_duplicate_diagnostics() {
    let (snapshot, facts) = fixture();
    let mut selected = policy();
    selected["forbidden"] =
        json!([{"from":"g:*", "to":"g:transport"}, {"from":"g:core", "to":"*:transport"}]);
    assert_eq!(
        run(&snapshot, &facts, selected.clone()).verdict,
        Some(Verdict::Pass)
    );
    selected["dependency_kind"] = json!("resolved");
    let failed = run(&snapshot, &facts, selected.clone());
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.matched_entities, 2);
    assert_eq!(failed.diagnostics.len(), 1);
    assert_eq!(
        failed.diagnostics[0].evidence["matching_directions"],
        json!([0, 1])
    );
    selected["forbidden"].as_array_mut().unwrap().reverse();
    let mut version = facts;
    version[0].coordinate = "g:core:2".into();
    let repeated = run(&snapshot, &version, selected);
    assert_eq!(
        failed.diagnostics[0].fingerprint,
        repeated.diagnostics[0].fingerprint
    );
}

#[test]
fn missing_ambiguous_foreign_or_unsupported_module_facts_are_never_passes() {
    let (snapshot, facts) = fixture();
    for missing in [
        vec![],
        vec![facts[0].clone()],
        vec![facts[0].clone(), facts[0].clone(), facts[1].clone()],
    ] {
        assert_eq!(
            run(&snapshot, &missing, policy()).execution.status,
            ExecutionStatus::Blocked
        );
    }
    for field in [
        "schema_version",
        "ecosystem",
        "snapshot_digest",
        "manifest",
        "coordinate",
    ] {
        let mut altered = serde_json::to_value(&facts).unwrap();
        altered[0][field] = match field {
            "schema_version" => json!(2),
            "coordinate" => json!("g:infra:1"),
            _ => json!("unavailable"),
        };
        let altered: Vec<ProjectFacts> = serde_json::from_value(altered).unwrap();
        assert_eq!(
            run(&snapshot, &altered, policy()).execution.status,
            ExecutionStatus::Blocked,
            "{field}"
        );
    }
}
