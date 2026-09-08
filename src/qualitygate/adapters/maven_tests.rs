use super::*;
use crate::{
    config::{CustomRule, RuleSetting},
    domain::*,
    snapshot::{File, Identity},
};
use serde_json::{Value, json};

fn inputs(root: &Path) -> (Snapshot, MavenProject, String, Value) {
    let snapshot = Snapshot {
        root: root.into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        files: [(
            "module/pom.xml".into(),
            File {
                bytes: b"<project><artifactId>sample</artifactId></project>".to_vec(),
                executable: false,
            },
        )]
        .into(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    };
    let spec = MavenProject {
        root: "module".into(),
        effective_pom: "effective.xml".into(),
        dependency_tree: "tree.json".into(),
    };
    let model = format!(
        "<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>sample</artifactId><version>1</version><build><sourceDirectory>{}/module/src/main/java</sourceDirectory><testSourceDirectory>{}/module/src/test/java</testSourceDirectory></build><dependencies><dependency><groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version><scope>test</scope></dependency></dependencies></project>",
        root.display(),
        root.display()
    );
    let tree = json!({"groupId":"fixture", "artifactId":"sample", "version":"1", "type":"jar", "classifier":"", "optional":"false", "scope":"", "children":[
        {"groupId":"junit", "artifactId":"junit", "version":"4.13.2", "type":"jar", "classifier":"", "optional":"true", "scope":"test", "children":[
            {"groupId":"org.hamcrest", "artifactId":"hamcrest-core", "version":"1.3", "type":"jar", "classifier":"", "optional":"false", "scope":"test"}
        ]}
    ]});
    (snapshot, spec, model, tree)
}

#[test]
fn effective_model_resolution_and_test_classpath_are_distinct_facts() {
    let root = tempfile::tempdir().unwrap();
    let (snapshot, spec, model, tree) = inputs(root.path());
    let facts = parse(
        &spec,
        model.as_bytes(),
        &serde_json::to_vec(&tree).unwrap(),
        root.path(),
        &snapshot,
        "facts",
    )
    .unwrap();
    assert_eq!(facts.source_root, "module/src/main/java");
    assert_eq!(facts.resolved.len(), 2);
    assert_eq!(facts.declared.len(), 1);
    assert!(facts.owns_test("module/src/test/java/T.java"));
    assert!(!facts.owns_test("module/src/test/java-other/T.java"));
    assert!(!facts.owns_test("other/src/test/java/T.java"));
    assert!(facts.has_test_dependency("junit", "junit"));
    assert!(!facts.has_test_dependency("org.hamcrest", "hamcrest-core"));
    for (field, value) in [
        ("scope", "runtime"),
        ("classifier", "sources"),
        ("type", "pom"),
    ] {
        let mut tree = tree.clone();
        tree["children"][0][field] = json!(value);
        let mut model = model.clone();
        if field == "scope" {
            model = model.replace("<scope>test</scope>", "<scope>runtime</scope>");
        } else {
            model = model.replace(
                "<scope>test</scope>",
                &format!("<scope>test</scope><{field}>{value}</{field}>"),
            );
        }
        let facts = parse(
            &spec,
            model.as_bytes(),
            &serde_json::to_vec(&tree).unwrap(),
            root.path(),
            &snapshot,
            "facts",
        )
        .unwrap();
        assert!(!facts.has_test_dependency("junit", "junit"));
    }
}

#[test]
fn malformed_filtered_foreign_and_unresolved_maven_reports_are_incomplete() {
    let root = tempfile::tempdir().unwrap();
    let (snapshot, spec, model, tree) = inputs(root.path());
    let parse_model = |model: &str| {
        parse(
            &spec,
            model.as_bytes(),
            &serde_json::to_vec(&tree).unwrap(),
            root.path(),
            &snapshot,
            "facts",
        )
    };
    for model in [
        model.replace("<project>", "<projects><project>").replace("</project>", "</project></projects>"),
        model.replace("4.0.0", "4.1.0"),
        model.replace("<artifactId>sample</artifactId>", "<artifactId>other</artifactId>"),
        model.replace("<groupId>fixture</groupId>", "<groupId>fixture</groupId><groupId>ambiguous</groupId>"),
        model.replace("<version>1</version>", "<version>${unresolved}</version>"),
        model.replace("<version>1</version>", "<version> </version>"),
        model.replace("build>", "absent>"),
        model.replace(&root.path().display().to_string(), "relative"),
        model.replace("/module/src/", "/outside/src/"),
        model.replace("/src/main/java", "/src/test/java"),
        model.replace("<scope>test</scope>", "<scope>import</scope>"),
        model.replace("4.13.2", "not resolved"),
        model.replace("<dependency>", "<typo>").replace("</dependency>", "</typo>"),
        model.replace("</dependencies>", "<dependency><groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version></dependency></dependencies>"),
    ] { assert!(parse_model(&model).is_err(), "{model}"); }
    for (field, value) in [
        ("groupId", json!("other")),
        ("optional", json!("invalid")),
        ("children", json!([])),
        ("unknown", json!(true)),
    ] {
        let mut tree = tree.clone();
        tree[field] = value;
        assert!(
            parse(
                &spec,
                model.as_bytes(),
                &serde_json::to_vec(&tree).unwrap(),
                root.path(),
                &snapshot,
                "facts"
            )
            .is_err()
        );
    }
    let mut missing = snapshot.clone();
    missing.files.clear();
    assert!(
        parse(
            &spec,
            model.as_bytes(),
            b"{}",
            root.path(),
            &missing,
            "facts"
        )
        .is_err()
    );
    assert!(parse(&spec, &[0xff], b"{}", root.path(), &snapshot, "facts").is_err());
    assert!(
        parse(
            &spec,
            model.as_bytes(),
            b"{}",
            root.path(),
            &snapshot,
            "facts"
        )
        .is_err()
    );
}

fn rule() -> CustomRule {
    serde_norway::from_str("id: paired\nversion: 1\nsource: {document: AGENTS.md, section: Rules, content_hash: sha256:dummy}\nlanguage: [java]\nrequires_capabilities: [test_methods, annotations, dependency_resolution]\napplies_to: {paths: ['module/src/test/**/*.java'], provenance_scope: all_added_tests}\nbinding: {marker: {type: annotation, name: Generated, fields: [author]}}\nwhen: {entity: test_method, change: added}\nthen: {require_marker: true, require_dependency: {group: junit, artifact: junit}}\nfix: Restore dependency and marker").unwrap()
}

#[test]
fn dependency_rules_scan_unchanged_marked_tests_and_reject_missing_ambiguous_or_foreign_facts() {
    let root = tempfile::tempdir().unwrap();
    let (mut snapshot, spec, model, tree) = inputs(root.path());
    let facts = parse(
        &spec,
        model.as_bytes(),
        &serde_json::to_vec(&tree).unwrap(),
        root.path(),
        &snapshot,
        "facts",
    )
    .unwrap();
    snapshot.files.insert(
        "module/src/test/java/T.java".into(),
        File {
            bytes: b"class T { @Test @Generated(author=\"fixture\") void test() {} }".to_vec(),
            executable: false,
        },
    );
    let run = |facts: &[ProjectFacts]| {
        crate::adapters::custom_rules::evaluate_with_projects(
            &rule(),
            &RuleSetting::default(),
            &snapshot,
            facts,
        )
    };
    let passed = run(std::slice::from_ref(&facts));
    assert_eq!(passed.verdict, Some(Verdict::Pass));
    assert_eq!(passed.metadata["dependency_scope"]["tests"], 1);
    let mut removed = facts.clone();
    removed.declared.clear();
    assert_eq!(run(&[removed]).verdict, Some(Verdict::Fail));
    assert_eq!(run(&[]).execution.status, ExecutionStatus::Blocked);
    assert_eq!(
        run(&[facts.clone(), facts.clone()]).execution.status,
        ExecutionStatus::Blocked
    );
    let mut foreign = facts.clone();
    foreign.snapshot_digest = "other".into();
    assert_eq!(run(&[foreign]).execution.status, ExecutionStatus::Blocked);
    let mut wrong_root = facts.clone();
    wrong_root.test_source_root = "other/src/test/java".into();
    assert_eq!(
        run(&[wrong_root]).execution.status,
        ExecutionStatus::Blocked
    );
    let mut missing_group = rule();
    missing_group
        .then
        .require_dependency
        .as_mut()
        .unwrap()
        .group = None;
    assert_eq!(
        crate::adapters::custom_rules::evaluate_with_projects(
            &missing_group,
            &RuleSetting::default(),
            &snapshot,
            &[facts]
        )
        .execution
        .status,
        ExecutionStatus::Blocked
    );
}
