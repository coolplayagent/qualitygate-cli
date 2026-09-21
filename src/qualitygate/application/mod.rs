//! Coordinates policy selection, snapshot execution and evidence-bound reports.

mod commands;
mod compatibility;
mod compatibility_inputs;
mod coverage_gate;
mod evidence;
mod external;
pub mod feedback;
mod generated_reports;
mod git_trailers;
pub mod judgment;
mod manual;
pub mod pilot;
mod policy;
pub mod policy_active;
pub mod policy_candidates;
pub mod policy_promotion;
pub mod policy_rollback;
pub mod policy_validation;
mod project_reports;
mod provenance;
mod python_install;
mod report_gate;
pub mod rule_context;
mod rule_execution;
pub mod selfcheck;
mod selfcheck_evidence;
mod selfcheck_io;
mod selfcheck_policy;
mod selfcheck_policy_io;
mod test_counts;
mod test_effectiveness;
mod tool_evidence;

use crate::{
    config::{self, Plan},
    domain::*,
    snapshot::{self, Selection},
};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub root: PathBuf,
    pub config: String,
    pub selection: Selection,
    pub snapshot_options: snapshot::CaptureOptions,
    pub profile: String,
    pub task: Option<String>,
    pub policy_ref: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub trust_store: Option<PathBuf>,
    pub evidence_dir: Option<PathBuf>,
}

pub async fn check(options: CheckOptions) -> Result<Report> {
    check_with_expected_base(options, None).await
}

/// Replays may require selectors with an implicit base to retain its identity.
pub async fn check_with_expected_base(
    options: CheckOptions,
    expected_base: Option<&str>,
) -> Result<Report> {
    let snapshot = Arc::new(
        snapshot::capture_with_options(
            &options.root,
            &options.selection,
            &options.snapshot_options,
        )
        .await?,
    );
    if expected_base.is_some_and(|expected| expected != snapshot.identity.base) {
        anyhow::bail!(
            "Snapshot base differs from the expected base; select a new comparison explicitly"
        );
    }
    let mut invalid = Vec::new();
    let active = policy_active::load(options.root.clone()).await?;
    let (loaded, active) = if let Some(active) = active {
        if active.active.evaluator_digest() != evaluator_digest().await? {
            anyhow::bail!("Active policy requires revalidation with this evaluator executable");
        }
        let (snapshot, options) = (Arc::clone(&snapshot), options.clone());
        let (loaded, errors, active) = tokio::task::spawn_blocking(move || {
            let mut errors = Vec::new();
            Ok::<_, anyhow::Error>((
                policy_active::prepare(&active.active, &snapshot, &options, &mut errors)?,
                errors,
                active,
            ))
        })
        .await??;
        invalid.extend(errors);
        (loaded, Some(active))
    } else {
        (policy::load(&snapshot, &options, &mut invalid).await?, None)
    };
    check_prepared(options, snapshot, loaded, invalid, true, active, None).await
}

async fn evaluator_digest() -> Result<String> {
    static IDENTITY: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    Ok(IDENTITY
        .get_or_try_init(|| async {
            Ok::<_, anyhow::Error>(crate::runner::identity::evaluator().await?.digest)
        })
        .await?
        .clone())
}

async fn check_prepared(
    options: CheckOptions,
    snapshot: Arc<snapshot::Snapshot>,
    loaded: policy::Loaded,
    mut invalid: Vec<String>,
    revalidate_source: bool,
    active: Option<policy_active::Authenticated>,
    recheck_argv: Option<Vec<String>>,
) -> Result<Report> {
    let evaluator_digest = evaluator_digest().await?;
    let environment_digest = crate::env::environment_digest()?;
    let policy::Loaded {
        catalog,
        evidence: policy,
        plan,
        protected_paths,
    } = loaded;
    let git_facts = git_trailers::load(&plan, &catalog, &snapshot).await;
    let external = match (&options.trust_store, &options.evidence_dir) {
        (Some(store), Some(directory)) => {
            let (root, store, directory, requests) = (
                options.root.clone(),
                store.clone(),
                directory.clone(),
                external::requests(&plan),
            );
            match tokio::task::spawn_blocking(move || {
                external::load(&root, &store, &directory, &requests)
            })
            .await?
            {
                Ok(inputs) => Some(Arc::new(inputs)),
                Err(error) => {
                    invalid.push(format!("External acceptance inputs are invalid: {error:#}"));
                    None
                }
            }
        }
        (None, None) => None,
        _ => {
            invalid.push("External acceptance requires both trust_store and evidence_dir".into());
            None
        }
    };
    let directory = crate::paths::run_directory(options.output_dir.as_deref())?;
    let run_id = directory
        .file_name()
        .context("Missing run directory name")?
        .to_string_lossy()
        .to_string();
    let mut results: Vec<CheckResult> = Vec::new();
    let mut provenance_proofs = std::collections::BTreeMap::new();
    let workspace = if plan.commands.iter().any(|check| {
        check.kind == config::CheckKind::Command
            && check.compatibility.is_none()
            && check.test_effectiveness.is_none()
    }) && invalid.is_empty()
    {
        Some(snapshot::materialize_shared(Arc::clone(&snapshot)).await?)
    } else {
        None
    };
    let input_guard = if let Some(workspace) = &workspace {
        Some(snapshot::InputGuard::for_snapshot(workspace.path(), Arc::clone(&snapshot)).await?)
    } else {
        None
    };
    for id in &plan.order {
        let rule = plan.rules.get(id);
        let command = plan.commands.iter().find(|command| &command.id == id);
        let (required, severity, dependencies) = if let Some(rule) = rule {
            (rule.required, rule.severity, &rule.depends_on)
        } else {
            let command = command.expect("planned command");
            (command.required, command.severity, &command.depends_on)
        };
        let dependency_failure = dependencies.iter().find(|id| {
            !results.iter().any(|result| {
                &result.id == *id
                    && result.execution.status == ExecutionStatus::Completed
                    && result.verdict == Some(Verdict::Pass)
            })
        });
        let mut result = if let Some(dependency) = dependency_failure {
            let mut result = CheckResult::pending(id, required, severity);
            result.block(
                ExecutionStatus::Blocked,
                format!("Prerequisite {dependency} did not pass"),
            );
            result
        } else if let Some(rule) = rule {
            match provenance::prepare(
                id,
                rule,
                &catalog.entries[id],
                &snapshot,
                &policy,
                external.as_ref(),
                &directory,
            )
            .await
            {
                Ok(proof) => {
                    let mut result = rule_execution::execute(
                        id,
                        rule,
                        &catalog.entries[id],
                        &snapshot,
                        &results,
                        proof.as_ref().map(|proof| &proof.facts),
                        git_facts.as_ref(),
                    )
                    .await?;
                    if let Some(proof) = proof {
                        result.execution.artifacts.extend(proof.artifacts);
                        result
                            .metadata
                            .insert("external_provenance".into(), proof.metadata);
                        provenance_proofs.insert(id.clone(), proof.facts);
                    }
                    result
                }
                Err(result) => *result,
            }
        } else if let Some(command) = command
            && command.kind == config::CheckKind::Manual
            && invalid.is_empty()
        {
            manual::execute(
                command,
                &plan,
                &snapshot,
                &policy,
                external.as_ref(),
                &directory,
            )
            .await
        } else if let Some(command) = command
            && command.compatibility.is_some()
            && invalid.is_empty()
        {
            compatibility::execute(command, &directory, &snapshot).await
        } else if let Some(command) = command
            && command.test_effectiveness.is_some()
            && invalid.is_empty()
        {
            test_effectiveness::execute(
                command,
                &directory,
                &snapshot,
                &protected_paths,
                options.snapshot_options.max_bytes,
            )
            .await
        } else if let (Some(command), Some(workspace), Some(inputs)) =
            (command, &workspace, &input_guard)
        {
            commands::execute(command, workspace.path(), &directory, &snapshot, inputs).await
        } else {
            let mut result = CheckResult::pending(id, required, severity);
            result.block(
                ExecutionStatus::Blocked,
                "Policy validation did not complete",
            );
            result
        };
        result
            .metadata
            .insert("depends_on".into(), serde_json::json!(dependencies));
        if let Some(command) = command {
            result
                .metadata
                .insert("command_definition".into(), serde_json::to_value(command)?);
        }
        if rule.is_some() {
            let entry = &catalog.entries[id];
            result.rule_version = entry.version();
            result
                .metadata
                .insert("rule_definition".into(), serde_json::json!(entry));
            if let Some(review) = policy.source_reviews.get(id) {
                result.metadata.insert("source_review".into(), serde_json::json!({"binding":review,"policy_source":policy.source,"policy_trust":policy.trust}));
                if review.status != ReviewStatus::Bound {
                    let earlier = result
                        .execution
                        .reason
                        .as_ref()
                        .map(|reason| format!("{reason}; "))
                        .unwrap_or_default();
                    result.block(ExecutionStatus::Blocked, format!("{earlier}Rule source review is missing or stale; inspect rules list and record a review in the selected policy"));
                }
            }
        }
        results.push(result);
    }
    if let Some(inputs) = &input_guard
        && let Err(error) = inputs.verify().await
    {
        invalid.push(format!("Execution inputs are invalid: {error:#}"));
    }
    if let Some(external) = external
        && let Err(error) =
            manual::revalidate(external.clone(), &plan, &snapshot, &policy, &mut results)
                .await
                .and_then(|()| provenance::revalidate(&external, &provenance_proofs, &mut results))
    {
        invalid.push(format!(
            "External acceptance evidence is invalid: {error:#}"
        ));
        for result in &mut results {
            if result.metadata.contains_key("manual_acceptance")
                || result.metadata.contains_key("external_provenance")
            {
                result.block(ExecutionStatus::Blocked, format!("External acceptance evidence changed or could not be revalidated: {error:#}"));
                for name in ["manual_acceptance", "external_provenance"] {
                    if let Some(evidence) = result.metadata.get_mut(name) {
                        evidence["valid_at_completion"] = serde_json::json!(false);
                    }
                }
            }
        }
    }
    if revalidate_source {
        match snapshot::capture_with_options(
            &options.root,
            &options.selection,
            &options.snapshot_options,
        )
        .await
        {
            Ok(current)
                if current.identity.content_digest != snapshot.identity.content_digest
                    || current.identity.base != snapshot.identity.base
                    || current.identity.head != snapshot.identity.head
                    || match (
                        &current.identity.merge_request,
                        &snapshot.identity.merge_request,
                    ) {
                        (Some(current), Some(previous)) => !current.same_comparison(previous),
                        (None, None) => false,
                        _ => true,
                    } =>
            {
                invalid
                    .push("Source snapshot changed during checks; recheck the final state".into())
            }
            Ok(_) => {}
            Err(error) => invalid.push(format!("Cannot revalidate the source snapshot: {error:#}")),
        }
    }
    if crate::env::environment_digest()? != environment_digest {
        invalid.push("Process environment changed during evaluation".into());
    }
    if let Some(active) = active
        && let Err(error) = policy_active::revalidate(options.root.clone(), active).await
    {
        invalid.push(format!("Active policy authorization is invalid: {error:#}"));
    }
    let recheck = recheck_argv.unwrap_or_else(|| {
        recheck(
            &options,
            &snapshot.identity.base,
            if policy.trust == "signed_active_policy" {
                Some(&policy.source)
            } else {
                policy.resolved_commit.as_deref()
            },
        )
    });
    for result in &mut results {
        for diagnostic in &mut result.diagnostics {
            diagnostic.recheck.argv = recheck.clone();
        }
    }
    let summary = Summary::from_checks(&results);
    let gate = evaluate(&results, &plan.required, &invalid);
    let mut delivery_options = options.clone();
    delivery_options.profile = "full".into();
    delivery_options.snapshot_options.path_filter = None;
    if let Selection::Path { base, .. } = &options.selection {
        delivery_options.selection = Selection::Worktree { base: base.clone() };
    }
    let delivery_recheck = self::recheck(
        &delivery_options,
        &snapshot.identity.base,
        if policy.trust == "signed_active_policy" {
            Some(&policy.source)
        } else {
            policy.resolved_commit.as_deref()
        },
    );
    let report = Report {
        context: Some(ReportContext {
            report_path: directory.join("report.json").display().to_string(),
            recheck: Recheck { argv: recheck },
            delivery_recheck: Recheck {
                argv: delivery_recheck,
            },
        }),
        verification: VerificationBoundary::for_check(
            &gate,
            &results,
            &options.profile,
            snapshot.path_filter.is_some(),
        ),
        schema_version: 1,
        run_id,
        scope: if snapshot.path_filter.is_some() {
            "path"
        } else if plan.task_id.is_some() {
            "task"
        } else {
            "repository"
        }
        .into(),
        profile: options.profile,
        evaluator_digest,
        environment_digest,
        snapshot: snapshot.identity.clone(),
        policy,
        plan: PlanSummary {
            task_id: plan.task_id,
            execution_order: plan.order,
            required_checks: plan.required,
            pending_delivery_checks: plan.pending_delivery,
            acceptance: plan.acceptance,
            acceptance_descriptions: plan.acceptance_descriptions,
        },
        gate,
        checks: results,
        summary,
    };
    tokio::fs::write(
        directory.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )
    .await?;
    Ok(report)
}

fn recheck(options: &CheckOptions, base: &str, policy_commit: Option<&str>) -> Vec<String> {
    let mut argv = vec![
        "qualitygate".into(),
        "--root".into(),
        options.root.display().to_string(),
        "check".into(),
    ];
    match &options.selection {
        Selection::Worktree { .. } => {
            argv.extend(["--worktree".into(), "--base".into(), base.into()])
        }
        Selection::Staged => argv.push("--staged".into()),
        Selection::Diff { head, .. } => argv.extend(["--diff".into(), format!("{base}..{head}")]),
        Selection::Path { path, .. } => {
            argv.extend(["--path".into(), path.clone(), "--base".into(), base.into()])
        }
        Selection::MergeRequest { url, api_base } => {
            argv.extend(["--mr".into(), url.clone()]);
            if let Some(base) = api_base {
                argv.extend(["--mr-api-base".into(), base.clone()]);
            }
        }
    }
    argv.extend([
        "--expect-base".into(),
        base.into(),
        "--snapshot-max-mib".into(),
        (options.snapshot_options.max_bytes / (1024 * 1024)).to_string(),
        "--snapshot-max-file-mib".into(),
        options
            .snapshot_options
            .max_file_bytes
            .div_ceil(1024 * 1024)
            .to_string(),
        "--snapshot-jobs".into(),
        options.snapshot_options.jobs.to_string(),
        "--snapshot-timeout-secs".into(),
        options.snapshot_options.timeout.as_secs().to_string(),
        "--config".into(),
        options.config.clone(),
        "--profile".into(),
        options.profile.clone(),
        "--format".into(),
        "json".into(),
    ]);
    if let Some(path) = &options.snapshot_options.path_filter {
        argv.extend(["--path".into(), path.clone()]);
    }
    if let Some(task) = &options.task {
        argv.extend(["--task".into(), task.clone()]);
    }
    if let Some(reference) = policy_commit.or(options.policy_ref.as_deref()) {
        argv.extend(["--policy-ref".into(), reference.into()]);
    }
    if let Some(path) = &options.trust_store {
        argv.extend(["--trust-store".into(), path.display().to_string()]);
    }
    if let Some(path) = &options.evidence_dir {
        argv.extend(["--evidence-dir".into(), path.display().to_string()]);
    }
    argv
}
