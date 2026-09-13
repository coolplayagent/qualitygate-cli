use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};

fn files(text: &str) -> BTreeMap<String, File> {
    BTreeMap::from([(
        "tests/test_example.py".into(),
        File {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        },
    )])
}

fn snapshot(base: BTreeMap<String, File>, head: BTreeMap<String, File>) -> Snapshot {
    Snapshot {
        root: ".".into(),
        identity: SnapshotIdentity {
            mode: "worktree".into(),
            base: "a".repeat(40),
            head: "b".repeat(40),
            content_digest: snapshot::content_digest(&head),
            merge_request: None,
        },
        changes: snapshot::compare_files(&base, &head),
        base_files: base,
        files: head,
        path_filter: None,
        commits: vec![],
    }
}

fn step(
    run: &str,
    before: &BTreeMap<String, File>,
    after: &BTreeMap<String, File>,
) -> ProvenanceStep {
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let changes = paths
        .into_iter()
        .filter(|path| before.get(*path).map(file_identity) != after.get(*path).map(file_identity))
        .map(|path| ProvenanceFileChange {
            path: path.clone(),
            before: before.get(path).map(file_identity),
            after: after.get(path).map(|file| ProvenanceFileContents {
                content_base64: STANDARD.encode(&file.bytes),
                executable: file.executable,
            }),
        })
        .collect();
    ProvenanceStep {
        run_id: run.into(),
        input_content_digest: snapshot::content_digest(before),
        output_content_digest: snapshot::content_digest(after),
        changes,
    }
}

fn setup() -> (SigningKey, TrustStore, Snapshot, ProvenanceRecord) {
    let key = SigningKey::from_bytes(&[29; 32]);
    let store = crate::config::attestation::parse(&serde_json::to_vec(&json!({"schema_version":1,"repository":"example/repo","max_age_seconds":3600,
        "keys":[{"id":"harness","public_key":STANDARD.encode(key.verifying_key().as_bytes()),"provenance_rules":["trace"]}]})).unwrap()).unwrap();
    let base = files("def test_existing():\n    assert True\n");
    let generated =
        files("def test_existing():\n    assert True\n\ndef test_ai():\n    assert False\n");
    let mixed = files(
        "def test_existing():\n    assert True\n\ndef test_ai():\n    assert False\n\ndef test_human():\n    assert True\n",
    );
    let repaired = files(
        "def test_existing():\n    assert True\n\ndef test_ai():\n    assert True\n\ndef test_human():\n    assert True\n",
    );
    let steps = vec![
        step("agent-1", &base, &generated),
        step("human-1", &generated, &mixed),
        step("human-1", &mixed, &repaired),
    ];
    let snapshot = snapshot(base, repaired);
    let policy = PolicyEvidence {
        resolved_commit: None,
        task_contract_source: None,
        source_reviews: Default::default(),
        source: "policy".into(),
        config_digest: "sha256:config".into(),
        rules_digest: "sha256:rules".into(),
        task_contract_digest: None,
        trust: "caller_supplied_ref".into(),
        changes: vec![],
    };
    let record = ProvenanceRecord {
        schema_version: 1,
        record_id: "record-1".into(),
        coverage: ProvenanceCoverage::CompleteComparison,
        subject: subject(&snapshot, &policy, &store.repository, "trace"),
        issued_at: 100,
        expires_at: 200,
        runs: vec![
            ProvenanceRun {
                run_id: "agent-1".into(),
                actor: ProvenanceActor {
                    kind: ActorKind::Agent,
                    id: "coding-agent".into(),
                    version: Some("1.0".into()),
                },
                started_at: 10,
                ended_at: 20,
            },
            ProvenanceRun {
                run_id: "human-1".into(),
                actor: ProvenanceActor {
                    kind: ActorKind::Human,
                    id: "developer".into(),
                    version: None,
                },
                started_at: 30,
                ended_at: 40,
            },
        ],
        steps,
    };
    (key, store, snapshot, record)
}

fn signed(key: &SigningKey, record: &impl Serialize) -> Vec<u8> {
    let bytes = serde_json::to_vec(record).unwrap();
    serde_json::to_vec(&json!({"payloadType":PAYLOAD_TYPE,"payload":STANDARD.encode(&bytes),
        "signatures":[{"keyid":"harness","sig":STANDARD.encode(key.sign(&attestation::pae(PAYLOAD_TYPE,&bytes)).to_bytes())}]})).unwrap()
}

fn participation(facts: &ProvenanceFacts, snapshot: &Snapshot) -> BTreeMap<String, bool> {
    snapshot
        .files
        .iter()
        .flat_map(|(path, file)| {
            super::super::syntax::parse(path, &file.bytes)
                .unwrap()
                .unwrap()
                .tests
                .into_iter()
                .map(|test| (test.name.clone(), facts.participated(path, &test)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn replay_preserves_agent_participation_after_human_repairs_and_named_file_moves() {
    let (key, store, mut snapshot, mut record) = setup();
    let facts = verify(
        &signed(&key, &record),
        &store,
        &record.subject,
        &snapshot,
        150,
    )
    .unwrap();
    assert_eq!(
        participation(&facts, &snapshot),
        BTreeMap::from([
            ("test_ai".into(), true),
            ("test_human".into(), false),
            ("test_existing".into(), false)
        ])
    );
    assert_eq!(facts.evidence()["agent_test_entities"], 1);
    assert!(facts.revalidate_time(&store, 200).is_err());
    assert!(facts.ensure_binding(&snapshot, "other").is_err());
    let moved = BTreeMap::from([("tests/moved.py".into(), File {bytes:b"def test_human():\n    assert True\n\ndef test_existing():\n    assert True\n\ndef test_ai():\n    assert True\n".to_vec(),executable:false})]);
    record.steps.push(step("human-1", &snapshot.files, &moved));
    snapshot = self::snapshot(snapshot.base_files, moved);
    record.subject.snapshot = SnapshotBinding::new(&snapshot.identity, None);
    assert!(facts.ensure_binding(&snapshot, "trace").is_err());
    let facts = verify(
        &signed(&key, &record),
        &store,
        &record.subject,
        &snapshot,
        150,
    )
    .unwrap();
    assert!(participation(&facts, &snapshot)["test_ai"]);
    assert!(!participation(&facts, &snapshot)["test_human"]);
}

#[test]
fn missing_steps_foreign_bytes_bad_actors_and_unsafe_changes_cannot_establish_scope() {
    let (key, store, snapshot, record) = setup();
    for case in [
        "missing",
        "input",
        "output",
        "unknown-run",
        "duplicate-run",
        "unused-run",
        "version",
        "time",
        "before",
        "path",
        "duplicate-path",
        "encoding",
        "coverage",
        "subject",
    ] {
        let mut value = serde_json::to_value(&record).unwrap();
        match case {
            "missing" => {
                value["steps"].as_array_mut().unwrap().pop();
            }
            "input" => value["steps"][0]["input_content_digest"] = json!("wrong"),
            "output" => value["steps"][0]["output_content_digest"] = json!("wrong"),
            "unknown-run" => value["steps"][0]["run_id"] = json!("unknown"),
            "duplicate-run" => value["runs"][1]["run_id"] = json!("agent-1"),
            "unused-run" => {
                let mut run = value["runs"][0].clone();
                run["run_id"] = json!("unused");
                value["runs"].as_array_mut().unwrap().push(run);
            }
            "version" => value["runs"][0]["actor"]["version"] = Value::Null,
            "time" => value["runs"][0]["ended_at"] = json!(101),
            "before" => value["steps"][0]["changes"][0]["before"]["digest"] = json!("wrong"),
            "path" => value["steps"][0]["changes"][0]["path"] = json!("../escape"),
            "duplicate-path" => {
                let change = value["steps"][0]["changes"][0].clone();
                value["steps"][0]["changes"]
                    .as_array_mut()
                    .unwrap()
                    .push(change);
            }
            "encoding" => {
                value["steps"][0]["changes"][0]["after"]["content_base64"] = json!("!bad")
            }
            "coverage" => value["coverage"] = json!("partial"),
            _ => value["subject"]["rule_id"] = json!("other"),
        }
        assert!(
            verify(
                &signed(&key, &value),
                &store,
                &record.subject,
                &snapshot,
                150
            )
            .is_err(),
            "{case}"
        );
    }
    let mut untrusted = store.clone();
    untrusted.keys[0].provenance_rules.clear();
    untrusted.keys[0].checks.push("trace".into());
    untrusted.keys[0].allow_repository_checks = true;
    assert!(
        verify(
            &signed(&key, &record),
            &untrusted,
            &record.subject,
            &snapshot,
            150
        )
        .is_err()
    );
    assert!(
        verify(
            &signed(&key, &record),
            &store,
            &record.subject,
            &snapshot,
            200
        )
        .is_err()
    );
    let mut revoked = store;
    revoked.revoked_records.push(record.record_id.clone());
    assert!(
        verify(
            &signed(&key, &record),
            &revoked,
            &record.subject,
            &snapshot,
            150
        )
        .is_err()
    );
}

#[test]
fn ambiguous_lineages_and_invalid_intermediate_syntax_remain_incomplete() {
    let (key, store, mut snapshot, mut record) = setup();
    let renamed = files(
        "def test_one():\n    assert True\n\ndef test_two():\n    assert True\n\ndef test_three():\n    assert True\n",
    );
    record
        .steps
        .push(step("human-1", &snapshot.files, &renamed));
    snapshot = self::snapshot(snapshot.base_files, renamed);
    record.subject.snapshot = SnapshotBinding::new(&snapshot.identity, None);
    let error = verify(
        &signed(&key, &record),
        &store,
        &record.subject,
        &snapshot,
        150,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(error.contains("Ambiguous"), "{error}");
    let (key, store, snapshot, mut record) = setup();
    let invalid = files("def test_broken(:\n");
    record.steps[0] = step("agent-1", &snapshot.base_files, &invalid);
    let error = verify(
        &signed(&key, &record),
        &store,
        &record.subject,
        &snapshot,
        150,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(
        error.contains("Syntax") || error.contains("syntax"),
        "{error}"
    );
}

#[test]
fn signed_empty_comparisons_are_distinct_from_missing_records_and_budget_overruns() {
    let (key, store, mut snapshot, mut record) = setup();
    snapshot = self::snapshot(snapshot.base_files.clone(), snapshot.base_files);
    record.subject.snapshot = SnapshotBinding::new(&snapshot.identity, None);
    record.runs.clear();
    record.steps.clear();
    let facts = verify(
        &signed(&key, &record),
        &store,
        &record.subject,
        &snapshot,
        150,
    )
    .unwrap();
    assert_eq!(facts.evidence()["steps_replayed"], 0);
    assert!(
        verify(
            &vec![0; MAX_ENVELOPE_BYTES + 1],
            &store,
            &record.subject,
            &snapshot,
            150
        )
        .is_err()
    );
    let mut value = serde_json::to_value(&record).unwrap();
    value["record_id"] = json!("");
    assert!(
        verify(
            &signed(&key, &value),
            &store,
            &record.subject,
            &snapshot,
            150
        )
        .is_err()
    );
}
