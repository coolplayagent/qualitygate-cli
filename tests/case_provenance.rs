mod common;
#[path = "common/evolution.rs"]
mod evolution;
use evolution::*;
use qualitygate::{
    config::{
        policy_candidates,
        policy_store::{Store, digest},
    },
    domain::{
        case_provenance::{CaseOrigin, CaseProvenance, Classification},
        evolution::{Actor, ActorKind, EvidenceRecord},
    },
};

fn record(f: &Fixture, id: &str, classification: Classification) -> (EvidenceRecord, Vec<u8>) {
    let store = Store::open(f.root.path()).unwrap();
    let mut record = policy_candidates::evidence(&store, &f.suite.motivating_evidence[0]).unwrap();
    let bytes = format!("Retained fixture source {id}").into_bytes();
    record.source_digest = digest(&bytes);
    record.case = Some(CaseProvenance {
        case_id: id.into(),
        origin: if classification == Classification::Generated {
            CaseOrigin::Generated
        } else {
            CaseOrigin::Observed
        },
        classification,
        reviewer: Some(Actor {
            id: "independent-reviewer".into(),
            kind: ActorKind::Human,
        }),
        used_for_tuning: false,
        derived_from: vec![],
        expected_detection: "CRLF defect is detected".into(),
        expected_allowed: "LF text is allowed".into(),
    });
    (record, bytes)
}
fn retain(f: &Fixture, record: EvidenceRecord, source: &[u8]) -> String {
    policy_candidates::retain_evidence(f.root.path(), record, source).unwrap()["evidence_ref"]
        .as_str()
        .unwrap()
        .into()
}
fn v2(f: &mut Fixture) {
    f.suite.schema_version = 2;
    for i in 0..3 {
        let (record, source) = record(
            f,
            &format!("case-{i}"),
            if i == 0 {
                Classification::Generated
            } else {
                Classification::Independent
            },
        );
        f.suite.cases[i].evidence_ref = Some(retain(f, record, &source));
    }
    f.write_inputs();
}

#[test]
fn generated_and_tuned_sources_cannot_be_relabelled_or_hidden_in_ancestry() {
    let f = Fixture::new();
    let (mut generated, bytes) = record(&f, "generated", Classification::Generated);
    let reference = retain(&f, generated.clone(), &bytes);
    let case = generated.case.as_mut().unwrap();
    case.classification = Classification::Independent;
    case.origin = CaseOrigin::Observed;
    assert!(
        policy_candidates::retain_evidence(f.root.path(), generated, &bytes)
            .unwrap_err()
            .to_string()
            .contains("relabeled")
    );
    let (mut child, source) = record(&f, "child", Classification::HumanConfirmed);
    child.case.as_mut().unwrap().derived_from = vec![reference];
    let child_ref = retain(&f, child, &source);
    let (mut independent, source) = record(&f, "unrelated-source", Classification::Independent);
    independent.case.as_mut().unwrap().derived_from = vec![child_ref];
    assert!(
        policy_candidates::retain_evidence(f.root.path(), independent.clone(), &source)
            .unwrap_err()
            .to_string()
            .contains("descends")
    );
    independent.case.as_mut().unwrap().derived_from = vec![f.suite.motivating_evidence[0].clone()];
    assert!(
        policy_candidates::retain_evidence(f.root.path(), independent.clone(), &source)
            .unwrap_err()
            .to_string()
            .contains("provenance")
    );
    independent.case.as_mut().unwrap().derived_from = vec![format!("sha256:{}", "f".repeat(64))];
    assert!(policy_candidates::retain_evidence(f.root.path(), independent, &source).is_err());
    let (mut tuned, source) = record(&f, "tuned", Classification::HumanConfirmed);
    tuned.case.as_mut().unwrap().used_for_tuning = true;
    retain(&f, tuned.clone(), &source);
    tuned.case.as_mut().unwrap().used_for_tuning = false;
    tuned.case.as_mut().unwrap().classification = Classification::Independent;
    assert!(policy_candidates::retain_evidence(f.root.path(), tuned, &source).is_err());
}

#[test]
fn protected_v2_rejects_missing_reused_self_reviewed_and_motivating_cases() {
    let mut f = Fixture::new();
    v2(&mut f);
    f.enable();
    let good = f.suite.clone();
    f.suite.cases[1].evidence_ref = None;
    f.write_inputs();
    f.validate("1", 2);
    f.suite = good.clone();
    f.suite.cases[1].evidence_ref = f.suite.cases[0].evidence_ref.clone();
    f.write_inputs();
    f.validate("1", 2);
    f.suite = good.clone();
    let (mut self_review, source) = record(&f, "self-reviewed", Classification::Independent);
    self_review
        .case
        .as_mut()
        .unwrap()
        .reviewer
        .as_mut()
        .unwrap()
        .id = "generator".into();
    f.suite.cases[1].evidence_ref = Some(retain(&f, self_review, &source));
    f.write_inputs();
    f.validate("1", 2);
    f.suite = good;
    // Motivation cannot be an independent holdout even with a different record ID.
    let store = Store::open(f.root.path()).unwrap();
    let mut motivation =
        policy_candidates::evidence(&store, &f.suite.motivating_evidence[0]).unwrap();
    motivation.case = record(&f, "motivation-as-holdout", Classification::Independent)
        .0
        .case;
    let source = store.blob(&motivation.source_digest).unwrap();
    f.suite.cases[1].evidence_ref = Some(retain(&f, motivation, &source));
    f.write_inputs();
    f.validate("1", 2);
}

#[test]
fn independent_normal_oracle_blocks_overfitting_even_when_generated_replay_passes() {
    let mut f = Fixture::new();
    v2(&mut f);
    f.enable();
    // The held-out contract explicitly permits CRLF for this file. The generated
    // replay is still satisfied by enabling the broad LF rule, but this is not.
    let base = evolution::head(f.root.path());
    std::fs::write(f.root.path().join("permitted-normal.txt"), "permitted\r\n").unwrap();
    common::git(f.root.path(), &["add", "."]);
    common::git(
        f.root.path(),
        &["commit", "-qm", "independent normal example"],
    );
    f.suite.cases[1].base = base;
    f.suite.cases[1].head = evolution::head(f.root.path());
    f.write_inputs();
    let result = f.validate("2", 1);
    assert_eq!(result["evaluation"]["conclusion"], "block");
    assert!(
        result["evaluation"]["cases"][1]["candidate"]["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m.as_str().unwrap().contains("expected Pass, observed Fail"))
    );
}

#[test]
fn fewer_alerts_still_fail_an_independent_known_defect_oracle() {
    let mut f = Fixture::new();
    v2(&mut f);
    // The unchanged disabled rule emits fewer alerts and misses the known replay.
    f.suite.min_improvements = 0;
    f.write_inputs();
    let result = f.validate("1", 1);
    assert_eq!(result["evaluation"]["conclusion"], "block");
    let replay = result["evaluation"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "replay")
        .unwrap();
    assert!(
        replay["candidate"]["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m.as_str().unwrap().contains("expected Fail, observed"))
    );
}

#[test]
fn successful_v2_validation_is_invalidated_if_a_case_is_later_used_for_tuning() {
    let mut f = Fixture::new();
    v2(&mut f);
    f.enable();
    let result = f.validate("2", 0);
    assert_eq!(result["evaluation"]["conclusion"], "pass");
    let trust = f.external.path().join("trust.json");
    let args = [
        "policy",
        "candidate",
        "approval-subject",
        &f.id,
        "--trust-store",
        trust.to_str().unwrap(),
    ];
    run(f.root.path(), &args, 0);
    let store = Store::open(f.root.path()).unwrap();
    let mut record =
        policy_candidates::evidence(&store, f.suite.cases[1].evidence_ref.as_deref().unwrap())
            .unwrap();
    let source = store.blob(&record.source_digest).unwrap();
    let case = record.case.as_mut().unwrap();
    case.classification = Classification::HumanConfirmed;
    case.used_for_tuning = true;
    retain(&f, record, &source);
    run(f.root.path(), &args, 2);
}
