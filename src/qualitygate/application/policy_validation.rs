//! Bounded paired runs over shared immutable snapshots and protected independent tasks.

use crate::{
    config::{
        self,
        policy_acceptance::{ProtectedInputs, ValidationCase},
        policy_candidates,
        policy_store::{Store, digest, now},
        policy_validation::Attempt,
    },
    domain::{
        evolution::{Actor, PolicyRevision, PolicyVersion},
        policy_evaluation::*,
        *,
    },
    snapshot,
};
use anyhow::{Context, Result, bail};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

pub struct ValidateOptions {
    pub root: PathBuf,
    pub candidate_id: String,
    pub baseline: String,
    pub task: PathBuf,
    pub trust_store: PathBuf,
    pub evidence_dir: PathBuf,
    pub jobs: Option<u16>,
    pub actor: Actor,
}

struct PreparedPolicy {
    version: PolicyVersion,
    config: config::Config,
    catalog: config::catalog::Catalog,
}
struct Prepared {
    inputs: Arc<ProtectedInputs>,
    baseline: Arc<PreparedPolicy>,
    candidate: Arc<PreparedPolicy>,
    revision: PolicyRevision,
    revision_ref: String,
}

fn prepare(options: &ValidateOptions, evaluator: &str) -> Result<Prepared> {
    let inputs = ProtectedInputs::load(&options.root, &options.task, &options.trust_store)?;
    crate::adapters::policy_approval::validate_keys(&inputs.trust)?;
    if !inputs
        .trust
        .evaluators
        .iter()
        .any(|digest| digest == evaluator)
    {
        bail!("This evaluator executable is not authorized in the protected acceptance epoch");
    }
    let store = Store::open(&options.root)?;
    let (revision_ref, revision) = policy_candidates::candidate(&store, &options.candidate_id)?;
    if !revision.editable()
        || revision.parent_policy_digest != options.baseline
        || inputs.suite.baseline_policy != options.baseline
    {
        bail!(
            "Validation baseline must be the candidate's immutable parent and match the protected suite"
        );
    }
    for evidence in &inputs.suite.motivating_evidence {
        if !revision.evidence_refs.contains(evidence) {
            bail!("Protected replay evidence does not motivate this candidate");
        }
        policy_candidates::evidence(&store, evidence)?;
    }
    config::case_provenance::validate_suite(&store, &inputs.suite, &revision)?;
    let load = |reference: &str| -> Result<_> {
        let (version, config, frozen) = policy_candidates::load_version(&store, reference)?;
        let (config, catalog) = frozen.resolve(&config)?;
        Ok(Arc::new(PreparedPolicy {
            version,
            config,
            catalog,
        }))
    };
    let baseline = load(&options.baseline)?;
    let candidate = load(&revision.policy_digest)?;
    Ok(Prepared {
        inputs: Arc::new(inputs),
        baseline,
        candidate,
        revision,
        revision_ref,
    })
}

pub async fn evaluator() -> Result<serde_json::Value> {
    Ok(
        serde_json::json!({"schema_version":1,"evaluator_digest":super::evaluator_digest().await?,"environment_digest":crate::env::environment_digest()?,"engine_version":env!("CARGO_PKG_VERSION")}),
    )
}

pub async fn validate(options: ValidateOptions) -> Result<(serde_json::Value, u8)> {
    let evaluator = super::evaluator_digest().await?;
    let environment = crate::env::environment_digest()?;
    let options = Arc::new(options);
    let prepared = {
        let options = Arc::clone(&options);
        let evaluator = evaluator.clone();
        tokio::task::spawn_blocking(move || prepare(&options, &evaluator)).await??
    };
    let budget = prepared.inputs.suite.budget.clone();
    let jobs = options.jobs.unwrap_or(budget.max_parallel);
    if jobs == 0 || jobs > budget.max_parallel {
        bail!("Validation jobs must be within the protected parallelism budget");
    }
    let directory = {
        let options = Arc::clone(&options);
        tokio::task::spawn_blocking(move || {
            let root = dunce::canonicalize(&options.root)?;
            let output = dunce::canonicalize(&options.evidence_dir)?;
            if output.starts_with(&root) || !output.is_dir() {
                bail!("Validation evidence directory must exist outside the checked repository");
            }
            crate::paths::run_directory(Some(&output))
        })
        .await??
    };
    let attempt = Attempt {
        candidate_id: options.candidate_id.clone(),
        input_revision: prepared.revision_ref.clone(),
        candidate_policy: prepared.revision.policy_digest.clone(),
        baseline_policy: options.baseline.clone(),
        suite_digest: digest(&prepared.inputs.suite_file.bytes),
        trust_digest: digest(&prepared.inputs.trust_file.bytes),
        evaluator_digest: evaluator.clone(),
        actor: options.actor.clone(),
        started_at: now()?,
    };
    let frozen = {
        let options = Arc::clone(&options);
        let attempt = attempt.clone();
        let inputs = Arc::clone(&prepared.inputs);
        tokio::task::spawn_blocking(move || {
            config::policy_validation::begin(
                &options.root,
                &attempt,
                &inputs.suite_file.bytes,
                &inputs.trust_file.bytes,
            )
        })
        .await??
    };
    let mut invalid = Vec::new();
    let mut violations = Vec::new();
    let candidate_count = prepared.candidate.config.rules.len();
    let baseline_count = prepared.baseline.config.rules.len();
    if candidate_count > prepared.inputs.suite.max_rules
        || candidate_count.saturating_sub(baseline_count) > prepared.inputs.suite.max_rule_growth
    {
        violations.push("Candidate exceeds the protected rule-library contribution budget".into());
    }
    let prepared = Arc::new(prepared);
    let mut cases = Vec::new();
    let semaphore = Arc::new(Semaphore::new(jobs as usize));
    let parallel_cases = usize::from(jobs)
        .min((budget.max_live_snapshot_mib / (budget.snapshot_max_mib * 2)) as usize);
    let mut pending = tokio::task::JoinSet::new();
    let mut next = 0;
    let started = Instant::now();
    while invalid.is_empty() && (next < prepared.inputs.suite.cases.len() || !pending.is_empty()) {
        while next < prepared.inputs.suite.cases.len() && pending.len() < parallel_cases {
            let case = prepared.inputs.suite.cases[next].clone();
            next += 1;
            let (prepared, options, directory, semaphore) = (
                Arc::clone(&prepared),
                Arc::clone(&options),
                directory.clone(),
                Arc::clone(&semaphore),
            );
            pending.spawn(async move {
                let id = case.id.clone();
                let result = tokio::time::timeout(
                    Duration::from_secs(prepared.inputs.suite.budget.case_timeout_seconds.into()),
                    run_case(&options, &prepared, &case, &directory, &semaphore),
                )
                .await;
                (id, result)
            });
        }
        let remaining = Duration::from_secs(budget.total_timeout_seconds.into())
            .saturating_sub(started.elapsed());
        match tokio::time::timeout(remaining, pending.join_next()).await {
            Ok(Some(Ok((id, Ok(Ok((case, pair))))))) => {
                if !case.baseline.complete || !case.candidate.complete {
                    invalid.push(format!(
                        "{id}: execution incomplete; stopped scheduling additional cases"
                    ));
                }
                let root = options.root.clone();
                match tokio::task::spawn_blocking(move || {
                    let mut case = case;
                    config::policy_validation::retain_reports(&root, &mut case, &pair.0, &pair.1)?;
                    Ok::<_, anyhow::Error>(case)
                })
                .await?
                {
                    Ok(case) => cases.push(case),
                    Err(error) => {
                        invalid.push(format!("{id}: could not archive paired reports: {error:#}"))
                    }
                }
            }
            Ok(Some(Ok((id, Ok(Err(error)))))) => invalid.push(format!("{id}: {error:#}")),
            Ok(Some(Ok((id, Err(_))))) => invalid.push(format!("{id}: paired case timed out")),
            Ok(Some(Err(error))) => invalid.push(format!("Evaluation worker failed: {error}")),
            Ok(None) => break,
            Err(_) => invalid.push("Evaluation exceeded its total time budget".into()),
        }
    }
    pending.abort_all();
    while pending.join_next().await.is_some() {}
    for case in &prepared.inputs.suite.cases {
        if !cases.iter().any(|result| result.id == case.id) {
            cases.push(EvaluatedCase {
                id: case.id.clone(),
                kind: case.kind,
                base: case.base.clone(),
                head: case.head.clone(),
                snapshot_digest: None,
                task_digest: digest(&serde_json::to_vec(&case.task)?),
                baseline: Observation::incomplete("Paired execution did not complete".into()),
                candidate: Observation::incomplete("Paired execution did not complete".into()),
            });
        }
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let integrity = {
        let (prepared, root) = (Arc::clone(&prepared), options.root.clone());
        tokio::task::spawn_blocking(move || prepared.inputs.unchanged(&root)).await?
    };
    if let Err(error) = integrity {
        invalid.push(format!("{error:#}"));
    }
    if crate::env::environment_digest()? != environment {
        invalid.push("Evaluation environment changed".into());
    }
    match crate::runner::identity::evaluator().await {
        Ok(current) if current.digest == evaluator => {}
        Ok(_) => invalid.push("Evaluator executable changed during evaluation".into()),
        Err(error) => invalid.push(format!("Cannot revalidate evaluator executable: {error:#}")),
    }
    let (mut conclusion, mut reasons) = decide(
        &cases,
        prepared.inputs.suite.cases.len(),
        prepared.inputs.suite.min_improvements,
        &invalid,
    );
    if !violations.is_empty() && conclusion != Conclusion::Incomplete {
        conclusion = Conclusion::Block;
    }
    reasons.extend(violations);
    let verification = VerificationBoundary { conclusion: match conclusion { Conclusion::Pass => "Candidate agrees with the protected paired and held-out acceptance cases", Conclusion::Block => "Candidate does not satisfy protected acceptance", Conclusion::Incomplete => "Protected evaluation did not complete" }.into(), verified_shapes: cases.iter().filter(|case| case.baseline.complete && case.candidate.complete).map(|case| format!("{}: {:?}, {}..{}", case.id, case.kind, case.base, case.head)).collect(), known_limits: vec!["Results cover only the protected cases, frozen evaluator epoch and matched resource budget.".into(), "Separate execution directories and immutable input guards do not provide an operating-system security sandbox.".into()], unverified_assumptions: vec!["Real downstream use, review effort and production benefit require later observations.".into()] };
    let run = EvaluationRun {
        schema_version: 1,
        candidate_id: options.candidate_id.clone(),
        candidate_revision: prepared.revision_ref.clone(),
        candidate_policy: prepared.revision.policy_digest.clone(),
        baseline_policy: options.baseline.clone(),
        suite_digest: attempt.suite_digest,
        trust_digest: attempt.trust_digest,
        evaluator_epoch: prepared.inputs.suite.evaluator_epoch.clone(),
        evaluator_digest: evaluator,
        environment_digest: environment,
        budget,
        jobs,
        generation_actor: prepared.revision.created_by.clone(),
        evaluation_actor: options.actor.clone(),
        started_at: attempt.started_at,
        ended_at: now()?,
        cases,
        conclusion,
        reasons,
        verification,
    };
    tokio::fs::write(
        directory.join("evaluation.json"),
        serde_json::to_vec_pretty(&run)?,
    )
    .await?;
    let root = options.root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut run = run;
        let mut result = config::policy_validation::finish(&root, &frozen, &mut run)?;
        result["evidence_directory"] = serde_json::json!(directory);
        Ok::<_, anyhow::Error>(result)
    })
    .await??;
    Ok((result, conclusion.exit_code()))
}

type CaseReports = (Report, Report);
async fn run_case(
    options: &ValidateOptions,
    prepared: &Prepared,
    case: &ValidationCase,
    directory: &Path,
    semaphore: &Arc<Semaphore>,
) -> Result<(EvaluatedCase, CaseReports)> {
    let b = &prepared.inputs.suite.budget;
    let capture = snapshot::CaptureOptions {
        max_bytes: b.snapshot_max_mib as usize * 1024 * 1024,
        max_file_bytes: snapshot::MAX_FILE_BYTES,
        jobs: b.snapshot_jobs.into(),
        timeout: Duration::from_secs(b.snapshot_timeout_seconds.into()),
        path_filter: None,
    };
    let snapshot = Arc::new(
        snapshot::capture_with_options(
            &options.root,
            &snapshot::Selection::Diff {
                base: case.base.clone(),
                head: case.head.clone(),
            },
            &capture,
        )
        .await?,
    );
    let run = |policy: Arc<PreparedPolicy>, policy_digest: String| {
        let (snapshot, capture) = (Arc::clone(&snapshot), capture.clone());
        async move {
            let _permit = semaphore
                .acquire()
                .await
                .context("Evaluation scheduler closed")?;
            let start = Instant::now();
            let task = case.task.clone();
            let digest = policy_digest.clone();
            let prepared =
                tokio::task::spawn_blocking(move || prepare_check(&policy, &task, &digest))
                    .await??;
            let recheck = validation_recheck(options);
            let options = super::CheckOptions {
                root: options.root.clone(),
                config: prepared.evidence.source.clone(),
                selection: snapshot::Selection::Diff {
                    base: case.base.clone(),
                    head: case.head.clone(),
                },
                snapshot_options: capture,
                profile: "full".into(),
                task: Some(case.task.task_id.clone()),
                policy_ref: None,
                output_dir: Some(directory.into()),
                trust_store: None,
                evidence_dir: None,
            };
            let report = super::check_prepared(
                options,
                snapshot,
                prepared,
                Vec::new(),
                false,
                None,
                Some(recheck),
            )
            .await?;
            let observation = Observation::from_report(
                &report,
                &case.expectations,
                start.elapsed().as_millis().try_into()?,
            );
            Ok::<_, anyhow::Error>((observation, report))
        }
    };
    let (baseline, candidate) = tokio::join!(
        run(Arc::clone(&prepared.baseline), options.baseline.clone()),
        run(
            Arc::clone(&prepared.candidate),
            prepared.revision.policy_digest.clone()
        )
    );
    let (baseline, baseline_report) = baseline?;
    let (candidate, candidate_report) = candidate?;
    if baseline_report.snapshot.content_digest != candidate_report.snapshot.content_digest
        || baseline_report.policy.task_contract_digest
            != candidate_report.policy.task_contract_digest
        || baseline_report.evaluator_digest != candidate_report.evaluator_digest
        || baseline_report.environment_digest != candidate_report.environment_digest
        || !matched_producers(
            &producer_environment(&baseline_report),
            &producer_environment(&candidate_report),
        )
    {
        bail!("Paired reports do not bind identical snapshot/task/evaluator/environment inputs");
    }
    Ok((
        EvaluatedCase {
            id: case.id.clone(),
            kind: case.kind,
            base: case.base.clone(),
            head: case.head.clone(),
            snapshot_digest: Some(snapshot.identity.content_digest.clone()),
            task_digest: digest(&serde_json::to_vec(&case.task)?),
            baseline,
            candidate,
        },
        (baseline_report, candidate_report),
    ))
}

fn prepare_check(
    policy: &PreparedPolicy,
    task: &config::TaskContract,
    reference: &str,
) -> Result<super::policy::Loaded> {
    let plan = config::Plan::build(&policy.config, Some(task), "full")?;
    if plan.order.len() > 512 {
        bail!("Evaluation plan exceeds 512 checks");
    }
    let evidence = PolicyEvidence {
        source: reference.into(),
        resolved_commit: Some(policy.version.source_commit.clone()),
        config_digest: policy.version.files[&policy.version.config_path]
            .digest
            .clone(),
        rules_digest: digest(&serde_json::to_vec(
            &serde_json::json!({"settings":policy.config.rules,"definitions":policy.catalog,"source_reviews":policy.config.source_reviews,"engine_version":env!("CARGO_PKG_VERSION")}),
        )?),
        task_contract_digest: Some(digest(&serde_json::to_vec(task)?)),
        task_contract_source: Some(task.task_id.clone()),
        trust: "protected_paired_evaluation".into(),
        changes: Vec::new(),
        source_reviews: config::source_reviews::evidence(&policy.config, &policy.catalog)?,
    };
    Ok(super::policy::Loaded {
        protected_paths: super::policy::protected_paths(
            &policy.config,
            &policy.catalog,
            &policy.version.config_path,
            None,
        ),
        catalog: policy.catalog.clone(),
        evidence,
        plan,
    })
}

fn validation_recheck(options: &ValidateOptions) -> Vec<String> {
    let mut argv = vec![
        "qualitygate".into(),
        "--root".into(),
        options.root.display().to_string(),
        "policy".into(),
        "candidate".into(),
        "validate".into(),
        options.candidate_id.clone(),
        "--baseline".into(),
        options.baseline.clone(),
        "--task".into(),
        options.task.display().to_string(),
        "--trust-store".into(),
        options.trust_store.display().to_string(),
        "--evidence-dir".into(),
        options.evidence_dir.display().to_string(),
        "--actor".into(),
        options.actor.id.clone(),
        "--actor-kind".into(),
        if options.actor.kind == crate::domain::evolution::ActorKind::Human {
            "human"
        } else {
            "agent"
        }
        .into(),
        "--format".into(),
        "json".into(),
    ];
    if let Some(jobs) = options.jobs {
        argv.extend(["--jobs".into(), jobs.to_string()]);
    }
    argv
}
