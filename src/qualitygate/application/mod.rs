//! Coordinates policy selection, snapshot execution and evidence-bound reports.

mod commands;
mod coverage_gate;
mod generated_reports;
mod policy;
mod report_gate;
mod test_counts;

use crate::{
    config::{self, Plan},
    domain::*,
    snapshot::{self, Selection},
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
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
    let directory = crate::paths::run_directory(options.output_dir.as_deref())?;
    let run_id = directory
        .file_name()
        .context("Missing run directory name")?
        .to_string_lossy()
        .to_string();
    let rule_snapshot = Arc::clone(&snapshot);
    let rule_settings = plan.rules.clone();
    let mut results = tokio::task::spawn_blocking(move || {
        rule_settings
            .iter()
            .map(|(id, setting)| {
                let entry = &catalog.entries[id];
                let mut result = if let Some(rule) = &entry.custom {
                    crate::adapters::custom_rules::evaluate(rule, setting, &rule_snapshot)
                } else {
                    let builtin = entry.builtin.as_ref().expect("resolved builtin");
                    crate::adapters::rules::evaluate_as(
                        id,
                        &builtin.implementation,
                        setting,
                        &rule_snapshot,
                    )
                };
                result.rule_version = entry.version();
                result
                    .metadata
                    .insert("rule_definition".into(), serde_json::json!(entry));
                result.metadata.insert(
                    "adapter_version".into(),
                    serde_json::json!(env!("CARGO_PKG_VERSION")),
                );
                let sources = setting
                    .source
                    .iter()
                    .chain(entry.custom.iter().map(|rule| &rule.source));
                for source in sources {
                    if let Err(error) = policy::validate_source(source, &rule_snapshot.files) {
                        result.block(
                            ExecutionStatus::Blocked,
                            format!("Rule source requires review: {error:#}"),
                        );
                    }
                }
                result
            })
            .collect::<Vec<_>>()
    })
    .await?;
    let workspace = if !plan.commands.is_empty() && invalid.is_empty() {
        Some(snapshot::materialize(&snapshot).await?)
    } else {
        None
    };
    for command in &plan.commands {
        let dependency_failure = command.depends_on.iter().find(|id| {
            !results.iter().any(|result| {
                &result.id == *id
                    && result.execution.status == ExecutionStatus::Completed
                    && result.verdict == Some(Verdict::Pass)
            })
        });
        let result = if let Some(dependency) = dependency_failure {
            let mut result = CheckResult::pending(&command.id, command.required, command.severity);
            result.block(
                ExecutionStatus::Blocked,
                format!("Prerequisite {dependency} did not pass"),
            );
            result
        } else if let Some(workspace) = &workspace {
            commands::execute(command, workspace.path(), &directory, &snapshot).await
        } else {
            let mut result = CheckResult::pending(&command.id, command.required, command.severity);
            result.block(
                ExecutionStatus::Blocked,
                "Policy validation did not complete",
            );
            result
        };
        results.push(result);
    }
    if let Some(workspace) = &workspace {
        for (name, file) in &snapshot.files {
            match crate::paths::confined(workspace.path(), Path::new(name))
                .and_then(|path| std::fs::read(path).map_err(Into::into))
            {
                Ok(bytes) if bytes == file.bytes => {}
                Ok(_) => invalid.push(format!("Checked input modified during execution: {name}")),
                Err(_) => invalid.push(format!(
                    "Checked input removed or replaced during execution: {name}"
                )),
            }
        }
    }
    let current = snapshot::capture(&options.root, &options.selection).await?;
    if current.identity.content_digest != snapshot.identity.content_digest
        || current.identity.base != snapshot.identity.base
        || current.identity.head != snapshot.identity.head
    {
        invalid.push("Source snapshot changed during checks; recheck the final state".into());
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
    argv
}
