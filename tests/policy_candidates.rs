mod common;

use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn run(root: &Path, args: &[&str]) -> Value {
    report(&cli(root, &[args, &["--format", "json"]].concat()), 0)
}

fn evidence(root: &Path) -> String {
    let source = b"A correction requested consistent file endings.";
    std::fs::write(root.join("source.txt"), source).unwrap();
    let record = json!({"schema_version":1,"kind":"conversation_correction","source_digest":qualitygate::snapshot::digest(source),"scope":"project","timestamp":1,"actor":{"id":"agent-fixture","kind":"agent"},"claims":["Require consistent file endings"],"sensitivity":"internal","rule_ids":["line-ending"],"known_limits":["A claim, not proof of downstream benefit"],"unverified_assumptions":[]});
    std::fs::write(
        root.join("evidence.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    run(
        root,
        &[
            "policy",
            "evidence",
            "add",
            "--input",
            "evidence.json",
            "--source",
            "source.txt",
        ],
    )["evidence_ref"]
        .as_str()
        .unwrap()
        .into()
}

fn create(root: &Path, reference: &str, evidence: &str) -> Value {
    run(
        root,
        &[
            "policy",
            "candidate",
            "create",
            "--from-policy-ref",
            reference,
            "--evidence",
            evidence,
            "--reason",
            "Enforce consistent endings",
            "--actor",
            "agent-fixture",
        ],
    )
}

fn fixture_policy() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules: {line-ending: {enabled: false}}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "parent policy"]);
    root
}

#[test]
fn candidates_keep_immutable_parent_evidence_and_rejected_experiences() {
    let root = fixture_policy();
    let root = root.path();
    let evidence_ref = evidence(root);
    let repeated = evidence(root);
    assert_eq!(evidence_ref, repeated);
    let before = std::fs::read(root.join("qualitygate.yaml")).unwrap();
    let created = create(root, "HEAD", &evidence_ref);
    let id = created["id"].as_str().unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let first_revision = created["revision_ref"].as_str().unwrap();
    assert_eq!(created["revision"]["evidence_refs"], json!([evidence_ref]));
    let enabled = run(
        root,
        &[
            "policy",
            "candidate",
            "rules",
            "enable",
            id,
            "line-ending",
            "--actor",
            "editor",
        ],
    );
    assert_eq!(enabled["revision"]["parent_policy_digest"], parent);
    assert_eq!(enabled["revision"]["previous_revision"], first_revision);
    assert_ne!(enabled["revision"]["policy_digest"], parent);
    assert_eq!(enabled["revision"]["created_by"]["id"], "agent-fixture");
    assert_eq!(enabled["revision"]["patch"][0]["actor"]["id"], "editor");
    assert!(enabled["revision"]["approval_ref"].is_null());
    assert_eq!(
        std::fs::read(root.join("qualitygate.yaml")).unwrap(),
        before
    );
    let baseline = run(root, &["policy", "show", parent]);
    assert_eq!(baseline["config"]["rules"]["line-ending"]["enabled"], false);
    assert_eq!(baseline["active"], false);
    let shown = run(root, &["policy", "candidate", "show", id]);
    assert_eq!(shown["config"]["rules"]["line-ending"]["enabled"], true);
    assert_eq!(
        run(
            root,
            &[
                "policy",
                "candidate",
                "rules",
                "enable",
                id,
                "line-ending",
                "--actor",
                "editor"
            ]
        )["changed"],
        false
    );
    run(
        root,
        &[
            "policy",
            "candidate",
            "rules",
            "configure",
            id,
            "line-ending",
            "--severity",
            "warning",
            "--actor",
            "editor",
        ],
    );
    run(
        root,
        &[
            "policy",
            "candidate",
            "rules",
            "disable",
            id,
            "line-ending",
            "--actor",
            "editor",
        ],
    );
    let rejected = run(
        root,
        &[
            "policy",
            "candidate",
            "reject",
            id,
            "--reason",
            "No verified benefit",
            "--actor",
            "reviewer",
            "--actor-kind",
            "human",
        ],
    );
    assert_eq!(rejected["revision"]["status"], "rejected");
    let output = cli(
        root,
        &[
            "policy",
            "candidate",
            "rules",
            "enable",
            id,
            "line-ending",
            "--actor",
            "editor",
            "--format",
            "json",
        ],
    );
    report(&output, 2);
    let listed = run(root, &["policy", "candidate", "list"]);
    assert_eq!(listed["records"][0]["status"], "rejected");
    let from_digest = create(root, parent, &evidence_ref);
    assert_eq!(from_digest["revision"]["parent_policy_digest"], parent);
    assert_ne!(from_digest["id"], id);
    let history = run(root, &["policy", "history", "--limit", "2"]);
    assert_eq!(history["events"].as_array().unwrap().len(), 2);
    let cursor = history["next_cursor"].as_str().unwrap();
    let next = run(root, &["policy", "history", "--cursor", cursor]);
    assert!(!next["events"].as_array().unwrap().is_empty());
    let store = qualitygate::config::policy_store::Store::open(root).unwrap();
    let retained: qualitygate::domain::evolution::PolicyRevision =
        store.record(first_revision, "candidate").unwrap();
    assert_eq!(retained.parent_policy_digest, parent);
    assert!(retained.patch.is_empty());
    assert_eq!(run(root, &["policy", "evidence", "list"])["total"], 1);
    assert_eq!(
        run(root, &["policy", "evidence", "show", &evidence_ref])["trust"],
        "unverified_evidence"
    );
}

#[test]
fn failures_tampering_and_writer_conflicts_never_publish_success() {
    let root = fixture_policy();
    let root = root.path();
    let evidence_ref = evidence(root);
    let candidate = create(root, "HEAD", &evidence_ref);
    let id = candidate["id"].as_str().unwrap();
    let head_path = root.join(".qualitygate/policy/HEAD");
    let head = std::fs::read(&head_path).unwrap();
    let invalid = [
        vec![
            "policy",
            "candidate",
            "rules",
            "configure",
            id,
            "line-ending",
            "--param",
            "unknown=true",
            "--actor",
            "editor",
        ],
        vec![
            "policy",
            "candidate",
            "rules",
            "enable",
            id,
            "invented-rule",
            "--actor",
            "editor",
        ],
        vec![
            "policy",
            "candidate",
            "create",
            "--from-policy-ref",
            "HEAD",
            "--evidence",
            "invalid",
            "--reason",
            "gap",
            "--actor",
            "editor",
        ],
    ];
    for args in invalid {
        report(
            &cli(root, &[args.as_slice(), &["--format", "json"]].concat()),
            2,
        );
        assert_eq!(std::fs::read(&head_path).unwrap(), head);
        assert!(!root.join(".qualitygate/policy/write.lock").exists());
    }
    let lock = root.join(".qualitygate/policy/write.lock");
    std::fs::write(&lock, "other writer").unwrap();
    report(
        &cli(
            root,
            &[
                "policy",
                "candidate",
                "reject",
                id,
                "--reason",
                "test",
                "--actor",
                "editor",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(std::fs::read(&lock).unwrap(), b"other writer");
    std::fs::remove_file(lock).unwrap();
    let source_digest =
        qualitygate::snapshot::digest(b"A correction requested consistent file endings.");
    let source_object = root.join(format!(
        ".qualitygate/policy/objects/{}",
        &source_digest[7..]
    ));
    std::fs::write(source_object, "tampered").unwrap();
    report(
        &cli(
            root,
            &[
                "policy",
                "evidence",
                "show",
                &evidence_ref,
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(std::fs::read(&head_path).unwrap(), head);
}

#[test]
fn source_claims_are_not_accepted_without_matching_bytes_and_valid_schema() {
    let root = fixture_policy();
    let root = root.path();
    evidence(root);
    let head = std::fs::read(root.join(".qualitygate/policy/HEAD")).unwrap();
    std::fs::write(root.join("source.txt"), "different input").unwrap();
    report(
        &cli(
            root,
            &[
                "policy",
                "evidence",
                "add",
                "--input",
                "evidence.json",
                "--source",
                "source.txt",
                "--format",
                "json",
            ],
        ),
        2,
    );
    let mut record: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence.json")).unwrap()).unwrap();
    record["kind"] = json!("capability_gap");
    std::fs::write(
        root.join("evidence.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    report(
        &cli(
            root,
            &[
                "policy",
                "evidence",
                "add",
                "--input",
                "evidence.json",
                "--source",
                "source.txt",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        std::fs::read(root.join(".qualitygate/policy/HEAD")).unwrap(),
        head
    );
}

#[test]
fn frozen_project_rules_and_catalog_survive_worktree_and_runtime_asset_changes() {
    let root = fixture_policy();
    let root = root.path();
    std::fs::create_dir_all(root.join("qualitygate/rules")).unwrap();
    let source = "# Rules\nReview dangerous patterns.\n";
    std::fs::write(root.join("AGENTS.md"), source).unwrap();
    let definition = json!({"id":"project-signal","version":1,"language":["rust"],
        "source":{"document":"AGENTS.md","section":"Rules","content_hash":qualitygate::snapshot::digest(source.as_bytes())},
        "requires_capabilities":["files"],"when":{"entity":"file"},"then":{"forbid_pattern":"danger"},"fix":"Review dangerous patterns."});
    std::fs::write(
        root.join("qualitygate/rules/project.yaml"),
        serde_json::to_vec(&definition).unwrap(),
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "available project rules"]);
    let evidence_ref = evidence(root);
    let created = create(root, "HEAD", &evidence_ref);
    let id = created["id"].as_str().unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let policy = run(root, &["policy", "show", parent]);
    assert_eq!(policy["version"]["files"].as_object().unwrap().len(), 3);
    assert!(policy["version"]["files"]["hello.txt"].is_null());
    std::fs::write(
        root.join("qualitygate/rules/project.yaml"),
        "invalid YAML: [",
    )
    .unwrap();
    std::fs::remove_file(root.join("AGENTS.md")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            root.join("missing-skill-assets"),
        )
        .arg("--root")
        .arg(root)
        .args([
            "policy",
            "candidate",
            "rules",
            "enable",
            id,
            "project-signal",
            "--actor",
            "editor",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let edited = report(&output, 0);
    assert_eq!(edited["revision"]["parent_policy_digest"], parent);
    let shown = run(root, &["policy", "candidate", "show", id]);
    assert_eq!(shown["config"]["custom_rules"], "qualitygate/rules");
    assert_eq!(shown["config"]["rules"]["project-signal"]["enabled"], true);
}

#[test]
fn archiving_policy_avoids_copying_eighteen_thousand_unrelated_source_blobs() {
    use qualitygate::{
        config::{
            policy_candidates::{Proposal, create_from_snapshot},
            policy_store::Store,
        },
        domain::evolution::{Actor, ActorKind},
    };
    let root = fixture_policy();
    let evidence = evidence(root.path());
    let unrelated = vec![b'x'; 2048];
    let config = b"schema_version: 1\nrules: {line-ending: {}}\n";
    let paths: Vec<_> = (0..18_000)
        .map(|index| format!("src/{index:05}.txt"))
        .collect();
    let inputs = paths
        .iter()
        .map(|name| (name.as_str(), unrelated.as_slice(), false))
        .chain(std::iter::once((
            "qualitygate.yaml",
            config.as_slice(),
            false,
        )));
    let started = std::time::Instant::now();
    let result = create_from_snapshot(
        root.path(),
        "qualitygate.yaml",
        &"a".repeat(40),
        inputs,
        Proposal {
            actor: Actor {
                id: "agent".into(),
                kind: ActorKind::Agent,
            },
            reason: "Policy archive performance".into(),
            evidence: vec![evidence],
        },
    )
    .unwrap();
    let elapsed = started.elapsed();
    eprintln!(
        "issue4 archive 18,001 borrowed files (>35 MiB), policy-only publication: {elapsed:?}"
    );
    assert!(elapsed < std::time::Duration::from_secs(5));
    let store = Store::open(root.path()).unwrap();
    assert!(store.index.object_bytes < 1024 * 1024);
    let policy = run(
        root.path(),
        &[
            "policy",
            "show",
            result["revision"]["policy_digest"].as_str().unwrap(),
        ],
    );
    assert_eq!(policy["version"]["files"].as_object().unwrap().len(), 1);
}
