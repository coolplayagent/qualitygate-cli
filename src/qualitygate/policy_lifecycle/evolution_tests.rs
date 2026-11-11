use super::*;
use serde_json::json;

fn record() -> EvidenceRecord {
    EvidenceRecord {
        case: None,
        schema_version: 1,
        kind: EvidenceKind::Review,
        source_digest: format!("sha256:{}", "a".repeat(64)),
        scope: "project".into(),
        timestamp: 1,
        actor: Actor {
            id: "reviewer".into(),
            kind: ActorKind::Human,
        },
        claims: vec!["Repeated false positive".into()],
        sensitivity: Sensitivity::Internal,
        rule_ids: vec!["line-ending".into()],
        known_limits: vec!["Observed on one task".into()],
        unverified_assumptions: Vec::new(),
    }
}

#[test]
fn evidence_preserves_claims_and_rejects_unbounded_or_invented_capability_evidence() {
    let evidence = record();
    assert!(evidence.validate().is_ok());
    for (field, invalid) in [
        ("schema_version", json!(2)),
        ("timestamp", json!(0)),
        ("source_digest", json!("bad")),
        ("scope", json!("")),
        ("claims", json!([])),
        ("claims", json!(vec!["claim"; 129])),
        ("known_limits", json!(vec!["limit"; 129])),
        ("unverified_assumptions", json!(vec!["assumption"; 129])),
        ("rule_ids", json!(vec!["rule"; 257])),
        ("rule_ids", json!(["same", "same"])),
        ("rule_ids", json!([""])),
        ("claims", json!(["x".repeat(4097)])),
        ("claims", json!(["invalid\0control"])),
        ("kind", json!("capability_gap")),
    ] {
        let mut value = serde_json::to_value(&evidence).unwrap();
        value[field] = invalid;
        assert!(
            serde_json::from_value::<EvidenceRecord>(value)
                .unwrap()
                .validate()
                .is_err(),
            "{field}"
        );
    }
    let mut gap = evidence;
    gap.kind = EvidenceKind::CapabilityGap;
    gap.rule_ids.clear();
    assert!(gap.validate().is_ok());
    assert!(validate_text("line one\nline two\ttext", 100).is_ok());
    for value in ["sha256:", "md5:abc", &format!("sha256:{}", "A".repeat(64))] {
        assert!(!valid_digest(value));
    }
}

#[test]
fn candidate_references_are_bounded_and_published_states_are_frozen() {
    let hash = format!("sha256:{}", "a".repeat(64));
    let mut revision = PolicyRevision {
        schema_version: 1,
        parent_policy_digest: hash.clone(),
        policy_digest: hash.clone(),
        previous_revision: None,
        patch: Vec::new(),
        status: RevisionStatus::Candidate,
        created_by: record().actor,
        reason: "Improve repeated failures".into(),
        evidence_refs: vec![hash.clone()],
        created_at: 1,
        evaluation_ref: None,
        approval_ref: None,
    };
    assert!(revision.validate().is_ok());
    assert!(revision.editable());
    for status in [
        RevisionStatus::Validating,
        RevisionStatus::Approved,
        RevisionStatus::Active,
        RevisionStatus::Rejected,
        RevisionStatus::Deprecated,
        RevisionStatus::Retired,
        RevisionStatus::Revoked,
    ] {
        revision.status = status;
        assert!(!revision.editable());
    }
    revision.status = RevisionStatus::Draft;
    assert!(revision.editable());
    for (field, invalid) in [
        ("schema_version", json!(2)),
        ("created_at", json!(0)),
        ("reason", json!(" ")),
        ("parent_policy_digest", json!("bad")),
        ("policy_digest", json!("bad")),
        ("previous_revision", json!("bad")),
        ("evaluation_ref", json!("bad")),
        ("approval_ref", json!("bad")),
        ("evidence_refs", json!([])),
        ("evidence_refs", json!(vec![hash.clone(); 129])),
        ("evidence_refs", json!(["bad"])),
        ("evidence_refs", json!([hash.clone(), hash.clone()])),
        ("patch", json!(vec![json!({}); 257])),
        ("created_by", json!({"id":"","kind":"agent"})),
    ] {
        let mut value = serde_json::to_value(&revision).unwrap();
        value[field] = invalid;
        assert!(
            serde_json::from_value::<PolicyRevision>(value)
                .unwrap()
                .validate()
                .is_err(),
            "{field}"
        );
    }
}
