use super::*;
use crate::domain::{CheckResult, Diagnostic, Recheck, Severity, Summary, evaluate};
use serde_json::json;

fn report() -> Report {
    serde_json::from_value(json!({
        "schema_version": 1, "run_id": "run", "scope": "delivery", "profile": "full",
        "evaluator_digest": digest(b"evaluator"),
        "snapshot": {"mode": "worktree", "base": "base", "head": "head",
            "content_digest": digest(b"content"), "verification_digest": digest(b"scope")},
        "policy": {"source": "trusted-ref", "trust": "caller_supplied_ref",
            "config_digest": digest(b"policy"), "rules_digest": digest(b"rules"),
            "task_contract_digest": digest(b"task"), "changes": []},
        "plan": {"required_checks": [], "pending_delivery_checks": [], "acceptance": {}},
        "gate": {"complete": true, "decision": "pass", "blockers": []},
        "checks": [], "summary": Summary::default()
    }))
    .unwrap()
}

fn artifact() -> Artifact {
    Artifact {
        path: "evidence/report.json".into(),
        digest: digest(b"report"),
        bytes: 17,
    }
}

fn diagnostic() -> Diagnostic {
    Diagnostic {
        id: "review".into(),
        fingerprint: "review-1".into(),
        file: None,
        range: None,
        message: "Review this finding".into(),
        evidence: json!({"count": 1}),
        fix: "Inspect the source".into(),
        recheck: Recheck::default(),
    }
}

fn rebound(envelope: &mut DecisionEnvelope) {
    envelope.decision_id = identity(envelope).unwrap();
}

fn rejects_rebound(mut envelope: DecisionEnvelope, message: &str) {
    rebound(&mut envelope);
    assert!(
        envelope
            .validate()
            .unwrap_err()
            .to_string()
            .contains(message)
    );
}

#[test]
fn check_routes_distinguish_review_violation_timeout_and_pending_delivery() {
    let clean = report();
    let accepted = DecisionEnvelope::from_check(&clean).unwrap();
    assert_eq!(accepted.gate.route, Route::Accept);
    assert_eq!(
        accepted.subject.snapshot_digest,
        clean.snapshot.verification_digest
    );
    assert_eq!(
        accepted.subject.task_digest,
        clean.policy.task_contract_digest
    );
    assert_eq!(accepted.policy.reference.as_deref(), Some("trusted-ref"));
    let parsed = DecisionEnvelope::parse(&serde_json::to_vec(&accepted).unwrap()).unwrap();
    assert_eq!(parsed.decision_id, accepted.decision_id);

    for (severity, status, expected) in [
        (Severity::Warning, ExecutionStatus::Completed, Route::Review),
        (Severity::Error, ExecutionStatus::Completed, Route::Block),
        (
            Severity::Error,
            ExecutionStatus::TimedOut,
            Route::InspectGap,
        ),
    ] {
        let mut source = clean.clone();
        let mut check = CheckResult::pending("analysis", true, severity);
        check.diagnostics.push(diagnostic());
        check.execution.artifacts.push(artifact());
        if status == ExecutionStatus::Completed {
            check.complete();
        } else {
            check.block(status, "deadline reached");
        }
        source.checks.push(check);
        source.gate = evaluate(&source.checks, &[], &[]);
        let envelope = DecisionEnvelope::from_check(&source).unwrap();
        assert_eq!(envelope.gate.route, expected);
        assert_eq!(envelope.evidence_refs[0].digest, artifact().digest);
        assert_eq!(
            envelope.gate.warning_count,
            usize::from(severity == Severity::Warning)
        );
        assert_eq!(
            envelope.gate.execution_gap_count,
            usize::from(status != ExecutionStatus::Completed)
        );
    }
    let mut pending = clean;
    pending
        .plan
        .pending_delivery_checks
        .push("slow-tests".into());
    let mut skipped = CheckResult::pending("not-applicable", false, Severity::Info);
    skipped.skip("No matching sources");
    pending.checks.push(skipped);
    pending.snapshot.verification_digest = None;
    let envelope = DecisionEnvelope::from_check(&pending).unwrap();
    assert_eq!(envelope.gate.route, Route::Review);
    assert_eq!(envelope.gate.execution_gap_count, 0);
    assert_eq!(envelope.gate.pending_check_count, 1);
    assert_eq!(
        envelope.subject.snapshot_digest,
        Some(pending.snapshot.content_digest)
    );
}

#[test]
fn rebinding_an_envelope_cannot_hide_changed_check_evidence() {
    let envelope = DecisionEnvelope::from_check(&report()).unwrap();
    for (pointer, value) in [
        ("/payload/gate/decision", json!("fail")),
        ("/payload/gate/complete", json!(false)),
        (
            "/payload/snapshot/verification_digest",
            json!(digest(b"another scope")),
        ),
        (
            "/payload/policy/task_contract_digest",
            json!(digest(b"another task")),
        ),
        ("/payload/policy/source", json!("other-ref")),
        (
            "/payload/policy/config_digest",
            json!(digest(b"other policy")),
        ),
        ("/payload/policy/trust", json!("local")),
        (
            "/payload/evaluator_digest",
            json!(digest(b"other evaluator")),
        ),
        ("/execution/omissions", json!(["hidden"])),
        ("/gate/warning_count", json!(1)),
        ("/gate/execution_gap_count", json!(1)),
        ("/gate/pending_check_count", json!(1)),
        ("/evidence_refs", json!([artifact()])),
        ("/verification/conclusion", json!("unearned claim")),
    ] {
        let mut serialized = serde_json::to_value(&envelope).unwrap();
        *serialized.pointer_mut(pointer).unwrap() = value;
        let mut changed: DecisionEnvelope = serde_json::from_value(serialized).unwrap();
        changed.gate.route = route(
            changed.gate.outcome,
            changed.gate.warning_count,
            changed.gate.pending_check_count,
            changed.gate.execution_gap_count,
        );
        rejects_rebound(changed, "Check payload conflicts");
    }
}

#[test]
fn metadata_outcomes_preserve_bindings_artifacts_and_omissions() {
    for kind in [CommandKind::Pilot, CommandKind::PolicyTransition] {
        for (code, outcome, expected_route) in [
            (0, Decision::Pass, Route::Accept),
            (1, Decision::Fail, Route::Block),
            (2, Decision::Incomplete, Route::InspectGap),
        ] {
            let payload = json!({"snapshot_digest": digest(b"snapshot"),
                "task_digest": digest(b"task"), "policy_ref": "reviewed",
                "policy_digest": digest(b"policy"), "evaluator_digest": digest(b"runtime"),
                "review_trust": "signed", "artifacts": [artifact()], "omissions": ["unavailable"]});
            let envelope = DecisionEnvelope::from_metadata(kind, payload.clone(), code).unwrap();
            assert_eq!(envelope.gate.outcome, outcome);
            assert_eq!(envelope.gate.route, expected_route);
            assert_eq!(envelope.execution.complete, code != 2);
            assert_eq!(envelope.execution.omissions, ["unavailable"]);
            assert_eq!(envelope.policy.trust, "signed");
            assert_eq!(envelope.payload, payload);
            assert_eq!(envelope.evidence_refs.len(), 1);
            let mut changed = envelope.clone();
            changed.payload["new_evidence"] = json!(true);
            rejects_rebound(changed, "Metadata source digest");
        }
    }
    let minimal = DecisionEnvelope::from_metadata(CommandKind::Pilot, json!({}), 0).unwrap();
    assert_eq!(minimal.policy.trust, "local_candidate");
    assert!(minimal.evidence_refs.is_empty());
    assert!(minimal.execution.omissions.is_empty());
    for key in [
        "snapshot_digest",
        "task_digest",
        "policy_ref",
        "policy_digest",
        "evaluator_digest",
        "review_trust",
    ] {
        let mut payload = json!({});
        payload[key] = json!(42);
        assert!(
            DecisionEnvelope::from_metadata(CommandKind::Pilot, payload, 0)
                .unwrap_err()
                .to_string()
                .contains("must be a string")
        );
    }
    for (kind, payload, code) in [
        (CommandKind::Check, json!({}), 0),
        (CommandKind::Pilot, json!([]), 0),
        (CommandKind::Pilot, json!({}), 3),
        (CommandKind::Pilot, json!({"artifacts": false}), 0),
        (CommandKind::Pilot, json!({"omissions": 1}), 0),
    ] {
        assert!(DecisionEnvelope::from_metadata(kind, payload, code).is_err());
    }
}

#[test]
fn validation_rejects_corruption_before_trusting_any_payload() {
    let envelope = DecisionEnvelope::from_metadata(CommandKind::Pilot, json!({}), 0).unwrap();
    let mut changed = envelope.clone();
    changed.schema_uri = "unknown".into();
    rejects_rebound(changed, "Unknown decision schema");
    let mut changed = envelope.clone();
    changed.protocol_version = "2".into();
    rejects_rebound(changed, "Unknown decision schema");
    for bad in [
        "",
        "sha256:abc",
        &format!("sha256:{}", "A".repeat(64)),
        &format!("sha256:{}", "z".repeat(64)),
    ] {
        let mut changed = envelope.clone();
        changed.subject.snapshot_digest = Some(bad.into());
        rejects_rebound(changed, "digest or evidence");
    }
    let mut changed = envelope.clone();
    changed.evidence_refs.push(Artifact {
        path: String::new(),
        ..artifact()
    });
    rejects_rebound(changed, "digest or evidence");
    let mut changed = envelope.clone();
    changed.policy.trust = "forged".into();
    assert!(
        changed
            .validate()
            .unwrap_err()
            .to_string()
            .contains("identity")
    );
    let mut changed = envelope.clone();
    changed.execution.complete = false;
    rejects_rebound(changed, "Incomplete execution");
    let mut changed = envelope;
    changed.gate.route = Route::Block;
    rejects_rebound(changed, "route conflicts");
    assert!(DecisionEnvelope::parse(b"not json").is_err());
}

#[test]
fn validation_and_selfcheck_outcomes_bind_their_own_sources() {
    for decision in [Decision::Pass, Decision::Fail, Decision::Incomplete] {
        let complete = decision != Decision::Incomplete;
        let validation = RuleValidationReport {
            schema_version: 1,
            schema_id: "rule-schema".into(),
            schema_digest: digest(b"schema"),
            complete,
            decision,
            files: vec![],
            issues: vec![],
            review_trust: "unreviewed".into(),
        };
        let envelope = DecisionEnvelope::from_rule_validation(&validation).unwrap();
        assert_eq!(envelope.command_kind, CommandKind::RuleValidation);
        assert_eq!(envelope.gate.execution_gap_count, usize::from(!complete));
        assert_eq!(envelope.policy.reference.as_deref(), Some("rule-schema"));
        let mut changed = envelope;
        changed.payload["review_trust"] = json!("signed");
        rejects_rebound(changed, "Rule validation payload conflicts");

        let source = SelfcheckReport {
            schema_version: 1,
            tool_version: "test".into(),
            corpus_digest: digest(b"corpus"),
            rules_digest: digest(b"rules"),
            fixture_filter: None,
            rule_filter: None,
            decision,
            complete,
            fixtures: vec![],
            errors: if complete {
                vec![]
            } else {
                vec!["missing fixture".into()]
            },
            verification: VerificationBoundary::default(),
        };
        let envelope = DecisionEnvelope::from_selfcheck(&source).unwrap();
        assert_eq!(envelope.subject.source_digest, Some(source.corpus_digest));
        assert_eq!(envelope.policy.trust, "bundled");
        assert_eq!(envelope.gate.execution_gap_count, usize::from(!complete));
        let mut changed = envelope;
        changed.payload["rules_digest"] = json!(digest(b"changed rules"));
        rejects_rebound(changed, "Selfcheck payload conflicts");
    }
}

fn feedback(report: &Report) -> Value {
    let mut payload = json!({"kind": "agent_feedback", "run_id": report.run_id,
        "gate": report.gate, "snapshot": report.snapshot, "full_report": artifact()});
    for key in [
        "findings",
        "execution_gaps",
        "blockers",
        "pending_delivery_checks",
        "acceptance",
    ] {
        payload[key] = json!({"items": [{"report_pointer": "/checks/0"}], "omitted": 0});
    }
    payload
}

#[test]
fn compact_feedback_preserves_full_evidence_and_counts_removed_previews_once() {
    let mut source = report();
    source
        .verification
        .verified_shapes
        .push("checked source".into());
    let mut payload = feedback(&source);
    payload["findings"]["omitted"] = json!(2);
    let mut envelope = DecisionEnvelope::from_feedback(&source, payload).unwrap();
    let original_id = envelope.decision_id.clone();
    envelope.compact_feedback().unwrap();
    assert_ne!(envelope.decision_id, original_id);
    assert_eq!(envelope.payload["findings"]["omitted"], 3);
    assert_eq!(envelope.payload["acceptance"]["omitted"], 1);
    assert_eq!(envelope.payload["truncated"], true);
    assert_eq!(envelope.evidence_refs[0].digest, artifact().digest);
    assert_eq!(
        envelope
            .execution
            .omissions
            .iter()
            .filter(|s| *s == "findings")
            .count(),
        1
    );
    assert!(
        envelope
            .execution
            .omissions
            .iter()
            .any(|s| s == "verification_shapes")
    );
    let compact_id = envelope.decision_id.clone();
    envelope.compact_feedback().unwrap();
    assert_eq!(envelope.decision_id, compact_id);
    assert!(
        DecisionEnvelope::from_check(&source)
            .unwrap()
            .compact_feedback()
            .is_err()
    );
}

#[test]
fn feedback_cannot_claim_a_different_run_snapshot_gate_or_omission_inventory() {
    let source = report();
    for (key, value) in [
        ("kind", json!("check")),
        ("run_id", json!("other")),
        ("gate", json!({"decision": "fail", "complete": true})),
        ("full_report", json!(null)),
    ] {
        let mut payload = feedback(&source);
        payload[key] = value;
        assert!(DecisionEnvelope::from_feedback(&source, payload).is_err());
    }
    let envelope = DecisionEnvelope::from_feedback(&source, feedback(&source)).unwrap();
    let mut changed = envelope.clone();
    changed.payload["snapshot"]["verification_digest"] = json!(digest(b"other snapshot"));
    rejects_rebound(changed, "Feedback payload conflicts");
    let mut changed = envelope.clone();
    changed.payload["findings"]["omitted"] = json!(1);
    rejects_rebound(changed, "omission count conflicts");
    let mut changed = envelope;
    changed.payload["acceptance"]["omitted"] = json!(null);
    rejects_rebound(changed, "omission count conflicts");
}
