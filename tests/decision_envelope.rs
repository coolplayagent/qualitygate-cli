mod common;

use common::{cli, fixture, report};
use qualitygate::domain::decision_envelope::{CommandKind, DecisionEnvelope};
#[cfg(unix)]
use qualitygate::domain::{ExecutionStatus, Report};
use serde_json::{Value, json};
#[cfg(unix)]
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::fs;

fn exported(root: &std::path::Path, name: &str) -> Value {
    report(&cli(root, &["schema", name, "--format", "json"]), 0)
}

#[test]
fn metadata_envelopes_reject_malformed_evidence_and_identity_fields() {
    let transition = DecisionEnvelope::from_metadata(
        CommandKind::PolicyTransition,
        json!({"policy_digest":format!("sha256:{}", "1".repeat(64)),"status":"candidate"}),
        0,
    )
    .unwrap();
    assert_eq!(transition.gate.outcome, qualitygate::domain::Decision::Pass);
    DecisionEnvelope::parse(&serde_json::to_vec(&transition).unwrap()).unwrap();
    let incomplete =
        DecisionEnvelope::from_metadata(CommandKind::Pilot, json!({"issues":["missing"]}), 2)
            .unwrap();
    assert_eq!(
        incomplete.gate.outcome,
        qualitygate::domain::Decision::Incomplete
    );
    assert!(!incomplete.execution.complete);
    assert!(
        DecisionEnvelope::from_metadata(
            CommandKind::Pilot,
            json!({"artifacts":[{"path":"report.json","digest":"bad","bytes":2}]}),
            0,
        )
        .is_err()
    );
    assert!(
        DecisionEnvelope::from_metadata(
            CommandKind::PolicyTransition,
            json!({"policy_digest":17}),
            0,
        )
        .is_err()
    );
}

#[test]
fn decision_schema_and_discriminants_reject_unknown_versions_variants_and_partial_states() {
    let repository = fixture();
    let root = repository.path();
    let schema = exported(root, "decision");
    assert_eq!(schema["$id"], "urn:qualitygate:decision:1");
    let validator = jsonschema::draft202012::options().build(&schema).unwrap();
    let selected = report(
        &cli(
            root,
            &[
                "selfcheck",
                "--fixture",
                "minimal",
                "--rule",
                "no-emoji",
                "--envelope",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(selected["command_kind"], "selfcheck");
    assert_eq!(selected["gate"]["outcome"], "pass");
    assert!(validator.is_valid(&selected));
    DecisionEnvelope::parse(&serde_json::to_vec(&selected).unwrap()).unwrap();
    for (pointer, replacement) in [
        ("/protocol_version", json!("2.0.0")),
        ("/command_kind", json!("unknown")),
        ("/gate/outcome", json!("incomplete")),
        ("/execution/complete", json!(false)),
        ("/subject/source_digest", json!("sha256:unbound")),
        ("/gate/warning_count", json!(1)),
        ("/payload/decision", json!("incomplete")),
        (
            "/decision_id",
            json!("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
        ),
    ] {
        let mut corrupted = selected.clone();
        *corrupted.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            DecisionEnvelope::parse(&serde_json::to_vec(&corrupted).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let mut unknown = selected.clone();
    unknown["new_required_field"] = json!(true);
    assert!(!validator.is_valid(&unknown));
    assert!(DecisionEnvelope::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#[test]
#[cfg(unix)]
fn check_and_rule_validation_envelopes_bind_outcomes_without_losing_warnings_or_gaps() {
    let repository = fixture();
    let root = repository.path();
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - {id: first, argv: [sh, -c, 'exit 0'], severity: warning}\n  - {id: second, argv: [sh, -c, 'exit 1']}\n").unwrap();
    let check = report(
        &cli(
            root,
            &["check", "--worktree", "--envelope", "--format", "json"],
        ),
        1,
    );
    assert_eq!(check["command_kind"], "check");
    assert_eq!(check["gate"]["outcome"], "fail");
    assert_eq!(check["gate"]["route"], "block");
    assert_eq!(check["payload"]["gate"]["decision"], "fail");
    assert_eq!(
        check["subject"]["snapshot_digest"],
        check["payload"]["snapshot"]["verification_digest"]
    );
    DecisionEnvelope::parse(&serde_json::to_vec(&check).unwrap()).unwrap();
    let mut rebound: DecisionEnvelope = serde_json::from_value(check.clone()).unwrap();
    rebound.policy.evaluator_digest = Some(format!("sha256:{}", "0".repeat(64)));
    rebound.decision_id.clear();
    rebound.decision_id = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&rebound).unwrap())
    );
    assert!(DecisionEnvelope::parse(&serde_json::to_vec(&rebound).unwrap()).is_err());
    let validation = report(
        &cli(
            root,
            &["rules", "validate", "--envelope", "--format", "json"],
        ),
        2,
    );
    assert_eq!(validation["command_kind"], "rule_validation");
    assert_eq!(validation["gate"]["outcome"], "incomplete");
    assert_eq!(validation["gate"]["execution_gap_count"], 1);
    DecisionEnvelope::parse(&serde_json::to_vec(&validation).unwrap()).unwrap();
}

#[test]
#[cfg(unix)]
fn exported_feedback_schema_accepts_bounded_existing_feedback() {
    let repository = fixture();
    let root = repository.path();
    let schema = exported(root, "feedback");
    let validator = jsonschema::draft202012::options().build(&schema).unwrap();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nchecks:\n  - {id: build, argv: [sh, -c, 'exit 0']}\n",
    )
    .unwrap();
    let feedback = report(&cli(root, &["check", "--feedback", "--format", "json"]), 0);
    assert!(validator.is_valid(&feedback));
    let mut unknown = feedback.clone();
    unknown["unknown_required_feature"] = json!(true);
    assert!(!validator.is_valid(&unknown));
    let mut false_pass = feedback.clone();
    false_pass["gate"]["complete"] = json!(false);
    assert!(!validator.is_valid(&false_pass));
    let wrapped = cli(
        root,
        &[
            "check",
            "--feedback",
            "--feedback-max-bytes",
            "8192",
            "--envelope",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        wrapped.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&wrapped.stdout)
    );
    assert!(wrapped.stdout.len() <= 8192);
    let envelope: Value = serde_json::from_slice(&wrapped.stdout).unwrap();
    assert_eq!(envelope["command_kind"], "feedback");
    assert_eq!(
        envelope["evidence_refs"][0]["digest"],
        envelope["payload"]["full_report"]["digest"]
    );
    DecisionEnvelope::parse(&wrapped.stdout).unwrap();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nchecks:\n  - {id: missing, argv: [qualitygate-missing-producer]}\n",
    )
    .unwrap();
    let incomplete = report(
        &cli(
            root,
            &["check", "--feedback", "--envelope", "--format", "json"],
        ),
        2,
    );
    assert_eq!(incomplete["execution"]["complete"], false);
    assert_eq!(incomplete["gate"]["outcome"], "incomplete");
    assert_eq!(incomplete["gate"]["route"], "inspect_gap");
    assert!(validator.is_valid(&incomplete["payload"]));
}

#[test]
#[cfg(unix)]
fn optional_execution_gap_requires_review_even_when_gate_passes() {
    let repository = fixture();
    let root = repository.path();
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nchecks:\n  - {id: passing, argv: [sh, -c, 'exit 0']}\n",
    )
    .unwrap();
    let value = report(&cli(root, &["check", "--worktree", "--format", "json"]), 0);
    let mut checked: Report = serde_json::from_value(value).unwrap();
    checked.checks[0].required = false;
    checked.checks[0].execution.status = ExecutionStatus::ToolError;
    checked.checks[0].verdict = None;
    let envelope = DecisionEnvelope::from_check(&checked).unwrap();
    assert_eq!(envelope.gate.execution_gap_count, 1);
    assert_eq!(
        envelope.gate.route,
        qualitygate::domain::decision_envelope::Route::Review
    );
    DecisionEnvelope::parse(&serde_json::to_vec(&envelope).unwrap()).unwrap();
}
