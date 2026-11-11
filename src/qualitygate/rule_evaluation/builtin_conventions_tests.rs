use super::*;
use crate::{
    adapters::rules,
    domain::{DependencyFact, Verdict},
    snapshot::{Change, File, Identity},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

fn file(bytes: impl AsRef<[u8]>) -> File {
    File {
        bytes: bytes.as_ref().to_vec(),
        executable: false,
    }
}

fn snapshot(path: &str, old: Option<&str>, current: &str) -> Snapshot {
    Snapshot {
        scope_evidence: Default::default(),
        root: ".".into(),
        identity: Identity {
            verification_digest: None,
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
        },
        files: BTreeMap::from([(path.into(), file(current))]),
        base_files: old
            .map(|text| BTreeMap::from([(path.into(), file(text))]))
            .unwrap_or_default(),
        changes: BTreeMap::from([(
            path.into(),
            Change {
                kind: if old.is_some() { "modified" } else { "added" }.into(),
                old_path: old.map(|_| path.into()),
                removed_lines: Default::default(),
                added_lines: (1..=current.lines().count()).collect(),
            },
        )]),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn setting(parameters: serde_json::Value) -> RuleSetting {
    RuleSetting {
        parameters: serde_json::from_value(parameters).unwrap(),
        ..RuleSetting::default()
    }
}

fn project(dependency: bool) -> ProjectFacts {
    let fact = DependencyFact {
        group: "org.junit.jupiter".into(),
        artifact: "junit-jupiter-api".into(),
        version: "5.0".into(),
        artifact_type: "jar".into(),
        classifier: String::new(),
        scope: "test".into(),
    };
    ProjectFacts {
        schema_version: 1,
        ecosystem: "maven".into(),
        root: ".".into(),
        manifest: "pom.xml".into(),
        coordinate: "fixture:sample:1".into(),
        source_root: "src/main/java".into(),
        test_source_root: "src/test/java".into(),
        declared: if dependency {
            vec![fact.clone()]
        } else {
            Vec::new()
        },
        resolved: if dependency { vec![fact] } else { Vec::new() },
        dependency_usage: None,
        python: None,
        producer_check: "maven-facts".into(),
        snapshot_digest: "digest".into(),
    }
}

#[test]
fn annotation_dependency_selects_added_annotated_tests_and_requires_bound_maven_facts() {
    let old = "class T { @Test @Trace void old() { oldBody(); } }";
    let current = "class T { @Test @Trace void old() { oldBody(); } @Test @Trace void added() { newBody(); } @Test void unrelated() { otherBody(); } }";
    let input = snapshot("src/test/java/T.java", Some(old), current);
    let options = setting(
        json!({"annotation":"Trace","group":"org.junit.jupiter","artifact":"junit-jupiter-api","languages":["java"]}),
    );
    let evaluate = |facts: &[ProjectFacts]| {
        rules::evaluate_with_projects(
            "test-annotation-dependency",
            "test-annotation-dependency",
            &options,
            &input,
            facts,
        )
    };
    let missing = evaluate(&[]);
    assert_eq!(missing.verdict, None);
    assert!(
        missing
            .execution
            .reason
            .unwrap()
            .contains("dependency_resolution")
    );
    let failed = evaluate(&[project(false)]);
    assert_eq!(failed.verdict, Some(Verdict::Fail));
    assert_eq!(failed.matched_entities, 1);
    assert_eq!(failed.diagnostics.len(), 1);
    assert_eq!(
        failed.diagnostics[0].evidence["assertion"],
        "require_dependency"
    );
    assert_eq!(evaluate(&[project(true)]).verdict, Some(Verdict::Pass));
    let mut undeclared = project(true);
    undeclared.declared.clear();
    assert_eq!(evaluate(&[undeclared]).verdict, Some(Verdict::Fail));
    let mut unresolved = project(true);
    unresolved.resolved.clear();
    assert_eq!(evaluate(&[unresolved]).verdict, Some(Verdict::Fail));
    let mut stale = project(true);
    stale.snapshot_digest = "another".into();
    assert_eq!(evaluate(&[stale]).verdict, None);
    let mut unowned = project(true);
    unowned.test_source_root = "elsewhere".into();
    assert_eq!(evaluate(&[unowned]).verdict, None);
    assert_eq!(evaluate(&[project(true), project(true)]).verdict, None);
    let unannotated = snapshot(
        "src/test/java/T.java",
        None,
        "class T { @Test void should_work_when_ready() {} }",
    );
    let no_requirement = rules::evaluate_with_projects(
        "test-annotation-dependency",
        "test-annotation-dependency",
        &options,
        &unannotated,
        &[],
    );
    assert_eq!(no_requirement.verdict, Some(Verdict::Pass));
    assert_eq!(no_requirement.matched_entities, 0);
}

#[test]
fn literal_secret_rule_checks_only_added_java_files_and_hides_the_value() {
    let options =
        setting(json!({"pattern":"(?i)password\\s*=\\s*\"[^\"]{4,}\"","languages":["java"]}));
    let added = snapshot(
        "src/Main.java",
        None,
        "class Main { String password = \"private-value\"; }\n",
    );
    let found = rules::evaluate_as("no-hardcoded-secrets", "file-pattern", &options, &added);
    assert_eq!(found.verdict, Some(Verdict::Fail));
    assert_eq!(found.matched_entities, 1);
    assert_eq!(found.diagnostics[0].evidence["assertion"], "forbid_pattern");
    assert!(!format!("{found:?}").contains("private-value"));
    let modified = snapshot(
        "src/Main.java",
        Some("class Main {}\n"),
        "class Main { String password = \"private-value\"; }\n",
    );
    assert_eq!(
        rules::evaluate_as("no-hardcoded-secrets", "file-pattern", &options, &modified).verdict,
        Some(Verdict::Pass)
    );
    let non_java = snapshot("src/Main.txt", None, "password = \"private-value\"\n");
    assert_eq!(
        rules::evaluate_as("no-hardcoded-secrets", "file-pattern", &options, &non_java)
            .matched_entities,
        0
    );
    let mut invalid = added;
    invalid.files.get_mut("src/Main.java").unwrap().bytes = vec![0xff];
    assert_eq!(
        rules::evaluate_as("no-hardcoded-secrets", "file-pattern", &options, &invalid).verdict,
        None
    );
    let excessive = snapshot(
        "src/Main.java",
        None,
        &"String password = \"exposed\";\n".repeat(10_001),
    );
    let overflow = rules::evaluate_as("no-hardcoded-secrets", "file-pattern", &options, &excessive);
    assert_eq!(overflow.verdict, None);
    assert!(overflow.diagnostics.is_empty());
}

#[test]
fn native_and_neutral_file_rules_do_not_pass_unreadable_selected_inputs() {
    let native = setting(json!({"pattern":"\\bstrcpy\\s*\\(","languages":["c","cpp"]}));
    let mut bad_native = snapshot("src/main.cpp", None, "strcpy(dst, src);\n");
    bad_native.files.get_mut("src/main.cpp").unwrap().bytes = vec![0xff];
    let incomplete = rules::evaluate_as("no-unsafe-string", "file-pattern", &native, &bad_native);
    assert_eq!(incomplete.verdict, None);
    assert!(incomplete.diagnostics.is_empty());
    assert!(incomplete.execution.reason.unwrap().contains("UTF-8"));
    let unsupported = setting(json!({"pattern":"strcpy","languages":["unknown"]}));
    assert_eq!(
        rules::evaluate_as(
            "no-unsafe-string",
            "file-pattern",
            &unsupported,
            &bad_native
        )
        .verdict,
        None
    );

    let neutral =
        setting(json!({"pattern":"\\bsleep\\s*\\(","languages":[],"paths":["**/tests/**"]}));
    let selected = snapshot("tests/timer.bin", None, "sleep(1);\n");
    let mut invalid = selected.clone();
    invalid.files.get_mut("tests/timer.bin").unwrap().bytes = vec![0xff];
    assert_eq!(
        rules::evaluate_as("no-test-sleep", "file-pattern", &neutral, &invalid).verdict,
        None
    );
    let modified = snapshot("tests/timer.py", Some("ready()\n"), "sleep(1);\n");
    assert_eq!(
        rules::evaluate_as("no-test-sleep", "file-pattern", &neutral, &modified).verdict,
        Some(Verdict::Pass)
    );
}

#[test]
fn large_unchanged_tree_and_many_java_changes_keep_bounded_analysis() {
    let mut input = snapshot(
        "src/test/java/Case000.java",
        None,
        "class Case000 { @Test void should_work_when_ready() {} }\n",
    );
    for index in 0..18_000 {
        input
            .files
            .insert(format!("bulk/file-{index:05}.txt"), file("unchanged\n"));
    }
    for index in 1..512 {
        let path = format!("src/test/java/Case{index:03}.java");
        input.files.insert(
            path.clone(),
            file(format!(
                "class Case{index:03} {{ @Test void should_work_when_ready() {{}} }}\n"
            )),
        );
        input.changes.insert(
            path,
            Change {
                kind: "added".into(),
                old_path: None,
                removed_lines: Default::default(),
                added_lines: [1].into(),
            },
        );
    }
    let started = Instant::now();
    let named = rules::evaluate_as(
        "test-naming-strict",
        "test-naming-strict",
        &setting(json!({"pattern":"^should_.+_when_.+","languages":["java"]})),
        &input,
    );
    let scanned = rules::evaluate_as(
        "no-hardcoded-secrets",
        "file-pattern",
        &setting(json!({"pattern":"password\\s*=","languages":["java"]})),
        &input,
    );
    assert_eq!(named.verdict, Some(Verdict::Pass));
    assert_eq!(scanned.verdict, Some(Verdict::Pass));
    assert_eq!(named.matched_entities, 512);
    assert_eq!(scanned.matched_entities, 512);
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "512 changed Java files with 18000 unrelated files exceeded analysis budget: {:?}",
        started.elapsed()
    );
}
