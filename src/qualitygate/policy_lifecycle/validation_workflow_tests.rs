use super::*;
use crate::domain::evolution::{ActorKind, EvidenceKind, EvidenceRecord, Sensitivity};
use serde_json::{Value, json};

pub(crate) async fn fixture() -> (tempfile::TempDir, tempfile::TempDir, ValidateOptions) {
    let policy = "schema_version: 1\nrules: {line-ending: {enabled: false}}\n";
    let root = crate::application::test_support::repository(policy).await;
    let external = crate::application::test_support::tempdir();
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
        assert_eq!(
            snapshot::run_git(root.path(), &["show", &format!("HEAD:{name}.txt")], None)
                .await
                .unwrap(),
            content.as_bytes(),
            "The fixture commit must retain its declared acceptance bytes"
        );
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
        budget: crate::selfcheck::policy_oracles::budget(),
        min_improvements: 1,
        max_rules: 32,
        max_rule_growth: 4,
        cases,
    };
    let suite_bytes = serde_json::to_vec(&suite).unwrap();
    let trust = crate::selfcheck::policy_oracles::trust(
        root.path(),
        digest(&suite_bytes),
        baseline.clone(),
        crate::application::evaluator_digest().await.unwrap(),
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

#[tokio::test]
async fn paired_validation_publishes_bound_reports_without_approving_candidate() {
    let (root, _external, options) = fixture().await;
    let id = options.candidate_id.clone();
    let (result, code) = validate(options).await.unwrap();
    assert_eq!(code, 0, "{result:#}");
    assert_eq!(result["evaluation"]["conclusion"], "pass");
    let store = Store::open(root.path()).unwrap();
    let (_, revision) = policy_candidates::candidate(&store, &id).unwrap();
    assert_eq!(
        revision.status,
        crate::domain::evolution::RevisionStatus::Candidate
    );
    assert!(store.index.active_policy.is_none());
    let evaluated = config::policy_promotion::complete_evaluation(&store, &id, &revision).unwrap();
    assert_eq!(evaluated.cases.len(), 3);
    for pair in &evaluated.cases {
        for observation in [&pair.baseline, &pair.candidate] {
            let report: Report = store
                .record(observation.report_ref.as_ref().unwrap(), "check_report")
                .unwrap();
            assert!(report.gate.complete);
            assert_eq!(report.scope, "task");
            assert_eq!(report.policy.trust, "protected_paired_evaluation");
        }
    }
    assert!(revision.approval_ref.is_none());
}

#[tokio::test]
async fn preparation_rejects_changed_parent_unauthorized_evaluator_and_resource_budget() {
    let (root, external, mut options) = fixture().await;
    let evaluator = crate::application::evaluator_digest().await.unwrap();
    assert!(prepare(&options, &digest(b"unauthorized runtime")).is_err());
    let original = options.baseline.clone();
    options.baseline = digest(b"other parent");
    assert!(prepare(&options, &evaluator).is_err());
    options.baseline = original;
    let prepared = prepare(&options, &evaluator).unwrap();
    let capture = policy_capture(&snapshot::CaptureOptions::default(), &prepared.candidate);
    assert!(
        capture
            .protected_paths
            .iter()
            .any(|p| p == "qualitygate.yaml")
    );
    let loaded = prepare_check(
        &prepared.candidate,
        &prepared.inputs.suite.cases[0].task,
        "policy-ref",
    )
    .unwrap();
    assert_eq!(loaded.evidence.source, "policy-ref");
    assert_eq!(
        loaded.evidence.task_contract_source.as_deref(),
        Some("replay")
    );
    options.jobs = Some(0);
    let before = std::fs::read(root.path().join(".qualitygate/policy/HEAD")).unwrap();
    assert!(validate(options).await.is_err());
    assert_eq!(
        std::fs::read(root.path().join(".qualitygate/policy/HEAD")).unwrap(),
        before
    );
    assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn validation_replay_preserves_exact_authority_and_optional_parallelism() {
    let (_root, _external, mut options) = fixture().await;
    let argv = validation_recheck(&options);
    for pair in [
        ["--baseline", options.baseline.as_str()],
        ["--actor-kind", "human"],
        ["--jobs", "2"],
    ] {
        assert!(argv.windows(2).any(|args| args == pair));
    }
    options.jobs = None;
    options.actor.kind = ActorKind::Agent;
    let argv = validation_recheck(&options);
    assert!(!argv.iter().any(|arg| arg == "--jobs"));
    assert!(
        argv.windows(2)
            .any(|args| args == ["--actor-kind", "agent"])
    );
    let identity: Value = evaluator().await.unwrap();
    assert!(
        identity["evaluator_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        identity["environment_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn protected_evaluation_distinguishes_missing_history_timeout_and_rule_budget_failure() {
    for scenario in ["missing-history", "case-timeout", "rule-budget"] {
        let (root, _external, mut options) = fixture().await;
        let mut suite: config::policy_acceptance::ValidationSuite =
            serde_json::from_slice(&std::fs::read(&options.task).unwrap()).unwrap();
        match scenario {
            "missing-history" => suite.cases[0].head = "0".repeat(40),
            "case-timeout" => {
                suite.budget.case_timeout_seconds = 1;
                for case in &mut suite.cases {
                    let mut task = serde_json::to_value(&case.task).unwrap();
                    task["acceptance"][0]["verification"]["argv"] = json!(["sh", "-c", "sleep 5"]);
                    case.task = serde_json::from_value(task).unwrap();
                }
            }
            _ => {
                policy_candidates::mutate(
                    root.path(),
                    &options.candidate_id,
                    Actor {
                        id: "author".into(),
                        kind: ActorKind::Agent,
                    },
                    config::rule_management::Mutation::Enable("diff-size".into()),
                )
                .unwrap();
                suite.max_rules = 1;
                suite.max_rule_growth = 1;
            }
        }
        let bytes = serde_json::to_vec(&suite).unwrap();
        let mut trust: config::policy_acceptance::EvolutionTrust =
            serde_json::from_slice(&std::fs::read(&options.trust_store).unwrap()).unwrap();
        trust.suites = vec![digest(&bytes)];
        std::fs::write(&options.task, bytes).unwrap();
        std::fs::write(&options.trust_store, serde_json::to_vec(&trust).unwrap()).unwrap();
        options.jobs = Some(1);
        let (result, code) = validate(options).await.unwrap();
        assert_eq!(
            code,
            if scenario == "rule-budget" { 1 } else { 2 },
            "{scenario}: {result:#}"
        );
        assert_eq!(
            result["evaluation"]["conclusion"],
            if scenario == "rule-budget" {
                "block"
            } else {
                "incomplete"
            }
        );
        let text = result.to_string();
        assert!(
            text.contains(match scenario {
                "case-timeout" => "paired case timed out",
                "rule-budget" => "contribution budget",
                _ => "incomplete",
            }),
            "{text}"
        );
        assert!(
            Store::open(root.path())
                .unwrap()
                .index
                .active_policy
                .is_none()
        );
    }
}

#[tokio::test]
async fn candidate_acquisition_pins_git_parent_and_reuses_only_published_policy_versions() {
    let (root, _external, options) = fixture().await;
    let store = Store::open(root.path()).unwrap();
    let (_, revision) = policy_candidates::candidate(&store, &options.candidate_id).unwrap();
    let proposal = || policy_candidates::Proposal {
        actor: Actor {
            id: "second-author".into(),
            kind: ActorKind::Human,
        },
        reason: "Review retained policy inputs".into(),
        evidence: revision.evidence_refs.clone(),
    };
    let head = snapshot::resolve_commit(root.path(), "HEAD").await.unwrap();
    // Dirty configuration is not admitted into a proposal whose parent is a Git commit.
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        b"schema_version: 999\n",
    )
    .unwrap();
    let created = crate::policy_lifecycle::candidate_acquisition::create(
        root.path().into(),
        "qualitygate.yaml".into(),
        "HEAD".into(),
        proposal(),
    )
    .await
    .unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap();
    let archived = policy_candidates::show(root.path(), "policy", parent).unwrap();
    assert_eq!(archived["config"]["schema_version"], 1);
    assert_eq!(archived["version"]["source_commit"], head);
    let reused = crate::policy_lifecycle::candidate_acquisition::create(
        root.path().into(),
        "qualitygate.yaml".into(),
        parent.into(),
        proposal(),
    )
    .await
    .unwrap();
    assert_eq!(reused["revision"]["parent_policy_digest"], parent);
}

#[tokio::test]
async fn different_acquisition_policies_keep_separate_snapshots_and_bind_both_comparisons() {
    let (_root, external, options) = fixture().await;
    let mut prepared = prepare(
        &options,
        &crate::application::evaluator_digest().await.unwrap(),
    )
    .unwrap();
    Arc::get_mut(&mut prepared.candidate)
        .unwrap()
        .config
        .exclude = vec!["replay.txt".into()];
    let case = &prepared.inputs.suite.cases[0];
    let (observed, (baseline, candidate)) = run_case(
        &options,
        &prepared,
        case,
        external.path(),
        &Arc::new(Semaphore::new(1)),
    )
    .await
    .unwrap();
    assert!(baseline.gate.complete && candidate.gate.complete);
    assert_ne!(
        baseline.snapshot.content_digest,
        candidate.snapshot.content_digest
    );
    assert_eq!(baseline.snapshot.base, candidate.snapshot.base);
    assert_eq!(baseline.snapshot.head, candidate.snapshot.head);
    assert_eq!(
        observed.snapshot_digest,
        Some(digest(
            &serde_json::to_vec(&(&baseline.snapshot, &candidate.snapshot)).unwrap()
        ))
    );
    assert_eq!(
        baseline.policy.task_contract_digest,
        candidate.policy.task_contract_digest
    );
}
