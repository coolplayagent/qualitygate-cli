use super::*;
use crate::domain::evolution::{ActorKind, valid_digest, validate_text};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

fn text(value: &str) -> Result<()> {
    validate_text(value, 4096).map_err(anyhow::Error::msg)
}
fn digest(value: &str) -> Result<()> {
    if !valid_digest(value) {
        bail!("Invalid observation digest");
    }
    Ok(())
}
fn fraction(value: f64) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("Invalid fraction threshold");
    }
    Ok(())
}
fn names(values: &[String]) -> Result<()> {
    if values.len() > 4096 || values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        bail!("Observation identities must be distinct and bounded");
    }
    for value in values {
        text(value)?;
    }
    Ok(())
}

pub fn validate(manifest: &Manifest) -> Result<()> {
    let p = &manifest.protocol;
    let t = &p.thresholds;
    if manifest.schema_version != 1
        || manifest.assignments.len() > 512
        || manifest.observations.len() > 512
        || p.task_count == 0
        || p.task_count > 512
        || p.days == 0
        || p.days > 366
        || !(1..=10).contains(&p.max_attempts)
        || !(1..=86400).contains(&p.max_seconds)
    {
        bail!("Invalid pilot version, inventory or budget");
    }
    text(&manifest.id)?;
    text(&p.project)?;
    text(&p.sampling)?;
    for value in [&p.owner, &p.reviewer, &p.archive, &p.monetary_cap]
        .into_iter()
        .flatten()
    {
        text(value)?;
    }
    for value in [
        t.detection_min,
        t.false_positive_max,
        t.repair_min,
        t.completion_min,
        t.review_reduction_min,
        p.review_fraction_min,
    ] {
        fraction(value)?;
    }
    for value in [t.full_p95_ratio_max, t.cost_ratio_max] {
        if !value.is_finite() || value <= 0.0 || value > 1000.0 {
            bail!("Invalid cost/time ratio threshold");
        }
    }
    if [p.sealed_at, p.start_at, p.end_at]
        .into_iter()
        .flatten()
        .any(|v| v == 0)
        || p.sealed_at.zip(p.start_at).is_some_and(|(a, b)| a > b)
        || p.start_at.zip(p.end_at).is_some_and(|(a, b)| a > b)
    {
        bail!("Invalid protocol chronology");
    }
    let mut assigned = BTreeMap::new();
    let mut task_inputs = BTreeMap::new();
    for a in &manifest.assignments {
        text(&a.id)?;
        text(&a.task_id)?;
        text(&a.input_id)?;
        if assigned.insert(a.id.as_str(), a).is_some() {
            bail!("Duplicate assignment");
        }
        if ![40, 64].contains(&a.base.len())
            || !a
                .base
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            bail!("Expected full Git base");
        }
        for d in [
            &a.initial_snapshot,
            &a.config_digest,
            &a.task_digest,
            &a.cohort.harness_digest,
            &a.cohort.environment_digest,
            &a.cohort.tools_digest,
        ] {
            digest(d)?;
        }
        for s in [
            &a.cohort.agent_version,
            &a.cohort.requested_model,
            &a.cohort.reasoning_effort,
            &a.cohort.cache,
            &a.cohort.permissions,
        ] {
            text(s)?;
        }
        if let Some(model) = &a.cohort.actual_model {
            text(model)?;
        }
        if let Some(reason) = &a.exclusion {
            text(reason)?;
        }
        names(&a.required_checks)?;
        if a.required_checks.is_empty() {
            bail!("Expected required check inventory cannot be empty");
        }
        if let Some(issues) = &a.expected_issues {
            names(issues)?;
        }
        // Replicas and workflows must refer to the same original task, including ground truth.
        let identity = serde_json::to_string(&(
            &a.task_id,
            &a.task_kind,
            &a.origin,
            &a.base,
            &a.initial_snapshot,
            &a.task_digest,
            &a.expected_issues,
        ))?;
        if task_inputs
            .insert(a.input_id.as_str(), identity.clone())
            .is_some_and(|old| old != identity)
        {
            bail!("Task identity differs across assigned groups");
        }
    }
    let mut observed = BTreeSet::new();
    let mut reports = BTreeSet::new();
    for o in &manifest.observations {
        if !assigned.contains_key(o.assignment_id.as_str()) || !observed.insert(&o.assignment_id) {
            bail!("Unknown or repeated observed assignment");
        }
        if o.observed_at == 0 || o.attempts.len() > 10 || o.findings.len() > 4096 {
            bail!("Invalid observation time or inventory");
        }
        if [o.review_active_ms, o.review_comments, o.rework_rounds]
            .into_iter()
            .flatten()
            .any(|v| v > 1_000_000_000)
        {
            bail!("Human observations exceed bounds");
        }
        for (i, a) in o.attempts.iter().enumerate() {
            if a.number as usize != i + 1
                || !["quick", "full"].contains(&a.profile.as_str())
                || a.elapsed_ms > 86_400_000
                || a.check_elapsed_ms.is_some_and(|ms| ms > a.elapsed_ms)
            {
                bail!("Invalid attempt sequence, profile or duration");
            }
            digest(&a.snapshot_digest)?;
            if let Some(r) = &a.report {
                text(&r.path)?;
                digest(&r.digest)?;
                if r.bytes > 16 * 1024 * 1024 || !reports.insert(&r.digest) {
                    bail!(
                        "Report references must be bounded and cannot be reused as independent attempts"
                    );
                }
            }
            if let Some(u) = &a.usage
                && (u.input_tokens > 1_000_000_000_000
                    || u.output_tokens > 1_000_000_000_000
                    || u.cached_input_tokens.is_some_and(|v| v > u.input_tokens)
                    || u.reasoning_output_tokens
                        .is_some_and(|v| v > u.output_tokens)
                    || u.cache_write_input_tokens
                        .is_some_and(|v| v > 1_000_000_000_000))
            {
                bail!("Invalid token usage observation");
            }
            if let Some(c) = &a.cost {
                text(&c.currency)?;
                text(&c.source)?;
                if c.priced_at == 0
                    || [c.model_micros, c.infrastructure_micros]
                        .into_iter()
                        .flatten()
                        .any(|v| v > 1_000_000_000_000)
                {
                    bail!("Invalid cost observation");
                }
            }
        }
        for f in &o.findings {
            text(&f.diagnostic_id)?;
            text(&f.check_id)?;
            text(&f.fingerprint)?;
            digest(&f.report_digest)?;
            if let Some(issue) = &f.issue_id {
                text(issue)?;
            }
            if let Some(review) = &f.review {
                review.actor.validate().map_err(anyhow::Error::msg)?;
                if review.actor.kind != ActorKind::Human
                    || review.label == Label::Confirmed && f.issue_id.is_none()
                {
                    bail!("Confirmed findings require human review and canonical issue identity");
                }
            }
        }
    }
    Ok(())
}
