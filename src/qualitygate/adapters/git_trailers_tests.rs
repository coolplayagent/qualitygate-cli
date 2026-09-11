use super::*;
use crate::snapshot::history::Commit;
use std::sync::Arc;

fn commit(oid: &str, parents: &[&str], text: &str, value: &str) -> Commit {
    let files = BTreeMap::from([(
        "test_example.py".into(),
        File {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        },
    )]);
    Commit {
        oid: oid.into(),
        parents: parents.iter().map(|id| (*id).into()).collect(),
        content_digest: snapshot::content_digest(&files),
        files: files
            .into_iter()
            .map(|(path, file)| (path, Arc::new(file)))
            .collect(),
        message: format!("test\n\nAI: {value}\n"),
        trailers: BTreeMap::from([("ai".into(), vec![value.into()])]),
    }
}

fn setup() -> (Snapshot, History, Marker) {
    let base = commit(
        "base",
        &[],
        "def test_existing():\n    assert True\n",
        "author='past'",
    );
    let text = "def test_existing():\n    assert False\n";
    let left = commit("left", &["base"], text, "author='left'");
    let right = commit("right", &["base"], text, "author='right'");
    let merge = commit("merge", &["left", "right"], text, "");
    let before = files(&base);
    let after = files(&merge);
    let snapshot = Snapshot {
        root: ".".into(),
        identity: snapshot::Identity {
            mode: "diff".into(),
            base: "base".into(),
            head: "merge".into(),
            content_digest: snapshot::content_digest(&after),
            merge_request: None,
        },
        changes: snapshot::compare_files(&before, &after),
        files: after,
        base_files: before,
        path_filter: None,
        commits: vec![],
    };
    (
        snapshot,
        History {
            commits: vec![base, left, right, merge],
        },
        Marker {
            kind: "git_trailer".into(),
            name: "AI".into(),
            fields: vec!["author".into()],
        },
    )
}

fn entity(snapshot: &Snapshot) -> Entity {
    super::super::syntax::parse("test_example.py", &snapshot.files["test_example.py"].bytes)
        .unwrap()
        .unwrap()
        .tests
        .remove(0)
}

#[test]
fn identical_merge_results_require_each_inherited_commit_to_satisfy_fields() {
    let (snapshot, mut history, marker) = setup();
    let facts = analyze(&snapshot, &history).unwrap();
    assert_eq!(
        facts.current.values().next().unwrap(),
        &BTreeSet::from([Some("left".into()), Some("right".into())])
    );
    assert!(
        facts
            .declaration(&marker, "test_example.py", &entity(&snapshot), false)
            .unwrap()
            .is_some()
    );
    history.commits[2]
        .trailers
        .insert("ai".into(), vec!["reason='missing author'".into()]);
    let facts = analyze(&snapshot, &history).unwrap();
    assert_eq!(
        facts
            .declaration(&marker, "test_example.py", &entity(&snapshot), false)
            .unwrap(),
        Some(String::new())
    );
    history.commits[2].trailers.clear();
    let facts = analyze(&snapshot, &history).unwrap();
    assert!(
        facts
            .declaration(&marker, "test_example.py", &entity(&snapshot), false)
            .unwrap()
            .is_none()
    );
}

#[test]
fn incomplete_duplicate_or_foreign_history_does_not_establish_bindings() {
    let (mut snapshot, mut history, _) = setup();
    let facts = analyze(&snapshot, &history).unwrap();
    facts.ensure_binding(&snapshot).unwrap();
    snapshot
        .files
        .get_mut("test_example.py")
        .unwrap()
        .bytes
        .push(b'\n');
    assert!(facts.ensure_binding(&snapshot).is_err());
    assert!(analyze(&snapshot, &history).is_err());
    let (snapshot, _, _) = setup();
    history.commits[1].parents[0] = "missing".into();
    assert!(analyze(&snapshot, &history).is_err());
    let (_, mut history, _) = setup();
    history.commits[1].content_digest = "wrong".into();
    assert!(analyze(&snapshot, &history).is_err());
    let (_, mut history, _) = setup();
    history.commits[1].oid = "base".into();
    assert!(analyze(&snapshot, &history).is_err());
}

#[test]
fn ambiguous_renames_and_malformed_historical_syntax_remain_incomplete() {
    let first = commit(
        "base",
        &[],
        "def test_one():\n    assert True\n\ndef test_two():\n    assert True\n",
        "author='past'",
    );
    let second = commit(
        "merge",
        &["base"],
        "def test_three():\n    assert True\n\ndef test_four():\n    assert True\n",
        "author='new'",
    );
    let (mut snapshot, _, _) = setup();
    snapshot.base_files = files(&first);
    snapshot.files = files(&second);
    snapshot.identity.content_digest = snapshot::content_digest(&snapshot.files);
    let error = analyze(
        &snapshot,
        &History {
            commits: vec![first, second],
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("Ambiguous"));
    let (_, mut history, _) = setup();
    history.commits[0] = commit("base", &[], "def test_broken(:\n", "author='past'");
    let error = analyze(&snapshot, &history).unwrap_err();
    assert!(error.to_string().contains("syntax") || error.to_string().contains("Syntax"));
}

#[test]
fn unchanged_tests_with_historical_trailers_keep_dependency_obligations() {
    use crate::domain::{DependencyFact, ProjectFacts, Verdict};
    let mut baseline = commit(
        "base",
        &[],
        "class T { @Test void test_case() {} }\n",
        "author='past'",
    );
    let source = baseline.files.remove("test_example.py").unwrap();
    baseline.files.insert("tests/T.java".into(), source);
    baseline.content_digest = snapshot::content_digest(&files(&baseline));
    let (mut snapshot, _, _) = setup();
    snapshot.identity.mode = "worktree".into();
    snapshot.identity.head = "base".into();
    snapshot.base_files = files(&baseline);
    snapshot.files = snapshot.base_files.clone();
    snapshot.files.insert(
        "pom.xml".into(),
        File {
            bytes: b"<project/>\n".to_vec(),
            executable: false,
        },
    );
    snapshot.identity.content_digest = snapshot::content_digest(&snapshot.files);
    snapshot.changes = snapshot::compare_files(&snapshot.base_files, &snapshot.files);
    let facts = analyze(
        &snapshot,
        &History {
            commits: vec![baseline],
        },
    )
    .unwrap();
    let rule = serde_norway::from_str("id: paired\nversion: 1\nsource: {document: AGENTS.md, section: Rules, content_hash: sha256:fixture}\nlanguage: [java]\nrequires_capabilities: [test_methods, commits, dependency_resolution]\napplies_to: {provenance_scope: all_added_tests}\nbinding: {marker: {type: git_trailer, name: AI, fields: [author]}}\nwhen: {entity: test_method, change: added}\nthen: {require_marker: true, require_dependency: {group: junit, artifact: junit}}\nfix: Restore declared and resolved dependencies\n").unwrap();
    let mut project = ProjectFacts {
        schema_version: 1,
        ecosystem: "maven".into(),
        root: ".".into(),
        manifest: "pom.xml".into(),
        coordinate: "fixture:project:1".into(),
        source_root: "src".into(),
        test_source_root: "tests".into(),
        declared: vec![],
        resolved: vec![],
        dependency_usage: None,
        python: None,
        producer_check: "maven".into(),
        snapshot_digest: snapshot.identity.content_digest.clone(),
    };
    let run = |project: &ProjectFacts| {
        super::super::custom_rules::evaluate_with_context(
            &rule,
            &RuleSetting::default(),
            &snapshot,
            &super::super::facts::RuleFacts {
                projects: std::slice::from_ref(project),
                provenance: None,
                git_trailers: Some(&facts),
            },
        )
    };
    let failed = run(&project);
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.diagnostics.len(), 1);
    assert_eq!(
        failed.diagnostics[0].evidence["assertion"],
        "require_dependency"
    );
    let dependency = DependencyFact {
        group: "junit".into(),
        artifact: "junit".into(),
        version: "4.13.2".into(),
        artifact_type: "jar".into(),
        classifier: "".into(),
        scope: "test".into(),
    };
    project.declared.push(dependency.clone());
    project.resolved.push(dependency);
    assert_eq!(run(&project).verdict, Some(Verdict::Pass));
}
