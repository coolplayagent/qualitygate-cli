use super::*;
use crate::domain::{Applicability, Decision, ExecutionStatus, Report, Verdict, evaluate};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct DiagnosticObservation {
    pub key: String,
    pub issue: Option<String>,
    pub attribution: Option<Attribution>,
    pub review: Option<Review>,
}
pub(super) struct Row<'a> {
    pub assignment: &'a Assignment,
    pub observation: Option<&'a Observation>,
    pub accepted: Option<bool>,
    pub first: Option<bool>,
    pub any_success: Option<bool>,
    pub completed: usize,
    pub required: usize,
    pub check_unknown: usize,
    pub diagnostics: Vec<DiagnosticObservation>,
    pub gaps: Vec<String>,
    pub value: Value,
}

fn bound(a: &Assignment, attempt: &Attempt, report: &Report) -> Result<()> {
    let required: BTreeSet<_> = a.required_checks.iter().collect();
    let actual: BTreeSet<_> = report.plan.required_checks.iter().collect();
    if report.schema_version != 1
        || report.run_id.is_empty()
        || report.scope != "task"
        || report.snapshot.base != a.base
        || report.snapshot.content_digest != attempt.snapshot_digest
        || report.policy.config_digest != a.config_digest
        || report.policy.task_contract_digest.as_deref() != Some(a.task_digest.as_str())
        || report.plan.task_id.as_deref() != Some(a.task_id.as_str())
        || report.profile != attempt.profile
        || report.environment_digest != a.cohort.environment_digest
        || tool_inventory_digest(report) != a.cohort.tools_digest
        || required != actual
        || actual.len() != report.plan.required_checks.len()
    {
        bail!(
            "Full report differs from assigned task, policy, environment, base, snapshot or check inventory"
        );
    }
    let recomputed = evaluate(&report.checks, &report.plan.required_checks, &[]);
    // A report may carry additional acquisition/policy failures. It cannot claim a
    // complete/pass result when the independent result-state evaluation disagrees.
    if report.gate.complete && (!recomputed.complete || recomputed.decision != report.gate.decision)
        || report.gate.complete != (report.gate.decision != Decision::Incomplete)
    {
        bail!("Report gate conflicts with retained check states");
    }
    Ok(())
}

pub(super) fn row<'a>(
    a: &'a Assignment,
    o: Option<&'a Observation>,
    reports: &Reports,
    p: &Protocol,
) -> Result<Row<'a>> {
    let mut row = Row {
        assignment: a,
        observation: o,
        accepted: None,
        first: None,
        any_success: None,
        completed: 0,
        required: 0,
        check_unknown: 0,
        diagnostics: Vec::new(),
        gaps: Vec::new(),
        value: Value::Null,
    };
    let mut states = Vec::new();
    let mut executions = Vec::new();
    let mut used_findings = BTreeSet::new();
    if let Some(o) = o {
        let mut elapsed = 0;
        for attempt in &o.attempts {
            elapsed += attempt.elapsed_ms;
            row.required += a.required_checks.len();
            let report = attempt.report.as_ref().map(|artifact| {
                reports
                    .get(&artifact.digest)
                    .ok_or_else(|| "Report was not loaded".to_owned())
                    .and_then(|v| v.as_ref().map_err(Clone::clone))
                    .and_then(|report| {
                        bound(a, attempt, report)
                            .map(|()| report)
                            .map_err(|e| e.to_string())
                    })
            });
            let within_budget = attempt.number <= p.max_attempts && elapsed <= p.max_seconds * 1000;
            let state = match report {
                Some(Ok(report)) => {
                    executions.push(json!({"attempt":attempt.number,"status":attempt.status,"within_budget":within_budget,
                        "gate":report.gate,"checks":report.checks.iter().filter(|c|a.required_checks.contains(&c.id)).map(|c|
                            json!({"id":c.id,"applicability":c.applicability,"status":c.execution.status,"verdict":c.verdict,"reason":c.execution.reason})).collect::<Vec<_>>()}));
                    for check in &report.checks {
                        if a.required_checks.contains(&check.id)
                            && check.applicability == Applicability::Applicable
                            && check.execution.status == ExecutionStatus::Completed
                            && matches!(check.verdict, Some(Verdict::Pass | Verdict::Fail))
                        {
                            row.completed += 1;
                        }
                        for diagnostic in &check.diagnostics {
                            let matching: Vec<_> = o
                                .findings
                                .iter()
                                .enumerate()
                                .filter(|(_, f)| {
                                    f.report_digest
                                        == attempt.report.as_ref().expect("loaded report").digest
                                        && f.check_id == check.id
                                        && f.fingerprint == diagnostic.fingerprint
                                })
                                .collect();
                            if matching.len() > 1 {
                                bail!("Multiple reviews for one retained diagnostic");
                            }
                            let (key, issue, attribution, review) = if let Some((i, f)) =
                                matching.first()
                            {
                                used_findings.insert(*i);
                                (
                                    format!("{}:{}", a.input_id, f.diagnostic_id),
                                    f.issue_id.as_ref().map(|id| format!("{}:{id}", a.input_id)),
                                    Some(f.attribution.clone()),
                                    f.review.clone(),
                                )
                            } else {
                                (
                                    format!(
                                        "{}:{}:{}",
                                        a.input_id, check.id, diagnostic.fingerprint
                                    ),
                                    None,
                                    None,
                                    None,
                                )
                            };
                            row.diagnostics.push(DiagnosticObservation {
                                key,
                                issue,
                                attribution,
                                review,
                            });
                        }
                    }
                    Some(
                        attempt.status == AttemptStatus::Completed
                            && within_budget
                            && attempt.profile == "full"
                            && report.gate.decision == Decision::Pass
                            && report.plan.pending_delivery_checks.is_empty()
                            && (!a.eligible_repair
                                || attempt.snapshot_digest != a.initial_snapshot),
                    )
                }
                Some(Err(error)) => {
                    row.gaps
                        .push(format!("Attempt {}: {error}", attempt.number));
                    row.check_unknown += a.required_checks.len();
                    None
                }
                None if matches!(
                    attempt.status,
                    AttemptStatus::Failed | AttemptStatus::TimedOut | AttemptStatus::Abandoned
                ) =>
                {
                    Some(false)
                }
                None => {
                    row.gaps
                        .push(format!("Attempt {} lacks a full report", attempt.number));
                    row.check_unknown += a.required_checks.len();
                    None
                }
            };
            states.push(state);
        }
        if used_findings.len() != o.findings.len() {
            bail!("Review references a missing, unbound or nonexistent report diagnostic");
        }
        row.any_success = if states.contains(&Some(true)) {
            Some(true)
        } else if states.is_empty() || states.contains(&None) {
            None
        } else {
            Some(false)
        };
        row.first = states.first().copied().flatten();
        row.accepted = states.last().copied().flatten();
        // An incomplete early attempt cannot be hidden by a later successful one.
        if !row.gaps.is_empty() {
            row.accepted = None;
        }
        if o.attempts.is_empty() {
            row.gaps.push("Assigned run has no attempts".into());
        }
    } else {
        row.gaps.push("Assigned run was not observed".into());
    }
    if row.required == 0 {
        row.required = a.required_checks.len();
        row.check_unknown = row.required;
    }
    if a.exclusion.is_some() {
        row.accepted = Some(false);
        row.first = Some(false);
        row.any_success = Some(false);
    }
    row.value = json!({"assignment_id":a.id,"task_id":a.task_id,"input_id":a.input_id,"exclusion":a.exclusion,
        "observed":o.is_some(),"accepted_within_budget":row.accepted,"first_attempt_success":row.first,
        "attempt_results":states,"executions":executions,"at_least_once_within_budget":row.any_success,"completed_checks":row.completed,"planned_checks":row.required,
        "unknown_checks":row.check_unknown,"gaps":row.gaps});
    Ok(row)
}

pub(super) fn diagnostics(rows: &[Row<'_>]) -> Result<BTreeMap<String, DiagnosticObservation>> {
    let mut map = BTreeMap::new();
    for d in rows.iter().flat_map(|r| &r.diagnostics) {
        if map
            .insert(d.key.clone(), d.clone())
            .is_some_and(|old| old != *d)
        {
            bail!("Conflicting canonical diagnostic review, issue or attribution");
        }
    }
    Ok(map)
}
