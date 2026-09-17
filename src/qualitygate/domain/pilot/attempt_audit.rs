use super::rows::bound_report;
use super::*;
use crate::domain::{Decision, Report, Severity, evaluate};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Audit {
    pub value: Value,
    pub evidence_complete: bool,
    pub compliant: bool,
}

fn baseline<'a>(assignment: &Assignment, reports: &'a Reports) -> Result<&'a Report> {
    let artifact = assignment
        .initial_report
        .as_ref()
        .context("Pilot initial report is missing")?;
    let report = reports
        .get(&artifact.digest)
        .context("Pilot initial report was not loaded")?
        .as_ref()
        .map_err(|error| anyhow::anyhow!("Pilot initial report: {error}"))?;
    bound_report(assignment, &assignment.initial_snapshot, "full", report)?;
    if !report.gate.complete
        || report.gate.decision == Decision::Incomplete
        || !report.plan.pending_delivery_checks.is_empty()
        || !canonical_blockers(report)
    {
        bail!("Pilot initial report requires a complete full gate");
    }
    Ok(report)
}

/// A plan's initial reports are checked before a CLI seal or start subject is emitted.
pub fn verify_initial_baselines(manifest: &Manifest, reports: &Reports) -> Result<()> {
    validate(manifest)?;
    if manifest.schema_version >= 6 {
        for assignment in &manifest.assignments {
            baseline(assignment, reports)?;
        }
    }
    Ok(())
}

fn debt(report: &Report) -> BTreeSet<String> {
    report
        .checks
        .iter()
        .filter(|check| check.severity == Severity::Error)
        .flat_map(|check| {
            check
                .diagnostics
                .iter()
                .map(move |diagnostic| format!("{}:{}", check.id, diagnostic.fingerprint))
        })
        .chain(
            report
                .gate
                .blockers
                .iter()
                .map(|blocker| format!("blocker:{blocker}")),
        )
        .collect()
}

fn canonical_blockers(report: &Report) -> bool {
    report.gate.blockers == evaluate(&report.checks, &report.plan.required_checks, &[]).blockers
}

fn attempt_report<'a>(
    assignment: &Assignment,
    attempt: &Attempt,
    reports: &'a Reports,
) -> Option<&'a Report> {
    let artifact = attempt.report.as_ref()?;
    let report = reports.get(&artifact.digest)?.as_ref().ok()?;
    bound_report(
        assignment,
        &attempt.snapshot_digest,
        &attempt.profile,
        report,
    )
    .ok()?;
    (report.gate.complete
        && report.gate.decision != Decision::Incomplete
        && report.plan.pending_delivery_checks.is_empty()
        && canonical_blockers(report)
        && attempt.profile == "full")
        .then_some(report)
}

pub(super) fn audit(manifest: &Manifest, reports: &Reports) -> Audit {
    let observations: BTreeMap<_, _> = manifest
        .observations
        .iter()
        .map(|observation| (observation.assignment_id.as_str(), observation))
        .collect();
    let mut assignments = Vec::new();
    let mut complete = true;
    let mut compliant = true;
    let limit = manifest
        .protocol
        .no_progress_limit
        .expect("validated v6 plan");
    for assignment in &manifest.assignments {
        let initial = baseline(assignment, reports).ok();
        let mut previous = initial.map(debt);
        let mut evidence_complete = initial.is_some();
        let mut deviations = Vec::<Value>::new();
        let mut attempts = Vec::new();
        let mut elapsed_ms = 0_u64;
        let mut stagnant = 0_u16;
        let mut stop_after = None;
        let observation = observations.get(assignment.id.as_str());
        if observation.is_none_or(|value| value.attempts.is_empty()) {
            evidence_complete = false;
        }
        if let Some(observation) = observation {
            for attempt in &observation.attempts {
                elapsed_ms += attempt.elapsed_ms;
                if stop_after.is_some() {
                    deviations.push(
                        json!({"attempt":attempt.number,"reason":"continued_after_no_progress"}),
                    );
                }
                let within_budget = attempt.number <= manifest.protocol.max_attempts
                    && elapsed_ms <= manifest.protocol.max_seconds * 1000;
                if !within_budget {
                    deviations.push(json!({"attempt":attempt.number,"reason":"attempt_or_time_budget_exceeded"}));
                }
                let before = previous.as_ref().map(BTreeSet::len);
                let report = attempt_report(assignment, attempt, reports);
                let after = report.map(debt);
                let progress = previous
                    .as_ref()
                    .zip(after.as_ref())
                    .is_some_and(|(old, new)| new.len() < old.len() && new.is_subset(old));
                if let Some(after) = &after {
                    previous = Some(after.clone());
                } else if attempt.report.is_some()
                    || !matches!(
                        attempt.status,
                        AttemptStatus::Failed | AttemptStatus::TimedOut | AttemptStatus::Abandoned
                    )
                {
                    evidence_complete = false;
                }
                stagnant = if progress { 0 } else { stagnant + 1 };
                if stagnant >= limit && stop_after.is_none() {
                    stop_after = Some(attempt.number);
                }
                attempts.push(json!({
                    "number":attempt.number,"report_digest":attempt.report.as_ref().map(|a|&a.digest),
                    "verified_report":report.is_some(),"debt_before":before,
                    "debt_after":after.as_ref().map(BTreeSet::len),"verified_progress":progress,
                    "consecutive_no_progress":stagnant,"within_budget":within_budget
                }));
            }
        }
        let status = if !deviations.is_empty() {
            compliant = false;
            "deviated"
        } else if !evidence_complete {
            "incomplete"
        } else {
            "matched"
        };
        complete &= evidence_complete;
        assignments.push(json!({"assignment_id":assignment.id,
            "initial_report_digest":assignment.initial_report.as_ref().map(|a|&a.digest),
            "baseline_verified":initial.is_some(),"status":status,"attempts":attempts,
            "elapsed_ms":elapsed_ms,"stop_after_attempt":stop_after,"deviations":deviations}));
    }
    let status = if !compliant {
        "deviated"
    } else if !complete {
        "incomplete"
    } else {
        "matched"
    };
    Audit {
        value: json!({"status":status,"no_progress_limit":limit,
            "max_attempts":manifest.protocol.max_attempts,
            "max_seconds":manifest.protocol.max_seconds,"assignments":assignments}),
        evidence_complete: complete,
        compliant,
    }
}
