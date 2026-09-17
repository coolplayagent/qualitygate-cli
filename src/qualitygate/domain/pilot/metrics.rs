use super::rows::{Row, diagnostics, row};
use super::*;
use crate::domain::Report;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub type Reports = BTreeMap<String, std::result::Result<Report, String>>;

pub(super) fn rate(n: usize, d: usize, unknown: usize) -> Value {
    json!({"numerator":n,"denominator":d,"unknown":unknown,
        "value":(d>0 && unknown==0).then(||n as f64/d as f64)})
}
pub(super) fn distribution(mut values: Vec<u64>, unknown: usize) -> Value {
    values.sort_unstable();
    let n = values.len();
    let median = if n == 0 || unknown > 0 {
        None
    } else if n % 2 == 1 {
        Some(values[n / 2] as f64)
    } else {
        Some((values[n / 2 - 1] as f64 + values[n / 2] as f64) / 2.0)
    };
    json!({"samples":n,"unknown":unknown,"median":median,
        "p95":(n>0 && unknown==0).then(||values[(95*n).div_ceil(100)-1])})
}
fn success(rows: &[&Row<'_>], select: fn(&Row<'_>) -> Option<bool>) -> Value {
    let states: Vec<_> = rows.iter().map(|r| select(r)).collect();
    rate(
        states.iter().filter(|v| **v == Some(true)).count(),
        states.len(),
        states.iter().filter(|v| v.is_none()).count(),
    )
}
fn usage(rows: &[Row<'_>]) -> Value {
    let attempts: Vec<_> = rows
        .iter()
        .flat_map(|r| r.observation.into_iter().flat_map(|o| &o.attempts))
        .collect();
    let missing = attempts.iter().filter(|a| a.usage.is_none()).count()
        + rows
            .iter()
            .filter(|r| r.observation.is_none_or(|o| o.attempts.is_empty()))
            .count();
    let sum = |select: fn(&Usage) -> Option<u64>| {
        let values: Vec<_> = attempts
            .iter()
            .filter_map(|a| a.usage.as_ref().and_then(select))
            .collect();
        let unknown = attempts.len() - values.len()
            + rows
                .iter()
                .filter(|r| r.observation.is_none_or(|o| o.attempts.is_empty()))
                .count();
        json!({"known":values.iter().sum::<u64>(),"unknown_attempts":unknown,"total":(unknown==0 && !attempts.is_empty()).then(||values.iter().sum::<u64>())})
    };
    json!({"missing_attempts":missing,"input_tokens":sum(|u|Some(u.input_tokens)),"output_tokens":sum(|u|Some(u.output_tokens)),
        "cached_input_tokens":sum(|u|u.cached_input_tokens),"reasoning_output_tokens":sum(|u|u.reasoning_output_tokens),
        "cache_write_input_tokens":sum(|u|u.cache_write_input_tokens),"note":"Provider-reported components; cached/reasoning counts are not added again to totals and tokens are not monetary cost"})
}
fn cost(rows: &[Row<'_>]) -> Value {
    let mut currencies = BTreeSet::new();
    let mut prices = BTreeSet::new();
    let (mut model, mut infra, mut unknown) = (0_u64, 0_u64, 0_usize);
    let mut sources = BTreeSet::new();
    for row in rows {
        if row.observation.is_none_or(|o| o.attempts.is_empty()) {
            unknown += 1;
        }
        for attempt in row.observation.into_iter().flat_map(|o| &o.attempts) {
            if let Some(c) = &attempt.cost {
                currencies.insert(c.currency.as_str());
                prices.insert(c.priced_at);
                sources.insert(c.source.as_str());
                model += c.model_micros.unwrap_or(0);
                infra += c.infrastructure_micros.unwrap_or(0);
                if c.model_micros.is_none() || c.infrastructure_micros.is_none() {
                    unknown += 1;
                }
            } else {
                unknown += 1;
            }
        }
    }
    let total =
        (unknown == 0 && currencies.len() == 1 && prices.len() == 1).then_some(model + infra);
    let accepted = rows.iter().filter(|r| r.accepted == Some(true)).count();
    json!({"currencies":currencies,"priced_at":prices,"sources":sources,
        "known_model_micros":model,"known_infrastructure_micros":infra,"unknown_attempts":unknown,
        "total_micros":total,"per_accepted_task_micros":total.filter(|_|accepted>0).map(|v|v as f64/accepted as f64)})
}
fn group(rows: &[Row<'_>], protocol: &Protocol) -> Result<Value> {
    let first = rows[0].assignment;
    let all: Vec<_> = rows.iter().collect();
    let eligible: Vec<_> = rows
        .iter()
        .filter(|r| r.assignment.eligible_repair)
        .collect();
    let issues: BTreeSet<_> = rows
        .iter()
        .flat_map(|r| {
            r.assignment
                .expected_issues
                .iter()
                .flatten()
                .map(|id| format!("{}:{id}", r.assignment.input_id))
        })
        .collect();
    let truth_unknown = rows
        .iter()
        .filter(|r| r.assignment.expected_issues.is_none() || !r.gaps.is_empty())
        .count();
    let ds = diagnostics(rows)?;
    let confirmed: BTreeSet<_> = ds
        .values()
        .filter(|d| {
            d.review
                .as_ref()
                .is_some_and(|r| r.label == Label::Confirmed)
        })
        .filter_map(|d| d.issue.as_ref())
        .collect();
    let reviewed = ds.values().filter(|d| d.review.is_some()).count();
    let false_positive = ds
        .values()
        .filter(|d| {
            d.review
                .as_ref()
                .is_some_and(|r| r.label == Label::FalsePositive)
        })
        .count();
    let mut by_origin = BTreeMap::<String, usize>::new();
    for d in ds.values() {
        *by_origin
            .entry(
                d.attribution
                    .as_ref()
                    .map(|a| {
                        serde_json::to_string(a)
                            .expect("enum")
                            .trim_matches('"')
                            .to_owned()
                    })
                    .unwrap_or_else(|| "unclassified".into()),
            )
            .or_default() += 1;
    }
    let mut repetitions = BTreeMap::<&str, Vec<&Row<'_>>>::new();
    for row in rows {
        repetitions
            .entry(&row.assignment.input_id)
            .or_default()
            .push(row);
    }
    let repeated: Vec<_> = repetitions.values().filter(|r| r.len() > 1).collect();
    let stable = rate(
        repeated
            .iter()
            .filter(|rs| rs.iter().all(|r| r.accepted == Some(true)))
            .count(),
        repeated.len(),
        repeated
            .iter()
            .filter(|rs| rs.iter().any(|r| r.accepted.is_none()))
            .count(),
    );
    let times = |profile: &str| {
        let attempts: Vec<_> = rows
            .iter()
            .filter(|r| r.assignment.exclusion.is_none())
            .flat_map(|r| r.observation.into_iter().flat_map(|o| &o.attempts))
            .filter(|a| a.profile == profile)
            .collect();
        distribution(
            attempts.iter().filter_map(|a| a.check_elapsed_ms).collect(),
            attempts
                .iter()
                .filter(|a| a.check_elapsed_ms.is_none())
                .count()
                + rows
                    .iter()
                    .filter(|r| r.observation.is_none_or(|o| o.attempts.is_empty()))
                    .count(),
        )
    };
    let human = |field: fn(&Observation) -> Option<u64>| {
        distribution(
            rows.iter()
                .filter_map(|r| r.observation.and_then(field))
                .collect(),
            rows.iter()
                .filter(|r| r.observation.and_then(field).is_none())
                .count(),
        )
    };
    let detection = rate(
        issues.iter().filter(|id| confirmed.contains(id)).count(),
        issues.len(),
        truth_unknown + ds.len() - reviewed,
    );
    let fp = rate(false_positive, reviewed, 0);
    let review_fraction = rate(reviewed, ds.len(), 0);
    let repair = success(&eligible, |r| r.accepted);
    let completion = rate(
        rows.iter().map(|r| r.completed).sum(),
        rows.iter().map(|r| r.required).sum(),
        rows.iter().map(|r| r.check_unknown).sum(),
    );
    let threshold = |metric: &Value, threshold: f64, minimum: bool| {
        metric["value"].as_f64().map(|v| {
            if minimum {
                v >= threshold
            } else {
                v <= threshold
            }
        })
    };
    let review_sufficient = review_fraction["value"]
        .as_f64()
        .is_some_and(|v| v >= protocol.review_fraction_min);
    let mut statuses = BTreeMap::<String, usize>::new();
    for a in rows
        .iter()
        .flat_map(|r| r.observation.into_iter().flat_map(|o| &o.attempts))
    {
        *statuses
            .entry(serde_json::to_string(&a.status)?.trim_matches('"').into())
            .or_default() += 1;
    }
    Ok(
        json!({"cohort":first.cohort,"origin":first.origin,"task_kind":first.task_kind,
        "assigned":rows.len(),"observed":rows.iter().filter(|r|r.observation.is_some()).count(),
        "excluded":rows.iter().filter(|r|r.assignment.exclusion.is_some()).count(),"attempt_statuses":statuses,
        "accepted":rows.iter().filter(|r|r.accepted==Some(true)).count(),
        "detection":detection,"false_positive":fp,"review_fraction":review_fraction,
        "diagnostics":{"distinct":ds.len(),"reviewed":reviewed,"unreviewed":ds.len()-reviewed,"by_attribution":by_origin},
        "repair":repair,"first_attempt_success":success(&all,|r|r.first),"within_budget_success":success(&all,|r|r.any_success),
        "repeat_stability":stable,"required_check_completion":completion,
        "quick_ms":times("quick"),"full_ms":times("full"),"review_active_ms":human(|o|o.review_active_ms),
        "review_comments":human(|o|o.review_comments),"rework_rounds":human(|o|o.rework_rounds),"cost":cost(rows),"usage":usage(rows),
        "threshold_observations":{"detection":threshold(&detection,protocol.thresholds.detection_min,true),
            "false_positive":if review_sufficient {threshold(&fp,protocol.thresholds.false_positive_max,false)}else{None},
            "repair":threshold(&repair,protocol.thresholds.repair_min,true),
            "completion":threshold(&completion,protocol.thresholds.completion_min,true)},
        "assignments":rows.iter().map(|r|&r.value).collect::<Vec<_>>()}),
    )
}

/// All calculations are pure. `now` is provided by the application, never read here.
pub fn summarize(manifest: &Manifest, reports: &Reports, now: u64) -> Result<Value> {
    summarize_with_authorization(manifest, reports, now, None)
}

/// A verified authorization is supplied only after an adapter authenticates
/// its DSSE envelope against a caller-controlled external trust store.
pub fn summarize_with_authorization(
    manifest: &Manifest,
    reports: &Reports,
    now: u64,
    authorization: Option<&VerifiedPlanAuthorization>,
) -> Result<Value> {
    validate(manifest)?;
    if let Some(value) = authorization
        && value.record.subject != authorization_subject(manifest)?
    {
        anyhow::bail!("Verified pilot authorization does not match the summarized plan");
    }
    let observed: BTreeMap<_, _> = manifest
        .observations
        .iter()
        .map(|o| (o.assignment_id.as_str(), o))
        .collect();
    let mut buckets = BTreeMap::new();
    for a in &manifest.assignments {
        buckets
            .entry((&a.origin, &a.task_kind, &a.cohort))
            .or_insert_with(Vec::new)
            .push(row(
                a,
                observed.get(a.id.as_str()).copied(),
                reports,
                &manifest.protocol,
            )?);
    }
    let attempt_audit =
        (manifest.schema_version >= 6).then(|| super::attempt_audit::audit(manifest, reports));
    let model_missing = manifest.schema_version >= 7
        && manifest
            .observations
            .iter()
            .any(|observation| observation.model_evidence.is_none());
    let complete = !manifest.assignments.is_empty()
        && buckets.values().flatten().all(|r| r.gaps.is_empty())
        && !model_missing
        && attempt_audit
            .as_ref()
            .is_none_or(|audit| audit.evidence_complete);
    let groups: Vec<_> = buckets
        .values()
        .map(|rows| group(rows, &manifest.protocol))
        .collect::<Result<_>>()?;
    let comparisons =
        super::comparison::compare(&groups, &manifest.assignments, &manifest.protocol);
    let p = &manifest.protocol;
    let mut limits = Vec::new();
    if manifest.plan_seal.is_none() {
        limits.push("The pre-observation pilot plan has no digest-verified integrity seal");
    }
    if authorization.is_none() {
        limits.push("Pilot start has no authenticated owner authorization");
    }
    if [&p.owner, &p.reviewer, &p.archive]
        .iter()
        .any(|v| v.is_none())
        || (manifest.schema_version == 1 && p.monetary_cap.is_none())
        || (manifest.schema_version >= 2 && p.budget.is_none())
    {
        limits.push("Owner, reviewer, durable archive or monetary budget is undeclared");
    }
    if p.sealed_at
        .zip(p.start_at)
        .zip(p.end_at)
        .is_none_or(|((sealed, start), end)| {
            sealed > start || end > now || end - start < u64::from(p.days) * 86400
        })
    {
        limits.push("The sealed observation period has not been demonstrated");
    }
    if manifest
        .assignments
        .iter()
        .map(|a| &a.input_id)
        .collect::<BTreeSet<_>>()
        .len()
        != p.task_count
    {
        limits.push("Assigned distinct tasks differ from the declared sample size");
    }
    if manifest.observations.iter().any(|o| {
        o.observed_at > now
            || p.start_at.is_none_or(|v| o.observed_at < v)
            || p.end_at.is_none_or(|v| o.observed_at > v)
    }) {
        limits.push("Observations lack a valid sealed time window");
    }
    if manifest.schema_version < 7
        && manifest
            .assignments
            .iter()
            .any(|a| a.cohort.actual_model.is_none())
    {
        limits.push("Actual model identity is unknown for at least one assigned run");
    }
    if manifest
        .assignments
        .iter()
        .any(|a| a.origin != Origin::Real)
    {
        limits.push(
            "Historical, injected and extracted-module samples do not establish real pilot benefit",
        );
    }
    if !complete {
        limits.push("Execution/report evidence is incomplete; assigned failures and missing runs remain in denominators");
    }
    if model_missing {
        limits.push("Observed runs lack archived model capture evidence");
    }
    let schedule_audit = (manifest.schema_version >= 5).then(|| super::schedule::audit(manifest));
    if let Some((_, status)) = &schedule_audit {
        match *status {
            "deviated" => limits.push("Observed run order differs from the sealed execution order"),
            "incomplete" => limits.push("Observed run order is missing start-sequence evidence"),
            _ => {}
        }
    }
    if let Some(audit) = &attempt_audit {
        if !audit.compliant {
            limits.push("Attempt budget or no-progress stop rule was violated");
        }
        if !audit.evidence_complete {
            limits.push("Initial or attempt reports do not establish complete progress evidence");
        }
    }
    let mut summary = json!({"schema_version":1,"id":manifest.id,"evaluated_at":now,"complete":complete,
        "authority":"descriptive_only","trial_acceptance":"requires_external_review",
        "declarations":"A verified plan authorization authenticates the configured owner and exact sealed subject, but does not provide an independent trusted timestamp. Canonical issue labels, reviewers, costs, tools, permissions and external archive authority remain caller declarations; report bindings are verified.",
        "protocol":p,"plan_seal":manifest.plan_seal,
        "plan_authorization":authorization.map_or_else(
            || json!({"status":"absent","required":true}),
            |value| json!({"status":"authenticated","method":"ed25519_dsse","evidence":value})
        ),
        "trial_acceptance_evidence":{"status":"pending","required":true},
        "protocol_ready":limits.is_empty(),"limitations":limits,
        "groups":groups,"comparisons":comparisons});
    if let Some(budget) = super::budget::assess(manifest)? {
        summary["budget"] = serde_json::to_value(budget)?;
    }
    if let Some(declared) = &p.task_mix {
        let mut roster = BTreeMap::<&str, &Assignment>::new();
        for assignment in &manifest.assignments {
            roster.entry(&assignment.input_id).or_insert(assignment);
        }
        let mut observed = BTreeMap::<TaskKind, usize>::new();
        for assignment in roster.values() {
            *observed.entry(assignment.task_kind.clone()).or_default() += 1;
        }
        summary["sampling_audit"] = json!({
            "declared":declared,"observed":observed,"distinct_inputs":roster.len(),
            "tasks":roster.values().map(|a|json!({"input_id":a.input_id,"task_id":a.task_id,
                "task_kind":a.task_kind,"task_digest":a.task_digest})).collect::<Vec<_>>()
        });
    }
    if manifest.schema_version >= 4 {
        summary["source_audit"] = json!({
            "distinct_inputs":manifest.sources.len(),
            "sources":manifest.sources.iter().map(|source|json!({
                "input_id":source.input_id,"kind":source.kind,
                "source_id":source.source_id,"digest":source.digest,
                "selected_at":source.selected_at
            })).collect::<Vec<_>>()
        });
    }
    if let Some((audit, _)) = schedule_audit {
        summary["schedule_audit"] = audit;
    }
    if let Some(audit) = attempt_audit {
        summary["attempt_audit"] = audit.value;
    }
    if manifest.schema_version >= 7 {
        let records: Vec<_> = manifest
            .observations
            .iter()
            .map(|observation| {
                observation.model_evidence.as_ref().map_or_else(
                    || json!({"assignment_id":observation.assignment_id,"status":"missing"}),
                    |evidence| {
                        json!({"assignment_id":observation.assignment_id,
                    "status":evidence.capture.status,
                    "requested_model":evidence.capture.requested_model,
                    "actual_model":evidence.capture.actual_model,
                    "unknown_reason":evidence.capture.unknown_reason,
                    "captured_at":evidence.capture.captured_at,
                    "artifact_digest":evidence.artifact.digest})
                    },
                )
            })
            .collect();
        summary["model_audit"] = json!({
            "comparison_scope":"requested_model_configuration",
            "actual_identity_unknown":manifest.observations.iter().filter(|o|
                o.model_evidence.as_ref().is_some_and(|e|e.capture.status == ModelIdentityStatus::Unknown)).count(),
            "unobserved":manifest.assignments.len()-manifest.observations.len(),
            "records":records,
            "note":"Archived records bind declared model metadata, but do not authenticate provider routing or capture time"
        });
    }
    Ok(summary)
}

pub fn summarize_with_acceptance(
    manifest: &Manifest,
    reports: &Reports,
    now: u64,
    authorization: Option<&VerifiedPlanAuthorization>,
    acceptance: Option<&VerifiedPilotAcceptance>,
) -> Result<Value> {
    let mut summary = summarize_with_authorization(manifest, reports, now, authorization)?;
    if let Some(value) = acceptance {
        let authorization = authorization
            .ok_or_else(|| anyhow::anyhow!("Pilot acceptance requires start authorization"))?;
        let expected = super::acceptance::subject_from_summary(manifest, &summary, authorization)?;
        if value.record.subject != expected {
            anyhow::bail!("Verified pilot acceptance does not match the summarized evidence");
        }
        acceptance_decision(&expected, value.record.decision)?;
        let status = match value.record.decision {
            PilotAcceptanceDecision::Accepted => "accepted",
            PilotAcceptanceDecision::Rejected => "rejected",
        };
        summary["authority"] = json!("authenticated_external_review");
        summary["trial_acceptance"] = json!(status);
        summary["trial_acceptance_evidence"] = json!({
            "status":status,"method":"ed25519_dsse","evidence":value
        });
    }
    Ok(summary)
}
