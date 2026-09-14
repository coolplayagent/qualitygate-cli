//! Policy fixtures invoke production oracles and signature verification with independent goldens.

use crate::{
    adapters::{attestation, policy_approval, policy_rollback},
    config::{
        policy_acceptance::{ApprovalKey, EvolutionTrust},
        policy_store::digest,
        selfcheck_policy::{AuthorizationChange, EvolutionFixture, Pair, ReportInput},
    },
    domain::{
        self,
        evolution::{Actor, ActorKind},
        policy_evaluation::*,
        *,
    },
};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub(super) fn observe(input: &EvolutionFixture) -> Result<Value> {
    match input {
        EvolutionFixture::Context { content, category } => {
            Ok(outcome(crate::config::rule_query::context_from_files(
                "qualitygate.yaml",
                [("qualitygate.yaml", content.as_bytes())],
                category.as_deref(),
            )))
        }
        EvolutionFixture::Acceptance { content } => {
            let result =
                crate::config::policy_acceptance::parse_suite(content.as_bytes()).map(|_| ());
            Ok(outcome(result))
        }
        EvolutionFixture::Paired {
            pairs,
            expected_count,
            min_improvements,
            invalid,
        } => paired(pairs, *expected_count, *min_improvements, invalid),
        EvolutionFixture::Authorization { rollback, change } => authorization(*rollback, *change),
        EvolutionFixture::Candidate { action } => super::selfcheck_policy_io::candidate(*action),
        EvolutionFixture::Workflow { scenario, jobs } => tokio::runtime::Handle::current()
            .block_on(super::selfcheck_policy_io::workflow(*scenario, *jobs)),
    }
}

pub(super) fn actor(id: &str) -> Actor {
    Actor {
        id: id.into(),
        kind: ActorKind::Agent,
    }
}

fn report(input: &ReportInput) -> Report {
    Report {
        schema_version: 1,
        run_id: "fixture".into(),
        scope: "task".into(),
        profile: "full".into(),
        evaluator_digest: digest(b"fixture evaluator"),
        environment_digest: digest(b"fixture environment"),
        snapshot: SnapshotIdentity {
            mode: "diff".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: digest(b"fixture snapshot"),
            merge_request: None,
        },
        policy: PolicyEvidence {
            source: "fixture policy".into(),
            resolved_commit: None,
            config_digest: digest(b"config"),
            rules_digest: digest(b"rules"),
            task_contract_digest: Some(digest(b"task")),
            task_contract_source: None,
            trust: "synthetic fixture".into(),
            changes: Vec::new(),
            source_reviews: BTreeMap::new(),
        },
        plan: PlanSummary {
            task_id: Some("fixture-task".into()),
            execution_order: input.checks.iter().map(|check| check.id.clone()).collect(),
            required_checks: input.required.clone(),
            pending_delivery_checks: input.pending.clone(),
            acceptance: BTreeMap::new(),
            acceptance_descriptions: BTreeMap::new(),
        },
        gate: domain::evaluate(&input.checks, &input.required, &input.invalid),
        summary: Summary::from_checks(&input.checks),
        checks: input.checks.clone(),
        verification: VerificationBoundary::default(),
    }
}

fn paired(
    pairs: &[Pair],
    expected_count: usize,
    min_improvements: usize,
    invalid: &[String],
) -> Result<Value> {
    let mut invalid = invalid.to_vec();
    let mut gates = Vec::new();
    let cases = pairs
        .iter()
        .enumerate()
        .map(|(index, pair)| {
            let baseline = report(&pair.baseline);
            let candidate = report(&pair.candidate);
            if !matched_producers(
                &producer_environment(&baseline),
                &producer_environment(&candidate),
            ) {
                invalid.push(format!("{index}: paired producer identities differ"));
            }
            gates.push(json!({"baseline":baseline.gate,"candidate":candidate.gate}));
            EvaluatedCase {
                id: format!("case-{index}"),
                kind: pair.kind,
                base: format!("base-{index}"),
                head: format!("head-{index}"),
                snapshot_digest: pair.snapshot_present.then(|| digest(b"fixture snapshot")),
                task_digest: digest(b"fixture task"),
                baseline: Observation::from_report(
                    &baseline,
                    &pair.expectations,
                    pair.baseline.duration_ms,
                ),
                candidate: Observation::from_report(
                    &candidate,
                    &pair.expectations,
                    pair.candidate.duration_ms,
                ),
            }
        })
        .collect::<Vec<_>>();
    let (conclusion, reasons) = decide(&cases, expected_count, min_improvements, &invalid);
    let run = EvaluationRun {
        schema_version: 1,
        candidate_id: "fixture-candidate".into(),
        candidate_revision: digest(b"revision"),
        candidate_policy: digest(b"candidate"),
        baseline_policy: digest(b"baseline"),
        suite_digest: digest(b"suite"),
        trust_digest: digest(b"trust"),
        evaluator_epoch: "fixture-epoch".into(),
        evaluator_digest: digest(b"evaluator"),
        environment_digest: digest(b"environment"),
        budget: budget(),
        jobs: 2,
        generation_actor: actor("generator"),
        evaluation_actor: actor("operator"),
        started_at: 1,
        ended_at: 2,
        cases,
        conclusion,
        reasons,
        verification: VerificationBoundary::default(),
    };
    Ok(
        json!({"conclusion":conclusion,"exit_code":conclusion.exit_code(),"reasons":run.reasons,
        "cases":run.cases,"gates":gates,"effectiveness":domain::policy_effectiveness::measure(digest(b"evaluation"), &run)}),
    )
}

pub(super) fn budget() -> EvaluationBudget {
    EvaluationBudget {
        snapshot_max_mib: 16,
        snapshot_jobs: 2,
        snapshot_timeout_seconds: 10,
        max_live_snapshot_mib: 64,
        max_parallel: 4,
        case_timeout_seconds: 10,
        total_timeout_seconds: 30,
    }
}

pub(super) fn trust(
    root: &Path,
    suite: String,
    baseline: String,
    evaluator: String,
) -> EvolutionTrust {
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

/// Public deterministic test key; only signs disposable fixture subjects, never user input.
pub(super) fn envelope(
    payload_type: &str,
    record: &Value,
    tamper: bool,
    unknown_key: bool,
) -> Result<Vec<u8>> {
    let key = SigningKey::from_bytes(&[7; 32]);
    let payload = serde_json::to_vec(record)?;
    let mut signature = key
        .sign(&attestation::pae(payload_type, &payload))
        .to_bytes();
    if tamper {
        signature[0] ^= 1;
    }
    Ok(serde_json::to_vec(
        &json!({"payloadType":payload_type,"payload":STANDARD.encode(payload),
        "signatures":[{"keyid":if unknown_key {"unknown"} else {"fixture-key"},"sig":STANDARD.encode(signature)}]}),
    )?)
}

pub(super) fn outcome<T: serde::Serialize>(result: Result<T>) -> Value {
    match result {
        Ok(value) => json!({"status":"completed","value":value}),
        Err(error) => json!({"status":"incomplete","reason":format!("{error:#}")}),
    }
}

fn authorization(rollback: bool, change: AuthorizationChange) -> Result<Value> {
    use AuthorizationChange::*;
    let directory = tempfile::tempdir()?;
    let root = dunce::canonicalize(directory.path())?;
    let mut trust = trust(
        &root,
        digest(b"suite"),
        digest(b"baseline"),
        digest(b"evaluator"),
    );
    let subject = if rollback {
        json!({"repository":trust.repository,"from_policy":digest(b"current"),"from_authorization":digest(b"approval"),
            "from_since":1,"to_policy":digest(b"baseline"),"history_cursor":digest(b"cursor"),
            "target_history_ref":digest(b"target-event"),"trust_digest":digest(b"trust"),
            "evaluator_digest":digest(b"evaluator"),"proposed_by":actor("generator")})
    } else {
        json!({"repository":trust.repository,"candidate_id":"fixture-candidate","candidate_revision":digest(b"revision"),
            "candidate_policy":digest(b"candidate"),"parent_policy":digest(b"baseline"),"evaluation_ref":digest(b"run"),
            "suite_digest":digest(b"suite"),"trust_digest":digest(b"trust"),"evaluator_digest":digest(b"evaluator"),
            "generation_actor":actor("generator")})
    };
    if matches!(change, SelfApproval) {
        trust.approval_keys[0].actor.id = "generator".into();
    }
    if matches!(change, AgentApprover) {
        trust.approval_keys[0].actor.kind = ActorKind::Agent;
    }
    let mut record = json!({"schema_version":1,"subject":subject,"approver":trust.approval_keys[0].actor,
        "decision":"approved","issued_at":100,"expires_at":200,"reason":"Synthetic fixture review"});
    match change {
        Rejected => record["decision"] = json!("rejected"),
        WrongIdentity => record["approver"]["id"] = json!("impostor"),
        WrongSubject => {
            record["subject"][if rollback {
                "history_cursor"
            } else {
                "candidate_revision"
            }] = json!(digest(b"stale"))
        }
        Expired => record["expires_at"] = json!(149),
        Future => record["issued_at"] = json!(151),
        ExcessiveLifetime => record["expires_at"] = json!(3701),
        _ => {}
    }
    let payload_type = if matches!(change, WrongPayloadType) {
        attestation::PAYLOAD_TYPE
    } else if rollback {
        policy_rollback::PAYLOAD_TYPE
    } else {
        policy_approval::PAYLOAD_TYPE
    };
    let bytes = envelope(
        payload_type,
        &record,
        matches!(change, TamperedSignature),
        matches!(change, UnknownKey),
    )?;
    if matches!(change, Revoked) {
        trust.revoked_approvals.push(digest(&bytes));
    }
    let verified = if rollback {
        policy_rollback::verify(&bytes, &trust, &serde_json::from_value(subject)?, 150)
            .map(|verified| json!({"decision":verified.approval.decision,"signer":verified.signer_key_id}))
    } else {
        policy_approval::verify(&bytes, &trust, &serde_json::from_value(subject)?, 150)
            .map(|verified| json!({"decision":verified.approval.decision,"signer":verified.signer_key_id}))
    };
    Ok(outcome(verified))
}
