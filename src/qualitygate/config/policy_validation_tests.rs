use super::*;
use crate::domain::{
    VerificationBoundary, evolution::ActorKind, policy_evaluation::EvaluationBudget,
};

fn fixture(root: &Path) -> Attempt {
    let created = super::super::policy_candidates::tests::candidate_fixture(root);
    Attempt {
        candidate_id: created["id"].as_str().unwrap().into(),
        input_revision: created["revision_ref"].as_str().unwrap().into(),
        candidate_policy: created["revision"]["policy_digest"]
            .as_str()
            .unwrap()
            .into(),
        baseline_policy: created["revision"]["parent_policy_digest"]
            .as_str()
            .unwrap()
            .into(),
        suite_digest: super::super::policy_store::digest(b"suite"),
        trust_digest: super::super::policy_store::digest(b"trust"),
        evaluator_digest: super::super::policy_store::digest(b"runtime"),
        actor: Actor {
            id: "validator".into(),
            kind: ActorKind::Human,
        },
        started_at: 1,
    }
}

fn run(attempt: &Attempt, conclusion: Conclusion) -> EvaluationRun {
    EvaluationRun {
        schema_version: 1,
        candidate_id: attempt.candidate_id.clone(),
        candidate_revision: attempt.input_revision.clone(),
        candidate_policy: attempt.candidate_policy.clone(),
        baseline_policy: attempt.baseline_policy.clone(),
        suite_digest: attempt.suite_digest.clone(),
        trust_digest: attempt.trust_digest.clone(),
        evaluator_epoch: "epoch-1".into(),
        evaluator_digest: attempt.evaluator_digest.clone(),
        environment_digest: super::super::policy_store::digest(b"environment"),
        budget: EvaluationBudget {
            snapshot_max_mib: 16,
            snapshot_max_file_mib: 2,
            snapshot_jobs: 1,
            snapshot_timeout_seconds: 10,
            max_live_snapshot_mib: 32,
            max_parallel: 1,
            case_timeout_seconds: 10,
            total_timeout_seconds: 30,
        },
        jobs: 1,
        generation_actor: Actor {
            id: "author".into(),
            kind: ActorKind::Human,
        },
        evaluation_actor: attempt.actor.clone(),
        started_at: attempt.started_at,
        ended_at: 2,
        cases: vec![],
        conclusion,
        reasons: vec!["unit observation".into()],
        verification: VerificationBoundary::default(),
    }
}

#[test]
fn immutable_attempt_rejects_each_changed_identity_and_reversed_time() {
    let root = tempfile::tempdir().unwrap();
    let attempt = fixture(root.path());
    let source = run(&attempt, Conclusion::Pass);
    binding(&attempt, &source).unwrap();
    for key in [
        "candidate_id",
        "candidate_revision",
        "candidate_policy",
        "baseline_policy",
        "suite_digest",
        "trust_digest",
        "evaluator_digest",
    ] {
        let mut value = serde_json::to_value(&source).unwrap();
        value[key] = json!("different");
        let changed = serde_json::from_value(value).unwrap();
        assert!(binding(&attempt, &changed).is_err(), "{key}");
    }
    let mut changed = source.clone();
    changed.evaluation_actor.id = "other".into();
    assert!(binding(&attempt, &changed).is_err());
    changed = source.clone();
    changed.started_at = 2;
    assert!(binding(&attempt, &changed).is_err());
    changed = source;
    changed.ended_at = 0;
    assert!(binding(&attempt, &changed).is_err());
}

#[test]
fn validation_freezes_revision_and_publishes_distinct_pass_block_and_incomplete_states() {
    for (conclusion, status) in [
        (Conclusion::Pass, "candidate"),
        (Conclusion::Block, "rejected"),
        (Conclusion::Incomplete, "candidate"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let attempt = fixture(root.path());
        let frozen = begin(root.path(), &attempt, b"suite", b"trust").unwrap();
        let state =
            policy_candidates::show(root.path(), "candidate", &attempt.candidate_id).unwrap();
        assert_eq!(state["revision"]["status"], "validating");
        assert_eq!(
            state["revision"]["previous_revision"],
            attempt.input_revision
        );
        assert!(
            policy_candidates::mutate(
                root.path(),
                &attempt.candidate_id,
                attempt.actor.clone(),
                super::super::rule_management::Mutation::Disable("line-ending".into())
            )
            .is_err()
        );
        let mut evaluation = run(&attempt, conclusion);
        let published = finish(root.path(), &frozen, &mut evaluation).unwrap();
        let current =
            policy_candidates::show(root.path(), "candidate", &attempt.candidate_id).unwrap();
        assert_eq!(current["revision"]["status"], status);
        assert_eq!(current["revision"]["previous_revision"], frozen);
        assert_eq!(
            current["revision"]["evaluation_ref"],
            published["evaluation_ref"]
        );
        assert!(current["revision"]["approval_ref"].is_null());
        let retained =
            self::evaluation(root.path(), published["evaluation_ref"].as_str().unwrap()).unwrap();
        assert_eq!(retained["evaluation"]["conclusion"], json!(conclusion));
        assert!(finish(root.path(), &frozen, &mut evaluation).is_err());
        // Storage completion is not approval: promotion must still replay its independent oracle.
        let store = Store::open(root.path()).unwrap();
        let (_, candidate) = policy_candidates::candidate(&store, &attempt.candidate_id).unwrap();
        assert!(
            super::super::policy_promotion::complete_evaluation(
                &store,
                &attempt.candidate_id,
                &candidate
            )
            .is_err()
        );
    }
}

#[test]
fn mismatched_bytes_stale_revision_and_cancelled_validation_cannot_publish_success() {
    let root = tempfile::tempdir().unwrap();
    let attempt = fixture(root.path());
    for (suite, trust) in [
        (b"wrong".as_slice(), b"trust".as_slice()),
        (b"suite".as_slice(), b"wrong".as_slice()),
    ] {
        assert!(
            begin(root.path(), &attempt, suite, trust)
                .unwrap_err()
                .to_string()
                .contains("digests differ")
        );
        assert_eq!(
            policy_candidates::show(root.path(), "candidate", &attempt.candidate_id).unwrap()["revision_ref"],
            attempt.input_revision
        );
    }
    let mut stale = attempt.clone();
    stale.input_revision = "old".into();
    assert!(
        begin(root.path(), &stale, b"suite", b"trust")
            .unwrap_err()
            .to_string()
            .contains("Candidate changed")
    );
    let frozen = begin(root.path(), &attempt, b"suite", b"trust").unwrap();
    let cancelled = abandon(
        root.path(),
        &attempt.candidate_id,
        attempt.actor.clone(),
        "operator cancelled",
    )
    .unwrap();
    assert_eq!(cancelled["revision"]["status"], "abandoned");
    assert!(cancelled["revision"]["evaluation_ref"].is_string());
    assert!(
        finish(root.path(), &frozen, &mut run(&attempt, Conclusion::Pass))
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    assert!(self::evaluation(root.path(), "unpublished").is_err());
    assert!(abandon(root.path(), &attempt.candidate_id, attempt.actor, "again").is_err());
}

#[test]
fn experience_pages_preserve_matched_groups_raw_records_and_unverified_evidence() {
    use super::super::policy_effectiveness as experience;
    let root = tempfile::tempdir().unwrap();
    let mut attempt = fixture(root.path());
    let mut retained = Vec::new();
    for index in 0..3 {
        let frozen = begin(root.path(), &attempt, b"suite", b"trust").unwrap();
        let mut evaluation = run(&attempt, Conclusion::Incomplete);
        if index == 2 {
            evaluation.environment_digest =
                super::super::policy_store::digest(b"different environment");
        }
        let published = finish(root.path(), &frozen, &mut evaluation).unwrap();
        retained.push(published["evaluation_ref"].as_str().unwrap().to_string());
        let store = Store::open(root.path()).unwrap();
        attempt.input_revision = policy_candidates::candidate(&store, &attempt.candidate_id)
            .unwrap()
            .0;
    }
    let page = experience::report(root.path(), Some(&attempt.baseline_policy), 0, 2).unwrap();
    assert_eq!(page["scanned"], 2);
    assert_eq!(page["next_offset"], 2);
    assert_eq!(page["groups"].as_array().unwrap().len(), 2);
    let all = experience::report(root.path(), None, 0, 10).unwrap();
    assert_eq!(
        all["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["observations"].as_array().unwrap().len())
            .sum::<usize>(),
        3
    );
    assert!(all["next_offset"].is_null());
    assert!(experience::report(root.path(), None, 0, 0).is_err());
    assert!(experience::report(root.path(), None, 0, 257).is_err());
    assert!(
        experience::report(
            root.path(),
            Some(&super::super::policy_store::digest(b"unknown")),
            0,
            10
        )
        .is_err()
    );
    let history = experience::rule_history(root.path(), "line-ending", 0, 10).unwrap();
    assert_eq!(
        history["evidence"][0]["trust"],
        "unverified_source_evidence"
    );
    assert_eq!(history["candidates"].as_array().unwrap().len(), 1);
    assert!(experience::rule_history(root.path(), "line-ending", 0, 0).is_err());
    assert!(
        experience::rule_history(root.path(), "unknown", 0, 10).unwrap()["candidates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let store = Store::open(root.path()).unwrap();
    let (reference, revision) =
        policy_candidates::candidate(&store, &attempt.candidate_id).unwrap();
    let raw = experience::record(root.path(), &reference, "candidate").unwrap();
    assert_eq!(raw["record"]["policy_digest"], revision.policy_digest);
    let blob = experience::record(root.path(), &attempt.suite_digest, "blob").unwrap();
    assert_eq!(blob["encoding"], "base64");
    assert_eq!(blob["data"], "c3VpdGU=");
    assert!(experience::record(root.path(), &reference, "unsupported").is_err());
    assert!(experience::record(root.path(), &reference, "check_report").is_err());
}
