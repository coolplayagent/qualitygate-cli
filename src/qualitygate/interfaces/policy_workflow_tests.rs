use crate::application::policy_validation::ValidateOptions;
use crate::config::{
    policy_acceptance::{ApprovalKey, EvolutionTrust},
    policy_candidates,
    policy_store::digest,
};
use crate::domain::{
    evolution::{Actor, ActorKind, EvidenceKind, EvidenceRecord, Sensitivity},
    policy_evaluation::{CaseKind, EvaluationBudget, Expected},
};
use crate::{config, snapshot};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
async fn repository(policy: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    crate::snapshot::run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), policy).unwrap();
    std::fs::write(root.path().join("source.rs"), "fn example() {}\n").unwrap();
    crate::snapshot::run_git(
        root.path(),
        &["-c", "core.autocrlf=false", "add", "."],
        None,
    )
    .await
    .unwrap();
    crate::snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit Test",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "test: initial snapshot",
        ],
        None,
    )
    .await
    .unwrap();
    root
}
fn budget() -> EvaluationBudget {
    EvaluationBudget {
        snapshot_max_mib: 16,
        snapshot_max_file_mib: 2,
        snapshot_jobs: 2,
        snapshot_timeout_seconds: 10,
        max_live_snapshot_mib: 64,
        max_parallel: 4,
        case_timeout_seconds: 10,
        total_timeout_seconds: 30,
    }
}

fn trust(root: &Path, suite: String, baseline: String, evaluator: String) -> EvolutionTrust {
    EvolutionTrust {
        schema_version: 1,
        repository: root.to_string_lossy().into_owned(),
        suites: vec![suite],
        baselines: vec![baseline],
        evaluators: vec![evaluator],
        approval_keys: vec![ApprovalKey {
            id: "fixture-key".into(),
            actor: Actor {
                id: "fixture-reviewer".into(),
                kind: ActorKind::Human,
            },
            public_key: STANDARD
                .encode(SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes()),
        }],
        revoked_approvals: Vec::new(),
        max_age_seconds: 3600,
    }
}

async fn fixture() -> (tempfile::TempDir, tempfile::TempDir, ValidateOptions) {
    let policy = "schema_version: 1\nrules: {line-ending: {enabled: false}}\n";
    let root = repository(policy).await;
    let external = tempfile::tempdir().unwrap();
    let base = String::from_utf8(
        snapshot::run_git(root.path(), &["rev-parse", "HEAD"], None)
            .await
            .unwrap(),
    )
    .unwrap()
    .trim()
    .to_string();
    let author = Actor {
        id: "author".into(),
        kind: ActorKind::Agent,
    };
    let evidence = policy_candidates::retain_evidence(
        root.path(),
        EvidenceRecord {
            schema_version: 1,
            case: None,
            kind: EvidenceKind::Review,
            source_digest: digest(b"require LF"),
            scope: "source text".into(),
            timestamp: 1,
            actor: author.clone(),
            claims: vec!["Require LF".into()],
            sensitivity: Sensitivity::Public,
            rule_ids: vec!["line-ending".into()],
            known_limits: vec!["synthetic unit evidence".into()],
            unverified_assumptions: vec![],
        },
        b"require LF",
    )
    .unwrap()["evidence_ref"]
        .as_str()
        .unwrap()
        .to_owned();
    let created = policy_candidates::create_from_snapshot(
        root.path(),
        "qualitygate.yaml",
        &base,
        [("qualitygate.yaml", policy.as_bytes(), false)],
        policy_candidates::Proposal {
            actor: author.clone(),
            reason: "Require LF".into(),
            evidence: vec![evidence.clone()],
        },
    )
    .unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    let baseline = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap()
        .to_string();
    policy_candidates::mutate(
        root.path(),
        &id,
        author,
        config::rule_management::Mutation::Enable("line-ending".into()),
    )
    .unwrap();
    let mut cases = vec![];
    for (name, kind, content, expected) in [
        ("replay", CaseKind::Replay, "bad\r\n", Expected::Fail),
        ("held-out", CaseKind::HeldOut, "good\n", Expected::Pass),
        ("anchor", CaseKind::Anchor, "same\n", Expected::Pass),
    ] {
        std::fs::write(root.path().join(format!("{name}.txt")), content).unwrap();
        snapshot::run_git(root.path(), &["add", &format!("{name}.txt")], None)
            .await
            .unwrap();
        snapshot::run_git(
            root.path(),
            &[
                "-c",
                "user.name=Unit Test",
                "-c",
                "user.email=unit@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                name,
            ],
            None,
        )
        .await
        .unwrap();
        let head = String::from_utf8(
            snapshot::run_git(root.path(), &["rev-parse", "HEAD"], None)
                .await
                .unwrap(),
        )
        .unwrap()
        .trim()
        .to_string();
        let task = serde_json::from_value(json!({"schema_version":1,"task_id":name,"acceptance":[{"id":"probe","description":"bounded tool probe",
            "verification":{"check_id":"probe","argv":["git","--version"],"timeout_seconds":5}}]})).unwrap();
        // Each case compares only its own change; the replay defect must not contaminate later cases.
        let previous = String::from_utf8(
            snapshot::run_git(root.path(), &["rev-parse", "HEAD^"], None)
                .await
                .unwrap(),
        )
        .unwrap()
        .trim()
        .to_string();
        cases.push(config::policy_acceptance::ValidationCase {
            evidence_ref: None,
            id: name.into(),
            kind,
            base: previous,
            head,
            task,
            expectations: [
                ("line-ending".into(), expected),
                ("probe".into(), Expected::Pass),
            ]
            .into(),
        });
    }
    let suite = config::policy_acceptance::ValidationSuite {
        schema_version: 1,
        id: "unit-acceptance".into(),
        baseline_policy: baseline.clone(),
        evaluator_epoch: "unit-epoch".into(),
        motivating_evidence: vec![evidence],
        budget: budget(),
        min_improvements: 1,
        max_rules: 32,
        max_rule_growth: 4,
        cases,
    };
    let suite_bytes = serde_json::to_vec(&suite).unwrap();
    let trust = trust(
        root.path(),
        digest(&suite_bytes),
        baseline.clone(),
        crate::application::policy_validation::evaluator()
            .await
            .unwrap()["evaluator_digest"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    let task = external.path().join("suite.json");
    let trust_store = external.path().join("trust.json");
    std::fs::write(&task, suite_bytes).unwrap();
    std::fs::write(&trust_store, serde_json::to_vec(&trust).unwrap()).unwrap();
    let options = ValidateOptions {
        root: root.path().into(),
        candidate_id: id,
        baseline,
        task,
        trust_store,
        evidence_dir: external.path().into(),
        jobs: Some(2),
        actor: Actor {
            id: "independent-validator".into(),
            kind: ActorKind::Human,
        },
    };
    (root, external, options)
}
fn signed(root: &Path, request: &Value, decision: &str) -> PathBuf {
    let now = crate::config::policy_store::now().unwrap();
    let record = json!({"schema_version":1,"subject":request["subject"],"approver":{"id":"fixture-reviewer","kind":"human"},"decision":decision,"issued_at":now,"expires_at":now+600,"reason":"Reviewed retained evidence"});
    let payload = serde_json::to_vec(&record).unwrap();
    let kind = request["payload_type"].as_str().unwrap();
    let mut pae = format!("DSSEv1 {} {} {} ", kind.len(), kind, payload.len()).into_bytes();
    pae.extend_from_slice(&payload);
    let key = SigningKey::from_bytes(&[7; 32]);
    let sig = key.sign(&pae);
    let path = root.join("approval.json");
    std::fs::write(&path,serde_json::to_vec(&json!({"payloadType":kind,"payload":STANDARD.encode(payload),"signatures":[{"keyid":"fixture-key","sig":STANDARD.encode(sig.to_bytes())}]})).unwrap()).unwrap();
    path
}

async fn dispatch(root: &std::path::Path, args: &[&str]) -> (Value, u8) {
    use clap::Parser;
    let cli = crate::interfaces::cli::Cli::try_parse_from(
        [
            "qualitygate",
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
        ]
        .into_iter()
        .chain(args.iter().copied()),
    )
    .unwrap();
    let (text, code) = cli.run().await.unwrap();
    (serde_json::from_str(&text).unwrap(), code)
}

#[tokio::test]
async fn policy_dispatch_keeps_signed_transition_and_lifecycle_evidence_separate_from_proposals() {
    let (root, external, options) = fixture().await;
    let id = options.candidate_id.clone();
    let baseline = options.baseline.clone();
    let trust = options.trust_store.clone();
    let (evaluation, code) = dispatch(
        root.path(),
        &[
            "policy",
            "candidate",
            "validate",
            &id,
            "--baseline",
            &baseline,
            "--task",
            options.task.to_str().unwrap(),
            "--trust-store",
            trust.to_str().unwrap(),
            "--evidence-dir",
            options.evidence_dir.to_str().unwrap(),
            "--jobs",
            "2",
            "--actor",
            &options.actor.id,
            "--actor-kind",
            "human",
        ],
    )
    .await;
    assert_eq!(code, 0);
    let reference = evaluation["evaluation_ref"].as_str().unwrap();
    let (retained, code) = dispatch(root.path(), &["policy", "evaluation", reference]).await;
    assert_eq!(code, 0);
    assert!(retained.is_object());
    let (request, _) = dispatch(
        root.path(),
        &[
            "policy",
            "candidate",
            "approval-subject",
            &id,
            "--trust-store",
            trust.to_str().unwrap(),
        ],
    )
    .await;
    let approval = signed(external.path(), &request, "approved");
    let (_, code) = dispatch(
        root.path(),
        &[
            "--envelope",
            "policy",
            "candidate",
            "approve",
            &id,
            "--trust-store",
            trust.to_str().unwrap(),
            "--approval",
            approval.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(code, 0);
    let (promoted, code) = dispatch(
        root.path(),
        &["--envelope", "policy", "candidate", "promote", &id],
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(promoted["gate"]["route"], "accept");
    let (context, _) = dispatch(root.path(), &["rules", "context"]).await;
    assert_eq!(context["review_trust"], "signed_active_policy");
    let (config, _) = dispatch(root.path(), &["config", "--show"]).await;
    assert_eq!(config["review_trust"], "signed_active_policy");
    let (request, _) = dispatch(
        root.path(),
        &[
            "policy",
            "rollback-subject",
            "--to",
            &baseline,
            "--trust-store",
            trust.to_str().unwrap(),
            "--actor",
            "operator",
            "--actor-kind",
            "human",
        ],
    )
    .await;
    let approval = signed(external.path(), &request, "approved");
    let (_, code) = dispatch(
        root.path(),
        &[
            "--envelope",
            "policy",
            "rollback",
            "--to",
            &baseline,
            "--trust-store",
            trust.to_str().unwrap(),
            "--approval",
            approval.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(code, 0);
}
