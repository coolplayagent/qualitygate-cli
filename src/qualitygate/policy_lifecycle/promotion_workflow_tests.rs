use super::*;
use crate::domain::evolution::{Actor, ActorKind};
use serde_json::{Value, json};

fn signed(root: &std::path::Path, request: &Value, decision: &str) -> PathBuf {
    let now = now().unwrap();
    let record = json!({"schema_version":1,"subject":request["subject"],
        "approver":{"id":"fixture-reviewer","kind":"human"},"decision":decision,
        "issued_at":now,"expires_at":now+600,"reason":"Reviewed independently against retained unit evidence"});
    let bytes = crate::selfcheck::policy_oracles::envelope(
        request["payload_type"].as_str().unwrap(),
        &record,
        false,
        false,
    )
    .unwrap();
    let path = root.join("approval.json");
    std::fs::write(&path, bytes).unwrap();
    path
}

#[tokio::test]
async fn unsigned_or_rejected_candidate_cannot_become_active() {
    let (root, external, options) =
        crate::policy_lifecycle::validation_workflow::tests::fixture().await;
    let id = options.candidate_id.clone();
    let trust = options.trust_store.clone();
    assert!(promote(root.path().into(), id.clone()).await.is_err());
    assert!(
        subject(root.path().into(), id.clone(), trust.clone())
            .await
            .is_err()
    );
    let (_, code) = crate::policy_lifecycle::validation_workflow::validate(options)
        .await
        .unwrap();
    assert_eq!(code, 0);
    let request = subject(root.path().into(), id.clone(), trust.clone())
        .await
        .unwrap();
    let envelope = signed(external.path(), &request, "rejected");
    let (result, code) = approve(root.path().into(), id.clone(), envelope, trust)
        .await
        .unwrap();
    assert_eq!(code, 1);
    assert_eq!(result["revision"]["status"], "rejected");
    assert!(promote(root.path().into(), id).await.is_err());
    assert!(
        Store::open(root.path())
            .unwrap()
            .index
            .active_policy
            .is_none()
    );
}

#[tokio::test]
async fn active_policy_requires_live_trust_and_rollback_authorizes_exact_current_state() {
    let (root, external, options) =
        crate::policy_lifecycle::validation_workflow::tests::fixture().await;
    let id = options.candidate_id.clone();
    let baseline = options.baseline.clone();
    let trust_path = options.trust_store.clone();
    let (evaluation, code) = crate::policy_lifecycle::validation_workflow::validate(options)
        .await
        .unwrap();
    assert_eq!(code, 0);
    let request = subject(root.path().into(), id.clone(), trust_path.clone())
        .await
        .unwrap();
    let envelope = signed(external.path(), &request, "approved");
    assert_eq!(
        approve(root.path().into(), id.clone(), envelope, trust_path.clone())
            .await
            .unwrap()
            .1,
        0
    );
    // Promotion consumes archived reports, not the disposable evaluator workspace.
    std::fs::remove_dir_all(evaluation["evidence_directory"].as_str().unwrap()).unwrap();
    let promoted = promote(root.path().into(), id.clone()).await.unwrap();
    assert_eq!(
        promoted["active_policy"],
        evaluation["evaluation"]["candidate_policy"]
    );
    let active = crate::policy_lifecycle::active_authentication::load(root.path().into())
        .await
        .unwrap()
        .unwrap();
    crate::policy_lifecycle::active_authentication::revalidate(root.path().into(), active)
        .await
        .unwrap();
    let config = crate::policy_lifecycle::active_authentication::configuration(
        root.path().into(),
        "qualitygate.yaml".into(),
    )
    .await
    .unwrap();
    assert_eq!(config["review_trust"], "signed_active_policy");
    assert_eq!(config["config"]["rules"]["line-ending"]["enabled"], true);
    let active = crate::policy_lifecycle::active_authentication::load(root.path().into())
        .await
        .unwrap()
        .unwrap();
    let mut options = crate::application::test_support::options(root.path());
    let snapshot = crate::application::test_support::snapshot(root.path());
    let plan = crate::policy_lifecycle::active_authentication::prepare_plan(
        &active.active,
        &snapshot,
        &options,
    )
    .unwrap();
    assert_eq!(plan.evidence.trust, "signed_active_policy");
    options.policy_ref = Some("other-policy".into());
    assert!(
        crate::policy_lifecycle::active_authentication::prepare_plan(
            &active.active,
            &snapshot,
            &options
        )
        .is_err()
    );
    options.policy_ref = None;
    options.task = Some("unretained-task.yaml".into());
    assert!(
        crate::policy_lifecycle::active_authentication::prepare_plan(
            &active.active,
            &snapshot,
            &options
        )
        .is_err()
    );
    let current_options = crate::application::test_support::options(root.path());
    let projection =
        crate::application::agent_contract::project(current_options.clone(), "implementer".into())
            .await
            .unwrap();
    assert!(projection.ready);
    let report = crate::application::check(current_options).await.unwrap();
    assert_eq!(report.policy.trust, "signed_active_policy");
    assert!(report.gate.complete);
    let preflight = crate::application::doctor::run(
        crate::application::test_support::options(root.path()),
        false,
    )
    .await;
    assert!(preflight.complete, "{:?}", preflight.diagnostics);
    assert_eq!(preflight.policy.unwrap().trust, "signed_active_policy");
    // Exercise the active-plan consumer with an archived task, independently of signature admission.
    let mut scoped = crate::config::policy_active::load(root.path())
        .unwrap()
        .unwrap();
    let task=b"schema_version: 1\ntask_id: archived-task\nacceptance:\n  - id: unit\n    description: Verify behavior\n    verification: {check_id: unit, argv: [git, --version]}\n";
    let task_digest = Store::transaction(root.path(), |store| store.put_blob(task)).unwrap();
    scoped.version.files.insert(
        "tasks/unit.yaml".into(),
        crate::domain::evolution::PolicyFile {
            digest: task_digest,
            executable: false,
        },
    );
    scoped.config.verification_assets.push("tasks/**".into());
    let mut task_options = crate::application::test_support::options(root.path());
    task_options.task = Some("tasks/unit.yaml".into());
    let (resolved, catalog) = scoped.frozen.resolve(&scoped.config).unwrap();
    let mut input = crate::application::test_support::snapshot(root.path());
    input.scope_evidence.exclude = resolved.exclude.clone();
    input.scope_evidence.resource_exclusions.policy = resolved.unstructured_documents.clone();
    input.scope_evidence.resource_exclusions.protected_paths =
        crate::policy_lifecycle::policy_loading::protected_paths(
            &resolved,
            &catalog,
            &task_options.config,
            task_options.task.as_deref(),
        );
    let mut invalid = vec![];
    let plan = crate::policy_lifecycle::active_authentication::prepare(
        &scoped,
        &input,
        &task_options,
        &mut invalid,
    )
    .unwrap();
    assert_eq!(plan.plan.task_id.as_deref(), Some("archived-task"));
    assert!(
        invalid
            .iter()
            .any(|reason| reason.contains("verification asset differs"))
    );
    input.files.insert(
        "tasks/unit.yaml".into(),
        crate::snapshot::File {
            bytes: task.to_vec(),
            executable: false,
        },
    );
    invalid.clear();
    crate::policy_lifecycle::active_authentication::prepare(
        &scoped,
        &input,
        &task_options,
        &mut invalid,
    )
    .unwrap();
    assert!(invalid.is_empty());
    let original_trust = std::fs::read(&trust_path).unwrap();
    let mut trust: Value = serde_json::from_slice(&original_trust).unwrap();
    trust["evaluators"] = json!([digest(b"different evaluator")]);
    std::fs::write(&trust_path, serde_json::to_vec(&trust).unwrap()).unwrap();
    assert!(
        crate::policy_lifecycle::active_authentication::load(root.path().into())
            .await
            .is_err()
    );
    assert!(
        crate::policy_lifecycle::active_authentication::revalidate(root.path().into(), active)
            .await
            .is_err()
    );
    std::fs::write(&trust_path, original_trust).unwrap();
    let actor = Actor {
        id: "rollback-proposer".into(),
        kind: ActorKind::Human,
    };
    let request = crate::policy_lifecycle::rollback_workflow::subject(
        root.path().into(),
        baseline.clone(),
        trust_path.clone(),
        actor,
    )
    .await
    .unwrap();
    let envelope = signed(external.path(), &request, "approved");
    crate::policy_lifecycle::rollback_workflow::rollback(
        root.path().into(),
        baseline.clone(),
        trust_path.clone(),
        envelope.clone(),
    )
    .await
    .unwrap();
    let active = crate::policy_lifecycle::active_authentication::load(root.path().into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.active.reference, baseline);
    crate::policy_lifecycle::active_authentication::revalidate(root.path().into(), active)
        .await
        .unwrap();
    assert!(
        crate::policy_lifecycle::rollback_workflow::rollback(
            root.path().into(),
            baseline,
            trust_path,
            envelope
        )
        .await
        .is_err()
    );
}
