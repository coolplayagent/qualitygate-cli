mod common;
#[path = "common/evolution.rs"]
mod evolution;

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::Signer;
use evolution::*;
use qualitygate::domain::{
    ManualDecision,
    evolution::{Actor, ActorKind},
    policy_approval::{PolicyApproval, PolicyApprovalSubject},
};
use serde_json::{Value, json};

fn subject(fixture: &Fixture) -> PolicyApprovalSubject {
    serde_json::from_value(
        run(
            fixture.root.path(),
            &[
                "policy",
                "candidate",
                "approval-subject",
                &fixture.id,
                "--trust-store",
                fixture.external.path().join("trust.json").to_str().unwrap(),
            ],
            0,
        )["subject"]
            .clone(),
    )
    .unwrap()
}

fn sign(
    fixture: &Fixture,
    subject: PolicyApprovalSubject,
    actor: &str,
    decision: ManualDecision,
) -> Value {
    let now = qualitygate::config::policy_store::now().unwrap();
    let record = PolicyApproval {
        schema_version: 1,
        subject,
        approver: Actor {
            id: actor.into(),
            kind: ActorKind::Human,
        },
        decision,
        issued_at: now,
        expires_at: now + 300,
        reason: "Independent fixture approval after paired validation".into(),
    };
    let payload = serde_json::to_vec(&record).unwrap();
    let signature = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).sign(
        &qualitygate::adapters::attestation::pae(
            qualitygate::adapters::policy_approval::PAYLOAD_TYPE,
            &payload,
        ),
    );
    let envelope = json!({"payloadType":qualitygate::adapters::policy_approval::PAYLOAD_TYPE,"payload":STANDARD.encode(payload),"signatures":[{"keyid":"reviewer-key","sig":STANDARD.encode(signature.to_bytes())}]});
    std::fs::write(
        fixture.external.path().join("approval.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .unwrap();
    envelope
}

fn approve(fixture: &Fixture, code: i32) -> Value {
    run(
        fixture.root.path(),
        &[
            "policy",
            "candidate",
            "approve",
            &fixture.id,
            "--approval",
            fixture
                .external
                .path()
                .join("approval.json")
                .to_str()
                .unwrap(),
            "--trust-store",
            fixture.external.path().join("trust.json").to_str().unwrap(),
        ],
        code,
    )
}
fn promote(fixture: &Fixture, code: i32) -> Value {
    run(
        fixture.root.path(),
        &["policy", "candidate", "promote", &fixture.id],
        code,
    )
}

#[test]
fn only_independent_signed_exact_validation_can_activate_a_policy_and_history_is_immutable() {
    let mut fixture = Fixture::new();
    fixture.enable();
    promote(&fixture, 2);
    let evaluation = fixture.validate("4", 0);
    let report_ref = evaluation["evaluation"]["cases"][0]["baseline"]["report_ref"]
        .as_str()
        .unwrap();
    let retained = run(
        fixture.root.path(),
        &["policy", "record", report_ref, "--kind", "check_report"],
        0,
    );
    let log = &retained["record"]["checks"][0]["execution"]["artifacts"][0];
    let bytes = run(
        fixture.root.path(),
        &[
            "policy",
            "record",
            log["digest"].as_str().unwrap(),
            "--kind",
            "blob",
        ],
        0,
    );
    assert_eq!(
        STANDARD
            .decode(bytes["data"].as_str().unwrap())
            .unwrap()
            .len() as u64,
        log["bytes"].as_u64().unwrap()
    );
    std::fs::remove_dir_all(evaluation["evidence_directory"].as_str().unwrap()).unwrap();
    let expected = subject(&fixture);
    assert_eq!(expected.evaluation_ref, evaluation["evaluation_ref"]);
    let original = std::fs::read(fixture.root.path().join("qualitygate.yaml")).unwrap();
    sign(
        &fixture,
        expected.clone(),
        "reviewer",
        ManualDecision::Approved,
    );
    let approved = approve(&fixture, 0);
    assert_eq!(approved["revision"]["status"], "approved");
    let result = promote(&fixture, 0);
    assert_eq!(result["active_policy"], expected.candidate_policy);
    assert_eq!(
        std::fs::read(fixture.root.path().join("qualitygate.yaml")).unwrap(),
        original
    );
    let store = qualitygate::config::policy_store::Store::open(fixture.root.path()).unwrap();
    assert_eq!(
        store.index.active_policy.as_deref(),
        Some(expected.candidate_policy.as_str())
    );
    let before: qualitygate::domain::evolution::PolicyRevision = store
        .record(&expected.candidate_revision, "candidate")
        .unwrap();
    assert_eq!(
        before.status,
        qualitygate::domain::evolution::RevisionStatus::Candidate
    );
    let history = run(fixture.root.path(), &["policy", "history"], 0);
    assert_eq!(history["events"][0]["event"]["action"], "policy_promoted");
    let config = run(fixture.root.path(), &["config", "--show"], 0);
    assert_eq!(config["source"], expected.candidate_policy);
    assert_eq!(config["config"]["rules"]["line-ending"]["enabled"], true);
    let case = &fixture.suite.cases[0];
    let diff = format!("{}..{}", case.base, case.head);
    let checked = run(
        fixture.root.path(),
        &["check", "--diff", &diff, "--profile", "full"],
        1,
    );
    assert_eq!(checked["policy"]["trust"], "signed_active_policy");
    assert_eq!(checked["policy"]["source"], expected.candidate_policy);
    assert_eq!(checked["checks"][0]["verdict"], "fail");
    run(fixture.root.path(), &["rules", "disable", "line-ending"], 2);
    run(
        fixture.root.path(),
        &["check", "--diff", &diff, "--policy-ref", "HEAD"],
        2,
    );
    run(
        fixture.root.path(),
        &[
            "rules",
            "categories",
            "create",
            "view",
            "--description",
            "Navigation only",
        ],
        0,
    );
    let context = run(
        fixture.root.path(),
        &["rules", "context", "--category", "view"],
        0,
    );
    assert_eq!(context["mandatory"][0]["id"], "line-ending");
    assert_eq!(context["review_trust"], "signed_active_policy");
    assert!(context["selected"].as_array().unwrap().is_empty());
    fixture.new_candidate();
    fixture.enable();
    fixture.validate("2", 0);
    sign(
        &fixture,
        subject(&fixture),
        "reviewer",
        ManualDecision::Approved,
    );
    approve(&fixture, 0);
    let stale = promote(&fixture, 2);
    assert!(
        stale["gate"]["blockers"]
            .to_string()
            .contains("Active policy changed")
    );
    rollback_to_parent(&mut fixture);
}

fn rollback_to_parent(fixture: &mut Fixture) {
    use qualitygate::domain::policy_rollback::{RollbackApproval, RollbackSubject};
    let trust = fixture.external.path().join("trust.json");
    let approval_path = fixture.external.path().join("rollback.json");
    let request = || -> RollbackSubject {
        serde_json::from_value(
            run(
                fixture.root.path(),
                &[
                    "policy",
                    "rollback-subject",
                    "--to",
                    &fixture.baseline,
                    "--trust-store",
                    trust.to_str().unwrap(),
                    "--actor",
                    "generator",
                ],
                0,
            )["subject"]
                .clone(),
        )
        .unwrap()
    };
    let create_signature = |subject: RollbackSubject| {
        let now = qualitygate::config::policy_store::now().unwrap();
        let payload = serde_json::to_vec(&RollbackApproval {
            schema_version: 1,
            subject,
            approver: Actor {
                id: "reviewer".into(),
                kind: ActorKind::Human,
            },
            decision: ManualDecision::Approved,
            issued_at: now,
            expires_at: now + 300,
            reason: "Restore the previously trusted baseline".into(),
        })
        .unwrap();
        let payload_type = qualitygate::adapters::policy_rollback::PAYLOAD_TYPE;
        let signature = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).sign(
            &qualitygate::adapters::attestation::pae(payload_type, &payload),
        );
        let envelope = serde_json::to_vec(
            &json!({"payloadType":payload_type,"payload":STANDARD.encode(payload),
            "signatures":[{"keyid":"reviewer-key","sig":STANDARD.encode(signature.to_bytes())}]}),
        )
        .unwrap();
        std::fs::write(&approval_path, &envelope).unwrap();
        envelope
    };
    create_signature(request());
    qualitygate::config::policy_candidates::create_from_version(
        fixture.root.path(),
        &fixture.baseline,
        qualitygate::config::policy_candidates::Proposal {
            actor: Actor {
                id: "generator".into(),
                kind: ActorKind::Agent,
            },
            reason: "New event invalidates a previously signed transition".into(),
            evidence: fixture.suite.motivating_evidence.clone(),
        },
    )
    .unwrap();
    let argv = [
        "policy",
        "rollback",
        "--to",
        &fixture.baseline,
        "--trust-store",
        trust.to_str().unwrap(),
        "--approval",
        approval_path.to_str().unwrap(),
    ];
    run(fixture.root.path(), &argv, 2);
    let envelope = create_signature(request());
    let rolled_back = run(fixture.root.path(), &argv, 0);
    assert_eq!(rolled_back["active_policy"], fixture.baseline);
    assert_eq!(
        run(fixture.root.path(), &["config", "--show"], 0)["config"]["rules"]["line-ending"]["enabled"],
        false
    );
    let history = run(fixture.root.path(), &["policy", "history"], 0);
    assert_eq!(
        history["events"][0]["event"]["action"],
        "policy_rolled_back"
    );
    run(fixture.root.path(), &argv, 2); // A signature cannot be replayed against the new state.
    fixture
        .trust
        .revoked_approvals
        .push(qualitygate::config::policy_store::digest(&envelope));
    fixture.write_inputs();
    run(fixture.root.path(), &["config", "--show"], 2);
}

#[test]
fn self_approval_tampering_revocation_and_changed_candidates_cannot_promote() {
    let mut fixture = Fixture::new();
    fixture.trust.approval_keys[0].actor.id = "generator".into();
    fixture.write_inputs();
    fixture.enable();
    fixture.validate("4", 0);
    sign(
        &fixture,
        subject(&fixture),
        "generator",
        ManualDecision::Approved,
    );
    assert!(
        approve(&fixture, 2)["gate"]["blockers"]
            .to_string()
            .contains("self-approval")
    );
    fixture.trust.approval_keys[0].actor.id = "reviewer".into();
    fixture.write_inputs();
    fixture.validate("4", 0);
    let expected = subject(&fixture);
    let mut envelope = sign(
        &fixture,
        expected.clone(),
        "reviewer",
        ManualDecision::Approved,
    );
    envelope["signatures"][0]["sig"] = json!(STANDARD.encode([0_u8; 64]));
    std::fs::write(
        fixture.external.path().join("approval.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .unwrap();
    approve(&fixture, 2);
    sign(&fixture, expected, "reviewer", ManualDecision::Approved);
    run(
        fixture.root.path(),
        &[
            "policy",
            "candidate",
            "rules",
            "configure",
            &fixture.id,
            "line-ending",
            "--severity",
            "warning",
            "--actor",
            "generator",
        ],
        0,
    );
    assert!(
        approve(&fixture, 2)["gate"]["blockers"]
            .to_string()
            .contains("no validation record")
    );
    fixture.validate("4", 0);
    let envelope = sign(
        &fixture,
        subject(&fixture),
        "reviewer",
        ManualDecision::Approved,
    );
    approve(&fixture, 0);
    fixture
        .trust
        .revoked_approvals
        .push(qualitygate::config::policy_store::digest(
            &serde_json::to_vec(&envelope).unwrap(),
        ));
    fixture.write_inputs();
    promote(&fixture, 2);
    assert!(
        qualitygate::config::policy_store::Store::open(fixture.root.path())
            .unwrap()
            .index
            .active_policy
            .is_none()
    );
}
