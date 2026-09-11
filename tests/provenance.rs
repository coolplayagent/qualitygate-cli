mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use ed25519_dalek::{Signer, SigningKey};
use qualitygate::{
    domain::*,
    snapshot::{self, Selection, Snapshot},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const SOURCE: &str = "# Team rules\nRequire complete declarations for participating tests.\n";
const BASE: &str = "# AI author='past'\ndef test_existing():\n    assert True\n";
const GENERATED: &str = "# AI author='past'\ndef test_existing():\n    assert True\n\ndef test_ai():\n    assert False\n";
const MIXED: &str = "# AI author='past'\ndef test_existing():\n    assert True\n\ndef test_ai():\n    assert True\n\ndef test_human():\n    assert True\n";
const REPAIRED: &str = "# AI author='past'\ndef test_existing():\n    assert True\n\n# AI author='agent'\ndef test_ai():\n    assert True\n\ndef test_human():\n    assert True\n";
const IDS: [&str; 2] = ["ai-code-traceability", "private-ai"];

fn capture(root: &Path) -> Snapshot {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(snapshot::capture(
            root,
            &Selection::Worktree {
                base: "HEAD".into(),
            },
        ))
        .unwrap()
}

struct Fixture {
    root: tempfile::TempDir,
    external: tempfile::TempDir,
    key: SigningKey,
    history: Vec<Snapshot>,
    actors: Vec<ActorKind>,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture();
        std::fs::create_dir_all(root.path().join("rules")).unwrap();
        std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
        std::fs::write(root.path().join("rules/ai.yaml"),format!(
            "id: private-ai\nversion: 1\nsource: {{document: AGENTS.md, section: Team rules, content_hash: {}}}\nlanguage: [python, java]\nrequires_capabilities: [test_methods, comments, external_provenance]\napplies_to: {{provenance_scope: ai_only}}\nbinding: {{marker: {{type: comment, name: AI, fields: [author]}}}}\nwhen: {{entity: test_method, change: added}}\nthen: {{require_marker: true}}\nfix: Restore the source declaration and recheck\n",snapshot::digest(SOURCE.as_bytes()))).unwrap();
        std::fs::write(root.path().join("qualitygate.yaml"),
            "schema_version: 1\ncustom_rules: rules\nrules:\n  ai-code-traceability:\n    provenance: {evidence_file: builtin.json}\n    parameters:\n      provenance_scope: ai_only\n      languages: [python, java]\n      marker: {type: comment, name: AI, fields: [author]}\n  private-ai:\n    provenance: {evidence_file: custom.json}\n").unwrap();
        std::fs::write(root.path().join("test_example.py"), BASE).unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "policy and baseline"]);
        let initial = capture(root.path());
        let result = Self {
            root,
            external: tempfile::tempdir().unwrap(),
            key: SigningKey::from_bytes(&[41; 32]),
            history: vec![initial],
            actors: vec![],
        };
        result.trust(json!([]), true);
        result
    }

    fn trust(&self, revoked: Value, provenance: bool) {
        std::fs::write(self.external.path().join("trust.json"),serde_json::to_vec(&json!({
            "schema_version":1,"repository":"fixture/project","max_age_seconds":3600,"revoked_records":revoked,
            "keys":[{"id":"harness","public_key":STANDARD.encode(self.key.verifying_key().as_bytes()),
                "checks":IDS,"allow_repository_checks":true,"provenance_rules":if provenance {json!(IDS)} else {json!([])}}]
        })).unwrap()).unwrap();
    }

    fn change(&mut self, text: &str, actor: ActorKind) {
        std::fs::write(self.root.path().join("test_example.py"), text).unwrap();
        self.record_state(actor);
    }

    fn record_state(&mut self, actor: ActorKind) {
        self.history.push(capture(self.root.path()));
        self.actors.push(actor);
    }

    fn run(&self, extra: &[&str], code: i32) -> Value {
        let store = self.external.path().join("trust.json");
        let mut args = vec![
            "check",
            "--trust-store",
            store.to_str().unwrap(),
            "--evidence-dir",
            self.external.path().to_str().unwrap(),
            "--format",
            "json",
        ];
        args.extend_from_slice(extra);
        report(&cli(self.root.path(), &args), code)
    }

    fn check<'a>(&self, output: &'a Value, id: &str) -> &'a Value {
        output["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == id)
            .unwrap_or_else(|| panic!("Missing {id}: {output}"))
    }

    fn record(&self, output: &Value, id: &str) -> ProvenanceRecord {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut runs = Vec::new();
        let mut steps = Vec::new();
        for (index, pair) in self.history.windows(2).enumerate() {
            let run_id = format!("run-{index}");
            runs.push(ProvenanceRun {
                run_id: run_id.clone(),
                actor: ProvenanceActor {
                    kind: self.actors[index],
                    id: if self.actors[index] == ActorKind::Agent {
                        "coding-agent"
                    } else {
                        "developer"
                    }
                    .into(),
                    version: (self.actors[index] == ActorKind::Agent).then(|| "1.0".into()),
                },
                started_at: now - 2,
                ended_at: now - 1,
            });
            let (before, after) = (&pair[0].files, &pair[1].files);
            let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
            let identity = qualitygate::adapters::provenance::file_identity;
            let changes = paths
                .into_iter()
                .filter(|path| before.get(*path).map(identity) != after.get(*path).map(identity))
                .map(|path| ProvenanceFileChange {
                    path: path.clone(),
                    before: before.get(path).map(identity),
                    after: after.get(path).map(|file| ProvenanceFileContents {
                        content_base64: STANDARD.encode(&file.bytes),
                        executable: file.executable,
                    }),
                })
                .collect();
            steps.push(ProvenanceStep {
                run_id,
                input_content_digest: snapshot::content_digest(before),
                output_content_digest: snapshot::content_digest(after),
                changes,
            });
        }
        ProvenanceRecord {
            schema_version: 1,
            record_id: format!("record-{id}"),
            coverage: ProvenanceCoverage::CompleteComparison,
            subject: serde_json::from_value(
                self.check(output, id)["metadata"]["external_provenance"]["expected_subject"]
                    .clone(),
            )
            .unwrap(),
            issued_at: now,
            expires_at: now + 600,
            runs,
            steps,
        }
    }

    fn publish(&self, output: &Value) {
        for id in IDS {
            self.publish_record(id, &self.record(output, id));
        }
    }

    fn publish_record(&self, id: &str, record: &ProvenanceRecord) {
        let bytes = serde_json::to_vec(record).unwrap();
        let kind = qualitygate::adapters::provenance::PAYLOAD_TYPE;
        let signature = self
            .key
            .sign(&qualitygate::adapters::attestation::pae(kind, &bytes));
        std::fs::write(self.external.path().join(if id==IDS[0] {"builtin.json"} else {"custom.json"}),serde_json::to_vec(&json!({
            "payloadType":kind,"payload":STANDARD.encode(bytes),"signatures":[{"keyid":"harness","sig":STANDARD.encode(signature.to_bytes())}]
        })).unwrap()).unwrap();
    }
}

#[test]
fn signed_mixed_history_requires_missing_agent_markers_without_marking_human_tests() {
    let mut fixture = Fixture::new();
    fixture.change(GENERATED, ActorKind::Agent);
    fixture.change(MIXED, ActorKind::Human);
    let missing = fixture.run(&[], 2);
    fixture.publish(&missing);
    let failed = fixture.run(&[], 1);
    for id in IDS {
        let check = fixture.check(&failed, id);
        assert_eq!(check["diagnostics"].as_array().unwrap().len(), 1);
        assert!(
            check["diagnostics"][0]["evidence"]["symbol"]
                .as_str()
                .unwrap()
                .contains("test_ai")
        );
        assert_eq!(
            check["metadata"]["external_provenance"]["evidence"]["agent_test_entities"],
            1
        );
        assert_eq!(
            check["metadata"]["external_provenance"]["valid_at_completion"],
            true
        );
        assert_eq!(check["execution"]["artifacts"].as_array().unwrap().len(), 2);
    }
    fixture.change(REPAIRED, ActorKind::Human);
    let stale = fixture.run(&[], 2);
    fixture.publish(&stale);
    fixture.run(&[], 0);
    let before = std::fs::read(fixture.root.path().join("test_example.py")).unwrap();
    std::fs::remove_file(fixture.root.path().join("test_example.py")).unwrap();
    std::fs::write(fixture.root.path().join("test_moved.py"), before).unwrap();
    fixture.record_state(ActorKind::Human);
    fixture.publish(&fixture.run(&[], 2));
    fixture.run(&[], 0);
}

#[test]
fn human_only_changes_do_not_require_markers_but_cannot_remove_existing_declarations() {
    let mut fixture = Fixture::new();
    fixture.change(
        &format!("{BASE}\ndef test_human():\n    assert False\n"),
        ActorKind::Human,
    );
    fixture.publish(&fixture.run(&[], 2));
    let passed = fixture.run(&[], 0);
    for id in IDS {
        assert_eq!(
            fixture.check(&passed, id)["metadata"]["external_provenance"]["evidence"]["agent_test_entities"],
            0
        );
    }
    fixture.change(
        "def test_existing():\n    assert True\n\ndef test_human():\n    assert False\n",
        ActorKind::Human,
    );
    fixture.publish(&fixture.run(&[], 2));
    let removed = fixture.run(&[], 1);
    for id in IDS {
        assert!(
            fixture.check(&removed, id)["diagnostics"][0]["evidence"]["symbol"]
                .as_str()
                .unwrap()
                .contains("test_existing")
        );
    }
}

#[test]
fn provenance_is_bound_to_staged_bytes_and_explicit_snapshot_modes() {
    let mut fixture = Fixture::new();
    fixture.change(GENERATED, ActorKind::Agent);
    fixture.publish(&fixture.run(&[], 2));
    fixture.run(&[], 1);
    git(fixture.root.path(), &["add", "test_example.py"]);
    std::fs::write(
        fixture.root.path().join("test_example.py"),
        GENERATED.replace("def test_ai()", "# AI author='agent'\ndef test_ai()"),
    )
    .unwrap();
    let staged = fixture.run(&["--staged"], 2);
    fixture.publish(&staged);
    let failed = fixture.run(&["--staged"], 1);
    assert_eq!(failed["snapshot"]["mode"], "staged");
    fixture.record_state(ActorKind::Human);
    fixture.publish(&fixture.run(&[], 2));
    fixture.run(&[], 0);
}

#[test]
fn missing_runs_untrusted_signers_revocation_and_tampering_stay_incomplete() {
    let mut fixture = Fixture::new();
    fixture.change(GENERATED, ActorKind::Agent);
    let missing = fixture.run(&[], 2);
    for id in IDS {
        let mut record = fixture.record(&missing, id);
        record.steps.clear();
        fixture.publish_record(id, &record);
    }
    fixture.run(&[], 2);
    fixture.publish(&missing);
    fixture.trust(json!([]), false);
    fixture.run(&[], 2);
    fixture.trust(
        json!(["record-ai-code-traceability", "record-private-ai"]),
        true,
    );
    fixture.run(&[], 2);
    fixture.trust(json!([]), true);
    for name in ["builtin.json", "custom.json"] {
        let path = fixture.external.path().join(name);
        let mut envelope: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        envelope["payload"] = json!(STANDARD.encode(b"{}"));
        std::fs::write(path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    }
    let tampered = fixture.run(&[], 2);
    for id in IDS {
        assert_eq!(fixture.check(&tampered, id)["verdict"], Value::Null);
    }
}

#[test]
fn mixed_language_replay_tracks_java_and_python_entities() {
    let mut fixture = Fixture::new();
    fixture.change(GENERATED, ActorKind::Agent);
    std::fs::write(
        fixture.root.path().join("T.java"),
        "class T {\n @Test void should_create_when_valid() { }\n}\n",
    )
    .unwrap();
    fixture.record_state(ActorKind::Agent);
    fixture.publish(&fixture.run(&[], 2));
    let failed = fixture.run(&[], 1);
    for id in IDS {
        assert_eq!(
            fixture.check(&failed, id)["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            fixture.check(&failed, id)["metadata"]["external_provenance"]["evidence"]["agent_test_entities"],
            2
        );
    }
    std::fs::write(
        fixture.root.path().join("T.java"),
        "class T {\n // AI author='agent'\n @Test void should_create_when_valid() { }\n}\n",
    )
    .unwrap();
    fixture.record_state(ActorKind::Human);
    fixture.change(
        GENERATED
            .replace("def test_ai()", "# AI author='agent'\ndef test_ai()")
            .as_str(),
        ActorKind::Human,
    );
    fixture.publish(&fixture.run(&[], 2));
    fixture.run(&[], 0);
}

#[test]
fn provenance_must_remain_unchanged_and_unexpired_until_commands_finish() {
    let helper = tempfile::tempdir().unwrap();
    let source = helper.path().join("lifecycle.rs");
    let executable = helper.path().join(if cfg!(windows) {
        "provenance-lifecycle.exe"
    } else {
        "provenance-lifecycle"
    });
    std::fs::write(&source, r#"fn main() {
        let args: Vec<_> = std::env::args().collect();
        if args[1] == "expire" {
            let deadline: u64 = args[2].parse().unwrap();
            while std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        } else {
            let mut bytes = std::fs::read(&args[2]).unwrap();
            bytes.push(b' ');
            std::fs::write(&args[2], bytes).unwrap();
        }
    }"#).unwrap();
    let compiled = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    for mode in ["mutate", "expire"] {
        let mut fixture = Fixture::new();
        let deadline = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 10;
        let argument = if mode == "expire" {
            deadline.to_string()
        } else {
            fixture
                .external
                .path()
                .join("builtin.json")
                .to_str()
                .unwrap()
                .to_owned()
        };
        let path = fixture.root.path().join("qualitygate.yaml");
        let mut policy: Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        policy["checks"] = json!([{
            "id":"after-provenance","kind":"command","depends_on":IDS,
            "argv":[executable,mode,argument],"timeout_seconds":30
        }]);
        std::fs::write(path, serde_norway::to_string(&policy).unwrap()).unwrap();
        git(fixture.root.path(), &["add", "qualitygate.yaml"]);
        git(
            fixture.root.path(),
            &["commit", "-qm", "add lifecycle command"],
        );
        fixture.history = vec![capture(fixture.root.path())];
        fixture.change(
            &GENERATED.replace("def test_ai()", "# AI author='agent'\ndef test_ai()"),
            ActorKind::Agent,
        );
        let missing = fixture.run(&[], 2);
        for id in IDS {
            let mut record = fixture.record(&missing, id);
            if mode == "expire" {
                record.expires_at = deadline;
            }
            fixture.publish_record(id, &record);
        }
        let completed = fixture.run(&[], 2);
        assert_eq!(
            fixture.check(&completed, "after-provenance")["verdict"],
            "pass"
        );
        for id in IDS {
            let check = fixture.check(&completed, id);
            assert_eq!(check["metadata"]["external_provenance"]["verified"], true);
            assert_eq!(
                check["metadata"]["external_provenance"]["valid_at_completion"],
                false
            );
            assert_eq!(check["verdict"], Value::Null);
            assert!(
                check["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains(if mode == "expire" {
                        "expired"
                    } else {
                        "changed"
                    })
            );
            assert_eq!(check["execution"]["artifacts"].as_array().unwrap().len(), 2);
        }
    }
}
