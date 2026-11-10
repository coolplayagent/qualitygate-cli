use super::*;

const POLICY: &[u8] = b"schema_version: 1\nrules:\n  line-ending: {enabled: true}\nverification_assets: [checks/**]\n";

fn actor() -> Actor {
    Actor {
        id: "author".into(),
        kind: ActorKind::Human,
    }
}

fn evidence_record(source: &[u8]) -> EvidenceRecord {
    EvidenceRecord {
        case: None,
        schema_version: 1,
        kind: EvidenceKind::Review,
        source_digest: super::super::policy_store::digest(source),
        scope: "candidate policy".into(),
        timestamp: 1,
        actor: actor(),
        claims: vec!["Review identified a missing check".into()],
        sensitivity: Sensitivity::Internal,
        rule_ids: vec!["line-ending".into()],
        known_limits: vec!["unverified claim".into()],
        unverified_assumptions: vec![],
    }
}

fn proposal(evidence: String) -> Proposal {
    Proposal {
        actor: actor(),
        reason: "Correct observed behavior".into(),
        evidence: vec![evidence],
    }
}

fn retained(root: &Path) -> String {
    let source = b"review source";
    retain_evidence(root, evidence_record(source), source).unwrap()["evidence_ref"]
        .as_str()
        .unwrap()
        .into()
}

pub(in crate::config) fn candidate_fixture(root: &Path) -> Value {
    let reference = retained(root);
    create_from_snapshot(
        root,
        "qualitygate.yaml",
        "base-commit",
        [
            ("qualitygate.yaml", POLICY, false),
            ("checks/run", b"test".as_slice(), true),
            ("src/unrelated.rs", b"not policy owned".as_slice(), false),
        ],
        proposal(reference),
    )
    .unwrap()
}

#[test]
fn retaining_identical_evidence_is_idempotent_and_preserves_unverified_trust() {
    let root = tempfile::tempdir().unwrap();
    let reference = retained(root.path());
    assert_eq!(retained(root.path()), reference);
    let listed = list(root.path(), "evidence", 0, 1).unwrap();
    assert_eq!(listed["total"], 1);
    assert!(listed["next_offset"].is_null());
    let shown = show(root.path(), "evidence", &reference).unwrap();
    assert_eq!(shown["trust"], "unverified_evidence");
    assert_eq!(
        shown["record"]["claims"][0],
        "Review identified a missing check"
    );
    assert_eq!(
        history(root.path(), None, 10).unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        retain_evidence(root.path(), evidence_record(b"expected"), b"changed")
            .unwrap_err()
            .to_string()
            .contains("source digest")
    );
    std::fs::write(
        root.path().join("record.json"),
        serde_json::to_vec(&evidence_record(b"review source")).unwrap(),
    )
    .unwrap();
    std::fs::write(root.path().join("source.txt"), b"review source").unwrap();
    assert_eq!(
        retain_from_files(root.path(), "record.json", "source.txt").unwrap()["evidence_ref"],
        reference
    );
}

#[test]
fn snapshot_proposal_archives_only_policy_inputs_and_mutation_keeps_parent_and_author() {
    let root = tempfile::tempdir().unwrap();
    let created = candidate_fixture(root.path());
    let id = created["id"].as_str().unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let archived = show(root.path(), "policy", parent).unwrap();
    let files = archived["version"]["files"].as_object().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files["checks/run"]["executable"], true);
    assert!(!files.contains_key("src/unrelated.rs"));
    assert_eq!(archived["active"], false);
    let editor = Actor {
        id: "editor".into(),
        kind: ActorKind::Agent,
    };
    let edited = mutate(
        root.path(),
        id,
        editor.clone(),
        Mutation::Disable("line-ending".into()),
    )
    .unwrap();
    assert_eq!(edited["changed"], true);
    assert_eq!(edited["revision"]["parent_policy_digest"], parent);
    assert_eq!(edited["revision"]["created_by"]["id"], "author");
    assert_eq!(
        edited["revision"]["previous_revision"],
        created["revision_ref"]
    );
    assert_ne!(edited["revision"]["policy_digest"], parent);
    assert_eq!(edited["revision"]["patch"][0]["actor"]["id"], "editor");
    let again = mutate(
        root.path(),
        id,
        editor,
        Mutation::Disable("line-ending".into()),
    )
    .unwrap();
    assert_eq!(again["changed"], false);
    assert_eq!(again["revision_ref"], edited["revision_ref"]);
    assert_eq!(
        show(root.path(), "policy", parent).unwrap()["config"]["rules"]["line-ending"]["enabled"],
        true
    );
    assert_eq!(
        show(root.path(), "candidate", id).unwrap()["config"]["rules"]["line-ending"]["enabled"],
        false
    );
    let store = Store::open(root.path()).unwrap();
    let (_, config, frozen) = load_version(&store, parent).unwrap();
    assert!(frozen.resolve(&config).unwrap().0.rules["line-ending"].enabled);
}

#[test]
fn rejecting_candidate_freezes_it_and_history_pagination_preserves_causal_order() {
    let root = tempfile::tempdir().unwrap();
    let created = candidate_fixture(root.path());
    let id = created["id"].as_str().unwrap();
    let rejected = reject(root.path(), id, actor(), "Unsupported evidence").unwrap();
    assert_eq!(rejected["revision"]["status"], "rejected");
    assert!(
        mutate(
            root.path(),
            id,
            actor(),
            Mutation::Enable("line-ending".into())
        )
        .unwrap_err()
        .to_string()
        .contains("frozen")
    );
    assert!(reject(root.path(), id, actor(), "again").is_err());
    let first = history(root.path(), None, 1).unwrap();
    assert_eq!(first["events"][0]["event"]["action"], "candidate_rejected");
    let second = history(root.path(), first["next_cursor"].as_str(), 1).unwrap();
    assert_eq!(second["events"][0]["event"]["action"], "candidate_created");
    let third = history(root.path(), second["next_cursor"].as_str(), 1).unwrap();
    assert_eq!(third["events"][0]["event"]["action"], "evidence_retained");
    assert!(third["next_cursor"].is_null());
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let fresh = create_from_version(root.path(), parent, proposal(retained(root.path()))).unwrap();
    assert_ne!(fresh["id"], id);
    let page = list(root.path(), "candidate", 0, 1).unwrap();
    assert_eq!(page["total"], 2);
    assert_eq!(page["next_offset"], 1);
    assert_eq!(
        list(root.path(), "candidate", 1, 1).unwrap()["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        list(root.path(), "candidate", usize::MAX, 1).unwrap()["records"],
        json!([])
    );
}

#[test]
fn missing_inputs_invalid_queries_and_unpublished_evidence_cannot_create_candidate() {
    let root = tempfile::tempdir().unwrap();
    let reference = retained(root.path());
    assert!(
        create_from_snapshot(
            root.path(),
            "missing.yaml",
            "base",
            std::iter::empty(),
            proposal(reference.clone())
        )
        .unwrap_err()
        .to_string()
        .contains("missing the policy")
    );
    let missing_asset = b"schema_version: 1\nrules:\n  line-ending:\n    source:\n      document: missing.md\n      section: rules\n      digest: sha256:0000000000000000000000000000000000000000000000000000000000000000\n";
    assert!(
        create_from_snapshot(
            root.path(),
            "qualitygate.yaml",
            "base",
            [("qualitygate.yaml", missing_asset.as_slice(), false)],
            proposal(reference)
        )
        .is_err()
    );
    for kind in ["unknown", "policy"] {
        assert!(list(root.path(), kind, 0, 1).is_err());
    }
    for limit in [0, 257] {
        assert!(list(root.path(), "candidate", 0, limit).is_err());
        assert!(history(root.path(), None, limit).is_err());
    }
    assert!(show(root.path(), "unknown", "id").is_err());
    assert!(show(root.path(), "candidate", "unknown").is_err());
    let unpublished = super::super::policy_store::digest(b"unpublished");
    assert!(show(root.path(), "evidence", &unpublished).is_err());
    assert!(show(root.path(), "policy", &unpublished).is_err());
    assert_eq!(list(root.path(), "candidate", 0, 1).unwrap()["total"], 0);
}

#[test]
fn published_policy_records_still_require_valid_schema_configuration_and_normalized_inventory() {
    let root = tempfile::tempdir().unwrap();
    let created = candidate_fixture(root.path());
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let store = Store::open(root.path()).unwrap();
    let (original, _, _) = load_version(&store, parent).unwrap();
    for (kind, expected) in [
        (0, "schema or input inventory"),
        (1, "missing its configuration"),
        (2, "Invalid policy version path"),
    ] {
        let mut changed = original.clone();
        match kind {
            0 => changed.schema_version = 99,
            1 => {
                changed.files.remove(&changed.config_path);
            }
            _ => {
                let file = changed.files[&changed.config_path].clone();
                changed.files.insert("./aliased.yaml".into(), file);
            }
        }
        let reference = Store::transaction(root.path(), |store| {
            let reference = store.put_record("policy", &changed)?;
            store.index.policies.push(reference.clone());
            Ok(reference)
        })
        .unwrap();
        let store = Store::open(root.path()).unwrap();
        assert!(
            load_version(&store, &reference)
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
        assert!(load_version(&store, parent).is_ok());
    }
}

#[test]
fn candidate_package_retains_project_definitions_and_all_normative_sources_without_unrelated_code()
{
    let root = tempfile::tempdir().unwrap();
    let evidence = retained(root.path());
    let source = json!({"document":"AGENTS.md","section":"Rules","content_hash":super::super::policy_store::digest(b"# Rules\nVerify behavior.\n")});
    let custom = json!({"id":"scenario-test","version":1,"source":source,"language":["python"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},"then":{"name_pattern":"^test_"},"fix":"Name the scenario"});
    let config = json!({"schema_version":1,"custom_rules":"qualitygate/rules","rules":{"line-ending":{"source":source},"scenario-test":{}},
        "constraints":[{"schema_version":1,"id":"line-policy","version":1,"source":source,"required":true,"statement":"Require LF","realization":{"kind":"checks","mechanism":"regex","checks":["line-ending"],"positive_examples":["LF"],"negative_examples":["CRLF"]}}],
        "agent_obligations":[{"schema_version":1,"id":"review","version":1,"source":source,"required":true,"scope":{"roles":[],"tasks":[]},"triggers":["start"],"must":["Review behavior"],"must_not":[],"enforcement":{"kind":"instruction"}}]});
    let policy = serde_json::to_vec(&config).unwrap();
    let custom = serde_json::to_vec(&custom).unwrap();
    let created = create_from_snapshot(
        root.path(),
        "qualitygate.yaml",
        "pinned-parent",
        [
            ("qualitygate.yaml", policy.as_slice(), false),
            ("qualitygate/rules/scenario.yaml", custom.as_slice(), false),
            (
                "AGENTS.md",
                b"# Rules\nVerify behavior.\n".as_slice(),
                false,
            ),
            ("src/unrelated.py", b"pass".as_slice(), false),
        ],
        proposal(evidence),
    )
    .unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let store = Store::open(root.path()).unwrap();
    let (version, config, catalog) = load_version(&store, parent).unwrap();
    assert_eq!(version.files.len(), 3);
    assert!(version.files.contains_key("AGENTS.md"));
    assert!(!version.files.contains_key("src/unrelated.py"));
    let (resolved, catalog) = catalog.resolve(&config).unwrap();
    assert!(catalog.entries["scenario-test"].custom.is_some());
    let original = super::super::policy_promotion::rules_digest(&resolved, &catalog).unwrap();
    let mut changed = resolved.clone();
    changed.constraints[0].statement = "Changed requirement".into();
    assert_ne!(
        original,
        super::super::policy_promotion::rules_digest(&changed, &catalog).unwrap()
    );
    changed = resolved;
    changed.agent_obligations[0].must = vec!["Different instruction".into()];
    assert_ne!(
        original,
        super::super::policy_promotion::rules_digest(&changed, &catalog).unwrap()
    );
}
