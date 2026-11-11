use super::*;
use crate::config::policy_store::digest;
use crate::domain::{
    case_provenance::{CaseOrigin, CaseProvenance},
    evolution::{Actor, ActorKind, EvidenceKind, Sensitivity},
};

fn record(id: &str, source: &[u8], independent: bool, parents: Vec<String>) -> EvidenceRecord {
    EvidenceRecord {
        schema_version: 1,
        kind: EvidenceKind::Review,
        source_digest: digest(source),
        scope: "scenario".into(),
        timestamp: 1,
        actor: Actor {
            id: "reviewer".into(),
            kind: ActorKind::Human,
        },
        claims: vec!["Observed behavior".into()],
        sensitivity: Sensitivity::Public,
        rule_ids: vec!["line-ending".into()],
        known_limits: vec!["Declared provenance is not identity authentication".into()],
        unverified_assumptions: vec![],
        case: Some(CaseProvenance {
            case_id: id.into(),
            origin: if independent {
                CaseOrigin::Observed
            } else {
                CaseOrigin::Generated
            },
            classification: if independent {
                Classification::Independent
            } else {
                Classification::Generated
            },
            reviewer: independent.then(|| Actor {
                id: "reviewer".into(),
                kind: ActorKind::Human,
            }),
            used_for_tuning: false,
            derived_from: parents,
            expected_detection: "CRLF rejected".into(),
            expected_allowed: "LF accepted".into(),
        }),
    }
}

#[test]
fn independence_cannot_be_reclaimed_by_omitting_generated_or_tuned_ancestry() {
    let root = tempfile::tempdir().unwrap();
    let generated = policy_candidates::retain_evidence(
        root.path(),
        record("generated", b"source", false, vec![]),
        b"source",
    )
    .unwrap();
    let reference = generated["evidence_ref"].as_str().unwrap().to_owned();
    let store = Store::open(root.path()).unwrap();
    for proposed in [
        record("derived", b"derived", true, vec![reference.clone()]),
        record("generated", b"different", true, vec![]),
        record("new-name", b"source", true, vec![]),
    ] {
        assert!(validate_record(&store, &proposed).is_err());
    }
    let child = record("generated-child", b"child", false, vec![reference]);
    policy_candidates::retain_evidence(root.path(), child, b"child").unwrap();
    let clean = record("independent", b"independent source", true, vec![]);
    policy_candidates::retain_evidence(root.path(), clean, b"independent source").unwrap();
    let mut no_provenance = record("legacy", b"legacy", false, vec![]);
    no_provenance.case = None;
    let legacy = policy_candidates::retain_evidence(root.path(), no_provenance, b"legacy").unwrap()
        ["evidence_ref"]
        .as_str()
        .unwrap()
        .to_owned();
    let store = Store::open(root.path()).unwrap();
    assert!(
        validate_record(
            &store,
            &record("unknown ancestry", b"other", false, vec![legacy])
        )
        .unwrap_err()
        .to_string()
        .contains("must retain provenance")
    );
    assert!(
        validate_record(
            &store,
            &record("missing", b"missing", false, vec![digest(b"unretained")])
        )
        .is_err()
    );
}

#[test]
fn version_two_suite_requires_distinct_independent_heldout_sources_and_reviewers() {
    let root = tempfile::tempdir().unwrap();
    let created = crate::config::policy_candidates::tests::candidate_fixture(root.path());
    let mut references = vec![];
    for (name, independent) in [("replay", false), ("heldout", true), ("anchor", true)] {
        references.push(
            policy_candidates::retain_evidence(
                root.path(),
                record(name, name.as_bytes(), independent, vec![]),
                name.as_bytes(),
            )
            .unwrap()["evidence_ref"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let store = Store::open(root.path()).unwrap();
    let (_, mut revision) =
        policy_candidates::candidate(&store, created["id"].as_str().unwrap()).unwrap();
    let task: crate::config::TaskContract = serde_json::from_value(serde_json::json!({"schema_version":1,"task_id":"task","acceptance":[{"id":"probe","description":"native probe","verification":{"check_id":"probe","argv":["git","--version"]}}]})).unwrap();
    let cases = [CaseKind::Replay, CaseKind::HeldOut, CaseKind::Anchor]
        .into_iter()
        .enumerate()
        .map(
            |(index, kind)| crate::config::policy_acceptance::ValidationCase {
                id: format!("case-{index}"),
                evidence_ref: Some(references[index].clone()),
                kind,
                base: "a".repeat(40),
                head: "b".repeat(40),
                task: task.clone(),
                expectations: Default::default(),
            },
        )
        .collect();
    let mut suite: ValidationSuite = serde_json::from_value(serde_json::json!({"schema_version":2,"id":"suite","baseline_policy":revision.parent_policy_digest,
        "evaluator_epoch":"epoch","motivating_evidence":revision.evidence_refs,"budget":{"snapshot_max_mib":16,"snapshot_max_file_mib":2,"snapshot_jobs":1,"snapshot_timeout_seconds":10,"max_live_snapshot_mib":32,"max_parallel":1,"case_timeout_seconds":10,"total_timeout_seconds":30},
        "min_improvements":1,"max_rules":32,"max_rule_growth":4,"cases":[]})).unwrap();
    suite.cases = cases;
    validate_suite(&store, &suite, &revision).unwrap();
    for case in 0..4 {
        let mut changed = suite.clone();
        match case {
            0 => changed.cases[1].evidence_ref = None,
            1 => changed.cases[1].evidence_ref = Some(references[0].clone()),
            2 => changed.cases[2].evidence_ref = Some(references[1].clone()),
            3 => changed.cases[1].evidence_ref = Some(revision.evidence_refs[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            validate_suite(&store, &changed, &revision).is_err(),
            "case {case}"
        );
    }
    revision
        .patch
        .push(serde_json::json!({"actor":{"id":"reviewer"}}));
    assert!(
        validate_suite(&store, &suite, &revision)
            .unwrap_err()
            .to_string()
            .contains("outside candidate authors")
    );
    revision.patch.pop();
    revision.evidence_refs.push(references[1].clone());
    assert!(
        validate_suite(&store, &suite, &revision)
            .unwrap_err()
            .to_string()
            .contains("cannot double")
    );
}
