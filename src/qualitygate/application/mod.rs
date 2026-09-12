//! Coordinates policy selection, snapshot execution and evidence-bound reports.

mod commands;
mod compatibility;
mod compatibility_inputs;
mod coverage_gate;
mod evidence;
mod external;
mod generated_reports;
mod git_trailers;
mod manual;
mod policy;
mod project_reports;
mod provenance;
mod python_install;
mod report_gate;
mod rule_execution;
mod test_counts;
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
    pub profile: String,
    pub task: Option<String>,
    pub policy_ref: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub trust_store: Option<PathBuf>,
    pub evidence_dir: Option<PathBuf>,
}

pub async fn check(options: CheckOptions) -> Result<Report> {
    let snapshot = Arc::new(snapshot::capture(&options.root, &options.selection).await?);
    if !snapshot.files.contains_key(&options.config) {
        anyhow::bail!(
            "Configuration is not present in the checked snapshot; run init and stage/commit it as appropriate"
        );
    }
    let mut invalid = Vec::new();
    let (config, catalog, mut policy) = policy::load(&snapshot, &options, &mut invalid).await?;
    let task_bytes = options
        .task
        .as_ref()
        .map(|path| {
            snapshot
                .files
                .get(path)
                .with_context(|| format!("Task contract missing from checked snapshot: {path}"))
        })
        .transpose()?;
    let task = task_bytes
        .map(|file| config::parse_task(&file.bytes))
        .transpose()?;
    policy.task_contract_digest = task_bytes.map(|file| snapshot::digest(&file.bytes));
    let plan = Plan::build(&config, task.as_ref(), &options.profile)?;
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
    let workspace = if plan
        .commands
        .iter()
        .any(|check| check.kind == config::CheckKind::Command && check.compatibility.is_none())
        && invalid.is_empty()
    {
        Some(snapshot::materialize(&snapshot).await?)
    } else {
        None
    };
    let input_guard = if let Some(workspace) = &workspace {
        Some(snapshot::InputGuard::new(workspace.path(), snapshot.files.clone()).await?)
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
        if rule.is_some() {
            let entry = &catalog.entries[id];
            result.rule_version = entry.version();
            result
                .metadata
                .insert("rule_definition".into(), serde_json::json!(entry));
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
    match snapshot::capture(&options.root, &options.selection).await {
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
            invalid.push("Source snapshot changed during checks; recheck the final state".into())
        }
        Ok(_) => {}
        Err(error) => invalid.push(format!("Cannot revalidate the source snapshot: {error:#}")),
    }
    let recheck = recheck(&options, &snapshot.identity.base);
    for result in &mut results {
        for diagnostic in &mut result.diagnostics {
            diagnostic.recheck.argv = recheck.clone();
        }
    }
    let summary = Summary::from_checks(&results);
    let gate = evaluate(&results, &plan.required, &invalid);
    let report = Report {
        schema_version: 1,
        run_id,
        scope: if snapshot.path_filter.is_some() {
            "path"
        } else if task.is_some() {
            "task"
        } else {
            "repository"
        }
        .into(),
        profile: options.profile,
        snapshot: snapshot.identity.clone(),
        policy,
        plan: PlanSummary {
            execution_order: plan.order,
            required_checks: plan.required,
            pending_delivery_checks: plan.pending_delivery,
            acceptance: plan.acceptance,
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

fn recheck(options: &CheckOptions, base: &str) -> Vec<String> {
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
        "--config".into(),
        options.config.clone(),
        "--profile".into(),
        options.profile.clone(),
        "--format".into(),
        "json".into(),
    ]);
    if let Some(task) = &options.task {
        argv.extend(["--task".into(), task.clone()]);
    }
    if let Some(reference) = &options.policy_ref {
        argv.extend(["--policy-ref".into(), reference.clone()]);
    }
    if let Some(path) = &options.trust_store {
        argv.extend(["--trust-store".into(), path.display().to_string()]);
    }
    if let Some(path) = &options.evidence_dir {
        argv.extend(["--evidence-dir".into(), path.display().to_string()]);
    }
    argv
}
