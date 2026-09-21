// Shared integration harness; individual targets exercise different helpers.
#![allow(dead_code)]

use crate::common::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use qualitygate::config::{
    policy_acceptance::{ApprovalKey, EvolutionTrust, ValidationCase, ValidationSuite},
    policy_candidates::{self, Proposal},
    policy_store::digest,
};
use qualitygate::domain::{
    evolution::{Actor, ActorKind, EvidenceKind, EvidenceRecord, Sensitivity},
    policy_evaluation::{CaseKind, EvaluationBudget, Expected},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, process::Command};

pub struct Fixture {
    pub root: tempfile::TempDir,
    pub external: tempfile::TempDir,
    pub id: String,
    pub baseline: String,
    pub suite: ValidationSuite,
    pub trust: EvolutionTrust,
}

pub fn run(root: &Path, args: &[&str], code: i32) -> Value {
    report(&cli(root, &[args, &["--format", "json"]].concat()), code)
}
pub fn head(root: &Path) -> String {
    String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .into()
}

impl Fixture {
    pub fn new() -> Self {
        let root = fixture();
        // macOS temporary-directory aliases may have symlink ancestors; the
        // protected-input contract requires the canonical operator path.
        let external =
            tempfile::tempdir_in(dunce::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            "schema_version: 1\nrules: {line-ending: {enabled: false}}\n",
        )
        .unwrap();
        std::fs::write(root.path().join(".gitignore"), ".qualitygate/\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "protected policy"]);
        let evidence = policy_candidates::retain_evidence(
            root.path(),
            EvidenceRecord {
                case: None,
                schema_version: 1,
                kind: EvidenceKind::ConversationCorrection,
                source_digest: digest(b"Require LF in new files"),
                scope: "project".into(),
                timestamp: 1,
                actor: Actor {
                    id: "generator".into(),
                    kind: ActorKind::Agent,
                },
                claims: vec!["Require LF in new files".into()],
                sensitivity: Sensitivity::Internal,
                rule_ids: vec!["line-ending".into()],
                known_limits: vec!["A request, not a measured benefit".into()],
                unverified_assumptions: Vec::new(),
            },
            b"Require LF in new files",
        )
        .unwrap()["evidence_ref"]
            .as_str()
            .unwrap()
            .to_owned();
        let created = run(
            root.path(),
            &[
                "policy",
                "candidate",
                "create",
                "--from-policy-ref",
                "HEAD",
                "--evidence",
                &evidence,
                "--reason",
                "Require LF",
                "--actor",
                "generator",
            ],
            0,
        );
        let id = created["id"].as_str().unwrap().into();
        let baseline: String = created["revision"]["parent_policy_digest"]
            .as_str()
            .unwrap()
            .into();
        let mut cases = Vec::new();
        for (name, kind, content, expected) in [
            ("replay", CaseKind::Replay, "bad\r\n", Expected::Fail),
            ("held-out", CaseKind::HeldOut, "good\n", Expected::Pass),
            ("anchor", CaseKind::Anchor, "anchor\n", Expected::Pass),
        ] {
            let base = head(root.path());
            std::fs::write(root.path().join(format!("{name}.txt")), content).unwrap();
            git(root.path(), &["add", "."]);
            git(root.path(), &["commit", "-qm", name]);
            let task = serde_json::from_value(json!({"schema_version":1,"task_id":format!("task-{name}"),"acceptance":[{"id":"probe","description":"Independent producer execution","verification":{"check_id":"probe","argv":["git","--version"],"timeout_seconds":5}}]})).unwrap();
            cases.push(ValidationCase {
                evidence_ref: None,
                id: name.into(),
                kind,
                base,
                head: head(root.path()),
                task,
                expectations: BTreeMap::from([
                    ("line-ending".into(), expected),
                    ("probe".into(), Expected::Pass),
                ]),
            });
        }
        let suite = ValidationSuite {
            schema_version: 1,
            id: "line-ending-acceptance".into(),
            baseline_policy: baseline.clone(),
            evaluator_epoch: "epoch-1".into(),
            motivating_evidence: vec![evidence],
            budget: EvaluationBudget {
                snapshot_max_mib: 16,
                snapshot_max_file_mib: 2,
                snapshot_jobs: 2,
                snapshot_timeout_seconds: 15,
                max_live_snapshot_mib: 64,
                max_parallel: 4,
                case_timeout_seconds: 20,
                total_timeout_seconds: 60,
            },
            min_improvements: 1,
            max_rules: 32,
            max_rule_growth: 4,
            cases,
        };
        let evaluator = run(root.path(), &["policy", "evaluator"], 0);
        let trust = EvolutionTrust {
            schema_version: 1,
            repository: dunce::canonicalize(root.path())
                .unwrap()
                .to_str()
                .unwrap()
                .into(),
            suites: vec![digest(&serde_json::to_vec(&suite).unwrap())],
            baselines: vec![baseline.clone()],
            evaluators: vec![evaluator["evaluator_digest"].as_str().unwrap().into()],
            approval_keys: vec![ApprovalKey {
                id: "reviewer-key".into(),
                actor: Actor {
                    id: "reviewer".into(),
                    kind: ActorKind::Human,
                },
                public_key: STANDARD.encode(
                    ed25519_dalek::SigningKey::from_bytes(&[7; 32])
                        .verifying_key()
                        .to_bytes(),
                ),
            }],
            revoked_approvals: Vec::new(),
            max_age_seconds: 3600,
        };
        let fixture = Self {
            root,
            external,
            id,
            baseline,
            suite,
            trust,
        };
        fixture.write_inputs();
        fixture
    }

    pub fn write_inputs(&self) {
        std::fs::write(
            self.external.path().join("suite.json"),
            serde_json::to_vec(&self.suite).unwrap(),
        )
        .unwrap();
        let mut trust = self.trust.clone();
        trust.suites = vec![digest(&serde_json::to_vec(&self.suite).unwrap())];
        std::fs::write(
            self.external.path().join("trust.json"),
            serde_json::to_vec(&trust).unwrap(),
        )
        .unwrap();
    }

    pub fn enable(&self) {
        run(
            self.root.path(),
            &[
                "policy",
                "candidate",
                "rules",
                "enable",
                &self.id,
                "line-ending",
                "--actor",
                "generator",
            ],
            0,
        );
    }

    pub fn validate(&self, jobs: &str, code: i32) -> Value {
        run(
            self.root.path(),
            &[
                "policy",
                "candidate",
                "validate",
                &self.id,
                "--baseline",
                &self.baseline,
                "--task",
                self.external.path().join("suite.json").to_str().unwrap(),
                "--trust-store",
                self.external.path().join("trust.json").to_str().unwrap(),
                "--evidence-dir",
                self.external.path().to_str().unwrap(),
                "--actor",
                "evaluator-operator",
                "--jobs",
                jobs,
            ],
            code,
        )
    }

    pub fn new_candidate(&mut self) {
        let created = policy_candidates::create_from_version(
            self.root.path(),
            &self.baseline,
            Proposal {
                actor: Actor {
                    id: "generator".into(),
                    kind: ActorKind::Agent,
                },
                reason: "Replay independent acceptance".into(),
                evidence: self.suite.motivating_evidence.clone(),
            },
        )
        .unwrap();
        self.id = created["id"].as_str().unwrap().into();
    }
}
