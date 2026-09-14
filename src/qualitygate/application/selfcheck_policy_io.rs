//! Disposable native policy archives and paired Git workflows for the shipped corpus.

use super::selfcheck_policy::{actor, budget, envelope, outcome, trust};
use crate::{
    config::{
        self,
        policy_candidates::{self, Proposal},
        policy_store::{Store, digest, now},
        rule_management::Mutation,
        selfcheck_policy::{CandidateAction, WorkflowScenario},
    },
    domain::{
        evolution::*,
        policy_evaluation::{CaseKind, Expected},
        rule_lifecycle::{RuleLifecycle, RuleState},
    },
    snapshot,
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const POLICY: &str = "schema_version: 1\nrules: {line-ending: {enabled: false}}\n";
const SOURCE: &[u8] = b"Require LF in changed files";

fn evidence(root: &Path, mismatch: bool) -> Result<Value> {
    policy_candidates::retain_evidence(
        root,
        EvidenceRecord {
            schema_version: 1,
            kind: EvidenceKind::ConversationCorrection,
            source_digest: digest(SOURCE),
            scope: "fixture".into(),
            timestamp: 1,
            actor: actor("generator"),
            claims: vec!["Require LF in changed files".into()],
            sensitivity: Sensitivity::Public,
            rule_ids: vec!["line-ending".into()],
            known_limits: vec!["Synthetic request; no downstream benefit claimed".into()],
            unverified_assumptions: Vec::new(),
        },
        if mismatch {
            b"different source"
        } else {
            SOURCE
        },
    )
}

fn create(root: &Path, commit: &str, evidence: &str) -> Result<Value> {
    policy_candidates::create_from_snapshot(
        root,
        "qualitygate.yaml",
        commit,
        [("qualitygate.yaml", POLICY.as_bytes(), false)],
        Proposal {
            actor: actor("generator"),
            reason: "Fixture LF proposal".into(),
            evidence: vec![evidence.into()],
        },
    )
}

fn field(value: &Value, key: &str) -> Result<String> {
    Ok(value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Fixture setup missing {key}"))?
        .into())
}

pub(super) fn candidate(action: CandidateAction) -> Result<Value> {
    use CandidateAction::*;
    let directory = tempfile::tempdir()?;
    let root = dunce::canonicalize(directory.path())?;
    if matches!(action, SourceMismatch) {
        return Ok(outcome(evidence(&root, true)));
    }
    let retained = evidence(&root, false)?;
    let evidence_ref = field(&retained, "evidence_ref")?;
    if matches!(action, MissingEvidence) {
        return Ok(outcome(create(
            &root,
            "fixture-commit",
            &digest(b"unretained"),
        )));
    }
    let initial = create(&root, "fixture-commit", &evidence_ref)?;
    let id = field(&initial, "id")?;
    let initial_ref = field(&initial, "revision_ref")?;
    let parent = field(&initial["revision"], "parent_policy_digest")?;
    let before = std::fs::read(root.join(".qualitygate/policy/HEAD"))?;
    let operation = match action {
        Edit => policy_candidates::mutate(
            &root,
            &id,
            actor("editor"),
            Mutation::Enable("line-ending".into()),
        ),
        Reject => {
            policy_candidates::reject(&root, &id, actor("reviewer"), "Fixture rejection")?;
            policy_candidates::mutate(
                &root,
                &id,
                actor("editor"),
                Mutation::Enable("line-ending".into()),
            )
        }
        CorruptObject => {
            std::fs::write(
                root.join(".qualitygate/policy/objects")
                    .join(initial_ref.trim_start_matches("sha256:")),
                b"corrupt",
            )?;
            return Ok(outcome(policy_candidates::show(&root, "candidate", &id)));
        }
        WrongRecordKind => Store::open(&root)?.record::<Value>(&initial_ref, "policy_approval"),
        Lifecycle { state: _ } | LifecycleWrongActor | LifecycleWrongEvidence => {
            policy_candidates::mutate(
                &root,
                &id,
                actor("editor"),
                Mutation::Enable("line-ending".into()),
            )?;
            let state = if let Lifecycle { state } = action {
                state
            } else {
                RuleState::Retired
            };
            let record = RuleLifecycle {
                state,
                actor: actor(if matches!(action, LifecycleWrongActor) {
                    "stranger"
                } else {
                    "editor"
                }),
                reason: "Fixture lifecycle evidence".into(),
                evidence_refs: vec![if matches!(action, LifecycleWrongEvidence) {
                    digest(b"other evidence")
                } else {
                    evidence_ref
                }],
                timestamp: 1,
            };
            policy_candidates::mutate(
                &root,
                &id,
                actor("editor"),
                Mutation::Lifecycle {
                    id: "line-ending".into(),
                    record,
                },
            )
        }
        UnapprovedPromotion => tokio::runtime::Handle::current()
            .block_on(super::policy_promotion::promote(root.clone(), id.clone())),
        MissingEvidence | SourceMismatch => unreachable!("handled before candidate setup"),
    };
    let store = Store::open(&root)?;
    let (reference, revision) = policy_candidates::candidate(&store, &id)?;
    let original: PolicyRevision = store.record(&initial_ref, "candidate")?;
    let (_, config, frozen) = policy_candidates::load_version(&store, &revision.policy_digest)?;
    let (resolved, _) = frozen.resolve(&config)?;
    let history = policy_candidates::history(&root, None, 256)?;
    Ok(
        json!({"operation":outcome(operation),"state":revision.status,
        "active_policy":store.index.active_policy,"parent_unchanged":revision.parent_policy_digest == parent,
        "original_unchanged":serde_json::to_value(original)? == initial["revision"],
        "revision_changed":reference != initial_ref,"head_unchanged":std::fs::read(root.join(".qualitygate/policy/HEAD"))? == before,
        "rule":resolved.rules.get("line-ending"),"lifecycle":config.rule_lifecycle.get("line-ending"),
        "patch":revision.patch,"evidence_trust":retained["trust"],
        "history_actions":history["events"].as_array().context("Fixture history missing events")?.iter().map(|event| &event["event"]["action"]).collect::<Vec<_>>()}),
    )
}

async fn git(root: &Path, args: &[&str]) -> Result<String> {
    Ok(
        String::from_utf8(snapshot::run_git(root, args, None).await?)?
            .trim()
            .into(),
    )
}

async fn commit(root: &Path) -> Result<String> {
    git(root, &["add", "."]).await?;
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
            "commit",
            "-qm",
            "[fixture]test: policy case",
        ],
    )
    .await?;
    git(root, &["rev-parse", "HEAD"]).await
}

// Called on the selfcheck blocking worker. Setup I/O does not occupy async scheduler threads.
pub(super) async fn workflow(scenario: WorkflowScenario, jobs: u16) -> Result<Value> {
    use WorkflowScenario::*;
    crate::env::require_isolated_git_environment()?;
    let directory = tempfile::tempdir()?;
    let root = dunce::canonicalize(directory.path())?;
    let external = tempfile::tempdir()?;
    let outside = dunce::canonicalize(external.path())?;
    git(&root, &["init", "-q", "--template="]).await?;
    git(&root, &["config", "core.autocrlf", "false"]).await?;
    git(&root, &["config", "core.safecrlf", "false"]).await?;
    std::fs::write(root.join("qualitygate.yaml"), POLICY)?;
    std::fs::write(root.join(".gitignore"), ".qualitygate/\n")?;
    let parent_commit = commit(&root).await?;
    let retained = evidence(&root, false)?;
    let evidence_ref = field(&retained, "evidence_ref")?;
    let initial = create(&root, &parent_commit, &evidence_ref)?;
    let id = field(&initial, "id")?;
    let parent = field(&initial["revision"], "parent_policy_digest")?;
    if !matches!(scenario, Block) {
        policy_candidates::mutate(
            &root,
            &id,
            actor("generator"),
            Mutation::Enable("line-ending".into()),
        )?;
    }
    let executable = crate::env::current_executable()?;
    if matches!(scenario, RuleBudget) {
        policy_candidates::mutate(
            &root,
            &id,
            actor("generator"),
            Mutation::Enable("diff-size".into()),
        )?;
    }
    let mut base = parent_commit;
    let mut cases = Vec::new();
    for (name, kind, content, expected) in [
        ("replay", CaseKind::Replay, "bad\r\n", Expected::Fail),
        ("held-out", CaseKind::HeldOut, "good\n", Expected::Pass),
        ("anchor", CaseKind::Anchor, "anchor\n", Expected::Pass),
    ] {
        std::fs::write(root.join(format!("{name}.txt")), content)?;
        let head = commit(&root).await?;
        let argv = if matches!(scenario, MissingTool) {
            vec![
                outside
                    .join("missing-fixture-producer")
                    .to_string_lossy()
                    .into_owned(),
            ]
        } else if matches!(scenario, Timeout) {
            // Git's fixed alias launches only our bounded probe. Using Git as the
            // producer also works for debug evaluators larger than the separate
            // 256 MiB producer-identity limit. The evaluator itself remains hashed.
            let native = executable.to_string_lossy().into_owned();
            let path = if cfg!(windows) {
                native.replace('\\', "/")
            } else {
                native
            };
            let quoted = format!("'{}'", path.replace('\'', "'\\''"));
            vec![
                "git".into(),
                "-c".into(),
                format!("alias.qualitygate-fixture=!{quoted} selfcheck-probe timeout"),
                "qualitygate-fixture".into(),
            ]
        } else {
            vec!["git".into(), "--version".into()]
        };
        let task = serde_json::from_value(
            json!({"schema_version":1,"task_id":format!("fixture-{name}"),
            "acceptance":[{"id":"probe","description":"Fixed native fixture producer","verification":{
                "check_id":"probe","argv":argv,"timeout_seconds":if matches!(scenario, Timeout) {1} else {5}}}]}),
        )?;
        let mut expectations = BTreeMap::from([
            ("line-ending".into(), expected),
            ("probe".into(), Expected::Pass),
        ]);
        if matches!(scenario, RuleBudget) {
            expectations.insert("diff-size".into(), Expected::Pass);
        }
        cases.push(config::policy_acceptance::ValidationCase {
            id: name.into(),
            kind,
            base,
            head: head.clone(),
            task,
            expectations,
        });
        base = head;
    }
    let suite = config::policy_acceptance::ValidationSuite {
        schema_version: 1,
        id: "fixture-policy-acceptance".into(),
        baseline_policy: parent.clone(),
        evaluator_epoch: "fixture-epoch".into(),
        motivating_evidence: vec![evidence_ref],
        budget: budget(),
        min_improvements: 1,
        max_rules: 32,
        max_rule_growth: if matches!(scenario, RuleBudget) { 0 } else { 4 },
        cases,
    };
    let suite_bytes = serde_json::to_vec(&suite)?;
    let trust = trust(
        &root,
        digest(&suite_bytes),
        parent.clone(),
        super::evaluator_digest().await?,
    );
    let suite_path = outside.join("suite.json");
    let trust_path = outside.join("trust.json");
    std::fs::write(&suite_path, suite_bytes)?;
    std::fs::write(&trust_path, serde_json::to_vec(&trust)?)?;
    let validation =
        super::policy_validation::validate(super::policy_validation::ValidateOptions {
            root: root.clone(),
            candidate_id: id.clone(),
            baseline: if matches!(scenario, WrongBaseline) {
                digest(b"wrong baseline")
            } else {
                parent.clone()
            },
            task: suite_path,
            trust_store: trust_path.clone(),
            evidence_dir: outside.clone(),
            jobs: Some(jobs),
            actor: actor("operator"),
        })
        .await;
    if matches!(scenario, WrongBaseline) {
        return Ok(outcome(validation));
    }
    let (evaluation, code) =
        validation.context("Native fixture validation failed before producing an evaluation")?;
    let mut result = json!({"validation":evaluation,"exit_code":code});
    let mut statuses = std::collections::BTreeSet::new();
    let mut blockers = std::collections::BTreeSet::new();
    for case in evaluation["evaluation"]["cases"]
        .as_array()
        .context("Fixture evaluation missing cases")?
    {
        for side in ["baseline", "candidate"] {
            if let Some(reference) = case[side]["report_ref"].as_str() {
                let report: crate::domain::Report =
                    Store::open(&root)?.record(reference, "check_report")?;
                blockers.extend(report.gate.blockers);
                for check in report.checks.iter().filter(|check| check.id == "probe") {
                    statuses.insert(
                        serde_json::to_value(check.execution.status)?
                            .as_str()
                            .context("Invalid execution status")?
                            .to_owned(),
                    );
                }
            }
        }
    }
    result["probe_statuses"] = json!(statuses);
    result["gate_blockers"] = json!(blockers);
    if matches!(scenario, Promote | Rollback | StaleRollback) && code == 0 {
        let request =
            super::policy_promotion::subject(root.clone(), id.clone(), trust_path.clone())
                .await
                .with_context(|| format!("Native fixture approval prerequisite: {result}"))?;
        let path = sign_request(&outside, &request, &trust)?;
        let (approval, approval_code) =
            super::policy_promotion::approve(root.clone(), id.clone(), path, trust_path.clone())
                .await?;
        result["approval_code"] = json!(approval_code);
        result["approval_state"] = approval["revision"]["status"].clone();
        // Approval must use retained report artifacts after the disposable run directory is gone.
        let evidence_directory = field(&evaluation, "evidence_directory")?;
        std::fs::remove_dir_all(evidence_directory)?;
        let promoted = super::policy_promotion::promote(root.clone(), id.clone()).await?;
        result["promoted_candidate"] =
            json!(promoted["active_policy"] == evaluation["evaluation"]["candidate_policy"]);
        if matches!(scenario, Rollback | StaleRollback) {
            let request = super::policy_rollback::subject(
                root.clone(),
                parent.clone(),
                trust_path.clone(),
                actor("operator"),
            )
            .await?;
            let path = sign_request(&outside, &request, &trust)?;
            if matches!(scenario, StaleRollback) {
                policy_candidates::create_from_version(
                    &root,
                    &parent,
                    Proposal {
                        actor: actor("generator"),
                        reason: "Advance history after signing".into(),
                        evidence: suite.motivating_evidence,
                    },
                )?;
            }
            result["rollback"] = outcome(
                super::policy_rollback::rollback(root.clone(), parent.clone(), trust_path, path)
                    .await,
            );
        }
    }
    let store = Store::open(&root)?;
    let (_, revision) = policy_candidates::candidate(&store, &id)?;
    result["state"] = serde_json::to_value(revision.status)?;
    result["active_is_parent"] = json!(store.index.active_policy.as_deref() == Some(&parent));
    result["active_is_candidate"] =
        json!(store.index.active_policy.as_deref() == Some(&revision.policy_digest));
    result["has_active"] = json!(store.index.active_policy.is_some());
    result["effectiveness"] = config::policy_effectiveness::report(&root, None, 0, 256)?;
    Ok(result)
}

fn sign_request(
    outside: &Path,
    request: &Value,
    trust: &config::policy_acceptance::EvolutionTrust,
) -> Result<PathBuf> {
    let issued_at = now()?;
    let record = json!({"schema_version":1,"subject":request["subject"],"approver":trust.approval_keys[0].actor,
        "decision":"approved","issued_at":issued_at,"expires_at":issued_at+300,"reason":"Disposable fixture authorization"});
    let path = outside.join("approval.json");
    std::fs::write(
        &path,
        envelope(&field(request, "payload_type")?, &record, false, false)?,
    )?;
    Ok(path)
}
