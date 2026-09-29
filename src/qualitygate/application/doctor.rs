//! Snapshot-bound readiness, without delivery verdicts or reusable gate credentials.

use super::{CheckOptions, capabilities, doctor_context, doctor_plan, policy_active};
use crate::{
    domain::{NextStep, PolicyInputError, doctor::*, runtime::ConfigurationError},
    snapshot,
};
use anyhow::{Result, ensure};
use std::sync::Arc;

pub async fn invalid_request(profile: String, probe_tools: bool, message: &str) -> Report {
    let mut report = Report::new(profile, probe_tools);
    match capabilities::report().await {
        Ok(runtime) => report.runtime = Some(runtime),
        Err(error) => report.diagnostics.push(diagnostic(
            "runtime.identity_incomplete",
            &error,
            Vec::new(),
        )),
    }
    report.diagnostics.push(Diagnostic::new(
        "preflight.invalid_request",
        message,
        Vec::new(),
    ));
    report
}

pub async fn run(mut options: CheckOptions, probe_tools: bool) -> Report {
    let mut report = Report::new(options.profile.clone(), probe_tools);
    if let Err(error) = run_inner(&mut options, &mut report).await {
        report
            .diagnostics
            .push(diagnostic("preflight.incomplete", &error, Vec::new()));
        if error.downcast_ref::<PolicyInputError>().is_some()
            || error.downcast_ref::<ConfigurationError>().is_some()
        {
            let context = super::policy_guidance::Context {
                root: options.root.clone(),
                config: options.config.clone(),
                policy_ref: options.policy_ref.is_some(),
                selection: Some(match options.selection {
                    snapshot::Selection::Staged => super::policy_guidance::SelectionKind::Staged,
                    snapshot::Selection::Diff { .. } => super::policy_guidance::SelectionKind::Diff,
                    _ => super::policy_guidance::SelectionKind::Worktree,
                }),
            };
            match super::policy_guidance::next_steps(&error, context).await {
                Ok(steps) => report.next_steps.extend(steps),
                Err(error) => {
                    report
                        .diagnostics
                        .push(diagnostic("guidance.incomplete", &error, Vec::new()))
                }
            }
        }
    }
    report.not_executed.retain(|item| {
        !report.checks.iter().any(|check| {
            check.id == item.check
                && check
                    .probes
                    .iter()
                    .any(|probe| probe.attempted && item.operation == format!("tool:{}", probe.id))
        })
    });
    report.complete = report.diagnostics.is_empty();
    remediation(&mut report);
    let (action, command, message) = if report.complete {
        (
            "run_check",
            "check",
            "Requested preflight passed. Run the formal check to obtain delivery evidence; preflight is not a delivery verdict.",
        )
    } else {
        (
            "retry_doctor",
            "doctor",
            "Repair the reported prerequisites and rerun the same preflight. No delivery conclusion is available.",
        )
    };
    report.next_steps.push(NextStep {
        action: action.into(),
        command: Some(argv(&options, command, probe_tools)),
        message: message.into(),
    });
    if let Some(runtime) = &report.runtime {
        for step in &mut report.next_steps {
            if let Some(first) = step.command.as_mut().and_then(|argv| argv.first_mut()) {
                *first = runtime.executable.path.clone();
            }
        }
    }
    report
}

async fn run_inner(options: &mut CheckOptions, report: &mut Report) -> Result<()> {
    report.runtime = Some(capabilities::report().await?);
    ensure!(
        matches!(
            options.selection,
            snapshot::Selection::Worktree { .. }
                | snapshot::Selection::Staged
                | snapshot::Selection::Diff { .. }
        ),
        "doctor supports worktree, staged and diff snapshots only"
    );
    let root = options.root.clone();
    options.root = tokio::task::spawn_blocking(move || root.canonicalize()).await??;
    let (initial, active) = doctor_plan::bootstrap(options).await?;
    report.policy = Some(initial.evidence.clone());
    report.planned_checks = initial.plan.order.clone();
    report.pending_delivery_checks = initial.plan.pending_delivery.clone();
    for id in &initial.plan.order {
        report.not_executed.push(NotExecuted {
            check: id.clone(),
            operation: "check".into(),
            reason: "Formal checks and rule evaluation are outside preflight scope".into(),
        });
    }
    for command in &initial.plan.commands {
        for tool in &command.tools {
            report.not_executed.push(NotExecuted {
                check: command.id.clone(),
                operation: format!("tool:{}", tool.id),
                reason: if report.probe_tools {
                    "Preflight prerequisites or shared budget did not permit execution"
                } else {
                    "Tool probes require --probe-tools"
                }
                .into(),
            });
        }
    }
    let mut protected = initial.protected_paths;
    protected.extend(
        initial
            .plan
            .commands
            .iter()
            .flat_map(|command| &command.tools)
            .flat_map(|tool| &tool.inputs)
            .map(|path| globset::escape(path)),
    );
    report.budgets = snapshot::selected_preflight::inspect(
        &options.root,
        &options.selection,
        &options.snapshot_options,
        &protected,
        if active.is_none() {
            options.policy_ref.as_deref()
        } else {
            None
        },
    )
    .await?;
    budget_diagnostics(options, report);
    if !report.diagnostics.is_empty() {
        return Ok(());
    }
    let snapshot = Arc::new(
        snapshot::capture_with_options(
            &options.root,
            &options.selection,
            &options.snapshot_options,
        )
        .await?,
    );
    ensure!(
        report.budgets[0].reference == snapshot.identity.base
            && report.budgets[1].reference == snapshot.identity.head,
        "Snapshot references changed during preflight acquisition"
    );
    report.snapshot = Some(snapshot.identity.clone());
    report.selection = Some(snapshot.scope_evidence.clone());
    let mut invalid = Vec::new();
    let (loaded, active) =
        doctor_plan::load(&snapshot, options, active, true, &mut invalid).await?;
    let a = &initial.evidence;
    let b = &loaded.evidence;
    ensure!(
        a.config_digest == b.config_digest
            && a.rules_digest == b.rules_digest
            && a.task_contract_digest == b.task_contract_digest
            && a.resolved_commit == b.resolved_commit,
        "Policy or task changed during preflight acquisition"
    );
    report.policy = Some(loaded.evidence.clone());
    for error in invalid {
        report.diagnostics.push(Diagnostic::new(
            "policy.snapshot_mismatch",
            error,
            loaded.plan.order.clone(),
        ));
    }
    if report.diagnostics.is_empty() {
        let workspace = snapshot::materialize_shared(Arc::clone(&snapshot)).await?;
        let guard =
            snapshot::InputGuard::for_snapshot(workspace.path(), Arc::clone(&snapshot)).await?;
        doctor_context::inspect(&loaded.plan, workspace.path(), &snapshot, report).await;
        if report.probe_tools && report.diagnostics.is_empty() {
            match tokio::time::timeout(
                std::time::Duration::from_secs(PROBE_SECONDS),
                doctor_context::probe(&loaded.plan, workspace.path(), &snapshot, &guard, report),
            )
            .await
            {
                Ok(result) => result?,
                Err(_) => report.diagnostics.push(Diagnostic::new(
                    "probe.budget_exhausted",
                    "Tool probes exhausted the shared 120-second budget",
                    loaded.plan.order.clone(),
                )),
            }
        }
        if let Err(error) = guard.verify().await {
            report.diagnostics.push(diagnostic(
                "snapshot.inputs_changed",
                &error,
                loaded.plan.order.clone(),
            ));
        }
    }
    let current = snapshot::capture_with_options(
        &options.root,
        &options.selection,
        &options.snapshot_options,
    )
    .await?;
    if serde_json::to_value(&current.identity)? != serde_json::to_value(&snapshot.identity)? {
        report.diagnostics.push(Diagnostic::new(
            "snapshot.source_changed",
            "Source snapshot changed during preflight; rerun against the final state",
            loaded.plan.order.clone(),
        ));
    }
    // Re-resolve moving policy refs and reread selected policy assets at completion.
    let (current_policy, active) =
        doctor_plan::load(&Arc::new(current), options, active, true, &mut Vec::new()).await?;
    if serde_json::to_value(&current_policy.evidence)? != serde_json::to_value(&loaded.evidence)? {
        report.diagnostics.push(Diagnostic::new(
            "policy.changed",
            "Policy changed during preflight",
            loaded.plan.order,
        ));
    }
    if let Some(active) = active {
        policy_active::revalidate(options.root.clone(), active).await?;
    }
    Ok(())
}

fn budget_diagnostics(options: &CheckOptions, report: &mut Report) {
    let mut recommended = None::<u64>;
    for budget in &report.budgets {
        if budget.ready() {
            continue;
        }
        for (condition, code, message) in [
            (
                budget.oversized_file_count > 0,
                "snapshot.file_limit",
                format!(
                    "{}: {} files exceed the {} byte per-file budget; the supported maximum is 8 MiB",
                    budget.side, budget.oversized_file_count, budget.max_file_bytes
                ),
            ),
            (
                budget.file_count > budget.max_files,
                "snapshot.file_count",
                format!(
                    "{}: {} entries exceed the {} file inventory budget",
                    budget.side, budget.file_count, budget.max_files
                ),
            ),
            (
                budget.total_bytes > budget.max_bytes as u64,
                "snapshot.total_limit",
                format!(
                    "{}: {} bytes exceed the {} byte total budget",
                    budget.side, budget.total_bytes, budget.max_bytes
                ),
            ),
            (
                budget.unsupported_entry_count > 0,
                "snapshot.unsupported_entry",
                format!(
                    "{}: {} unsupported or unreadable entries",
                    budget.side, budget.unsupported_entry_count
                ),
            ),
            (
                budget.protected_exclusion_count > 0,
                "snapshot.protected_excluded",
                format!(
                    "{}: {} excluded protected verification inputs",
                    budget.side, budget.protected_exclusion_count
                ),
            ),
        ] {
            if condition {
                report.diagnostics.push(Diagnostic::new(
                    code,
                    message,
                    report.planned_checks.clone(),
                ));
            }
        }
        if let Some(mib) = budget.recommended_max_file_mib {
            recommended = Some(recommended.unwrap_or(0).max(mib));
        }
    }
    let above_capacity = report
        .budgets
        .iter()
        .any(|budget| budget.oversized_file_count > 0 && budget.recommended_max_file_mib.is_none());
    if let Some(bytes) = report
        .budgets
        .iter()
        .filter(|budget| budget.total_bytes > budget.max_bytes as u64)
        .map(|budget| budget.total_bytes)
        .max()
        && bytes <= 1024 * 1024 * 1024
    {
        let mut adjusted = options.clone();
        adjusted.snapshot_options.max_bytes = bytes.div_ceil(1024 * 1024) as usize * 1024 * 1024;
        report.next_steps.push(NextStep { action: "increase_total_budget".into(), command: Some(argv(&adjusted, "doctor", report.probe_tools)),
            message: "Increase the total acquisition budget if these inputs belong to the requested checks; per-file and file-count limits still apply.".into() });
    }
    if let Some(mib) = recommended.filter(|_| !above_capacity) {
        let mut adjusted = options.clone();
        adjusted.snapshot_options.max_file_bytes = mib as usize * 1024 * 1024;
        report.next_steps.push(NextStep { action: "increase_file_budget".into(), command: Some(argv(&adjusted, "doctor", report.probe_tools)),
            message: format!("The smallest sufficient per-file budget is {mib} MiB. Increasing the total budget does not change the per-file limit.") });
    }
    if report.budgets.iter().any(|budget| {
        budget.oversized_file_count > 0 || budget.total_bytes > budget.max_bytes as u64
    }) {
        report.next_steps.push(NextStep { action: "review_exclusions".into(), command: None,
            message: if above_capacity { "Files exceed the supported 8 MiB single-file capacity. Review explicit exclusions for unrelated resources in the selected policy; protected inputs cannot be excluded." }
                else { "Review explicit exclusions for unrelated resources in the selected policy. Excluded resources are unavailable to builds and checks; protected inputs cannot be excluded." }.into() });
    }
}

fn remediation(report: &mut Report) {
    let missing: std::collections::BTreeSet<_> = report
        .checks
        .iter()
        .flat_map(|check| &check.required_env)
        .filter(|variable| !variable.present)
        .map(|variable| variable.name.as_str())
        .collect();
    if !missing.is_empty() {
        report.next_steps.push(NextStep { action: "provide_environment".into(), command: None,
            message: format!("Set nonempty values for {} in the calling process using the project's documented setup, then rerun doctor. Do not print the values.", missing.into_iter().collect::<Vec<_>>().join(", ")) });
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.ends_with("executable_missing"))
    {
        report.next_steps.push(NextStep { action: "provide_executable".into(), command: None,
            message: "Make the reported executable available on PATH or at its declared path in the selected snapshot working directory. Follow the project's setup instructions; no tool is installed automatically.".into() });
    }
}

pub(super) fn diagnostic(code: &str, error: &anyhow::Error, checks: Vec<String>) -> Diagnostic {
    if let Some(error) = error.downcast_ref::<ConfigurationError>() {
        Diagnostic {
            code: error.code.clone(),
            message: error.message.clone(),
            checks,
            field: error.field.clone(),
            line: error.line,
            column: error.column,
        }
    } else {
        Diagnostic::new(
            if error.downcast_ref::<PolicyInputError>().is_some() {
                "config.missing"
            } else {
                code
            },
            format!("{error:#}"),
            checks,
        )
    }
}

fn argv(options: &CheckOptions, command: &str, probe_tools: bool) -> Vec<String> {
    let mut argv = vec![
        "qualitygate".into(),
        "--root".into(),
        options.root.display().to_string(),
        "--config".into(),
        options.config.clone(),
        command.into(),
    ];
    match &options.selection {
        snapshot::Selection::Worktree { base } => {
            argv.extend(["--worktree".into(), "--base".into(), base.clone()])
        }
        snapshot::Selection::Staged => argv.push("--staged".into()),
        snapshot::Selection::Diff { base, head } => {
            argv.extend(["--diff".into(), format!("{base}..{head}")])
        }
        _ => return Vec::new(),
    }
    argv.extend([
        "--profile".into(),
        options.profile.clone(),
        "--format".into(),
        "json".into(),
        "--snapshot-max-file-mib".into(),
        options
            .snapshot_options
            .max_file_bytes
            .div_ceil(1024 * 1024)
            .to_string(),
        "--snapshot-max-mib".into(),
        options
            .snapshot_options
            .max_bytes
            .div_ceil(1024 * 1024)
            .to_string(),
        "--snapshot-jobs".into(),
        options.snapshot_options.jobs.to_string(),
        "--snapshot-timeout-secs".into(),
        options.snapshot_options.timeout.as_secs().to_string(),
    ]);
    if let Some(task) = &options.task {
        argv.extend(["--task".into(), task.clone()]);
    }
    if let Some(reference) = &options.policy_ref {
        argv.extend(["--policy-ref".into(), reference.clone()]);
    }
    if command == "doctor" && probe_tools {
        argv.push("--probe-tools".into());
    }
    argv
}
