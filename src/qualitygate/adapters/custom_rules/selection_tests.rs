use super::*;
use crate::snapshot::{File, Identity, compare_files};
use serde_json::json;

fn snapshot() -> Snapshot {
    let files = BTreeMap::from([(
        "test_example.py".into(),
        File {
            bytes: b"# example\nimport math\ndef test_one():\n    assert True\n".to_vec(),
            executable: false,
        },
    )]);
    Snapshot {
        scope_evidence: Default::default(),
        root: "/repo".into(),
        identity: Identity {
            verification_digest: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        changes: compare_files(&BTreeMap::new(), &files),
        files,
        base_files: BTreeMap::new(),
        path_filter: None,
        commits: vec![("oid".into(), "Subject\n\nCommit body".into())],
    }
}

fn rule(entity: &str) -> CustomRule {
    let capability = match entity {
        "file" => "files",
        "commit" => "commits",
        "test_method" => "test_methods",
        "comment" => "comments",
        "import" => "imports",
        _ => unreachable!(),
    };
    serde_json::from_value(json!({
        "schema_version": 3, "id":"bounded-source", "version":1,
        "source":{"document":"AGENTS.md","section":"Facts","content_hash":format!("sha256:{}", "0".repeat(64))},
        "when":{"entity":entity,"change":if entity == "file" { "all" } else { "added" }},
        "requires_capabilities":[capability],
        "expression":{"op":"bool","value":true},"fix":"Provide complete facts"
    })).unwrap()
}

#[test]
fn retained_marker_subjects_still_consume_acquisition_capacity() {
    let mut snapshot = snapshot();
    let path = "test_example.py";
    snapshot.files.get_mut(path).unwrap().bytes =
        b"# @generated author=bot\ndef test_one():\n    assert True\n".to_vec();
    snapshot.base_files = snapshot.files.clone();
    snapshot.files.get_mut(path).unwrap().bytes = b"def test_one():\n    assert True\n".to_vec();
    snapshot.changes = compare_files(&snapshot.base_files, &snapshot.files);
    let mut rule = rule("test_method");
    rule.binding = Some(
        serde_json::from_value(json!({"marker":{"type":"comment","name":"@generated"}})).unwrap(),
    );
    rule.requires_capabilities.push("comments".into());
    rule.applies_to.provenance_scope = Some("all_added_tests".into());
    rule.then.require_marker = true;
    let mut budget = Budget::new();
    let retained = collect(&rule, &snapshot, &facts(), false, Some(&mut budget)).unwrap();
    assert_eq!(retained.len(), 1);
    assert!(!retained[0].triggered);
    assert!(retained[0].declaration.is_none());
    assert_eq!(budget.entities, 1);
    budget.entities = MAX_ENTITIES;
    assert!(collect(&rule, &snapshot, &facts(), false, Some(&mut budget)).is_err());
}

fn facts() -> RuleFacts<'static> {
    RuleFacts {
        projects: &[],
        provenance: None,
        git_trailers: None,
    }
}

#[test]
fn every_selector_shares_entity_and_byte_allowances_with_the_next_source() {
    let snapshot = snapshot();
    for kind in ["file", "commit", "test_method", "comment", "import"] {
        let rule = rule(kind);
        let mut budget = Budget::new();
        budget.entities = MAX_ENTITIES - 1;
        let subjects = collect(&rule, &snapshot, &facts(), false, Some(&mut budget)).unwrap();
        assert_eq!(subjects.len(), 1, "{kind}");
        let subject = &subjects[0];
        let bytes =
            subject.name.len() + subject.text.len() + subject.file.as_ref().map_or(0, String::len);
        assert_eq!((budget.entities, budget.bytes), (MAX_ENTITIES, bytes));
        let error = collect(&rule, &snapshot, &facts(), true, Some(&mut budget))
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("50000 entities"),
            "{kind}: {error}"
        );
        // A rejected reservation does not commit partial accounting.
        assert_eq!((budget.entities, budget.bytes), (MAX_ENTITIES, bytes));

        let mut budget = Budget::new();
        budget.bytes = MAX_FACT_BYTES - bytes;
        collect(&rule, &snapshot, &facts(), false, Some(&mut budget)).unwrap();
        assert_eq!(budget.bytes, MAX_FACT_BYTES);
        assert!(collect(&rule, &snapshot, &facts(), true, Some(&mut budget)).is_err());
        assert_eq!((budget.entities, budget.bytes), (1, MAX_FACT_BYTES));
    }
}

#[test]
fn file_budget_is_reserved_before_decoding_or_copying_the_next_fact() {
    let mut snapshot = snapshot();
    snapshot.files.values_mut().next().unwrap().bytes = vec![0xff];
    let mut budget = Budget::new();
    budget.bytes = MAX_FACT_BYTES;
    let error = collect(&rule("file"), &snapshot, &facts(), true, Some(&mut budget))
        .err()
        .unwrap();
    assert!(error.to_string().contains("32 MiB facts"));
    // Without exhaustion the same input reaches strict decoding, never lossy UTF-8.
    assert!(
        collect(&rule("file"), &snapshot, &facts(), true, None)
            .err()
            .unwrap()
            .to_string()
            .contains("utf-8")
    );
    assert_eq!(budget.entities, 0);
    budget.bytes = 0;
    assert!(budget.reserve(usize::MAX, 1, 1).is_err());
    assert_eq!(budget.bytes, 0);
}

#[test]
fn expired_sources_and_empty_sources_cannot_restart_the_shared_deadline() {
    let mut snapshot = snapshot();
    for empty in [false, true] {
        if empty {
            snapshot.files.clear();
            snapshot.changes.clear();
            snapshot.commits.clear();
        }
        for kind in ["file", "commit", "test_method", "comment", "import"] {
            let mut budget = Budget::new();
            collect(&rule(kind), &snapshot, &facts(), false, Some(&mut budget)).unwrap();
            budget.deadline = Instant::now();
            let error = collect(&rule(kind), &snapshot, &facts(), true, Some(&mut budget))
                .err()
                .unwrap();
            assert!(error.to_string().contains("deadline"), "{kind}: {error}");
        }
    }
}

#[test]
fn syntax_acquisition_propagates_the_deadline_before_parsing() {
    let mut snapshot = snapshot();
    snapshot.files.values_mut().next().unwrap().bytes = b"def broken(\n".to_vec();
    let error = entity_changes::collect_before(&snapshot, &[], &[], true, Instant::now())
        .err()
        .unwrap();
    assert!(error.to_string().contains("30 seconds"));
    let error = syntax::parse_before("test.py", b"def broken(\n", Instant::now()).unwrap_err();
    assert!(error.to_string().contains("budget"));
    let error = syntax::parse_before(
        "test.py",
        b"def broken(\n",
        crate::adapters::parallel::deadline(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("parsed completely"));
}
