use super::*;
use crate::domain::evolution::{ActorKind, valid_digest, validate_text};
use anyhow::{Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
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

#[derive(Serialize)]
struct Plan<'a> {
    schema_version: u32,
    id: &'a str,
    protocol: &'a Protocol,
    assignments: Vec<Assignment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<&'a [TaskSource]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_order: Option<&'a [String]>,
}

fn plan_digest(manifest: &Manifest) -> Result<String> {
    let mut assignments = manifest.assignments.clone();
    for assignment in &mut assignments {
        // Providers may disclose the actual routed model only after execution.
        // The pre-observation plan binds the requested model and every other
        // cohort input while allowing that observation to be added later.
        assignment.cohort.actual_model = None;
    }
    let plan = Plan {
        schema_version: manifest.schema_version,
        id: &manifest.id,
        protocol: &manifest.protocol,
        assignments,
        sources: (manifest.schema_version >= 4).then_some(&manifest.sources),
        run_order: (manifest.schema_version >= 5).then_some(&manifest.run_order),
    };
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&plan)?)
    ))
}

fn planned_cohort(assignment: &Assignment) -> Result<String> {
    let mut cohort = assignment.cohort.clone();
    cohort.actual_model = None;
    Ok(serde_json::to_string(&cohort)?)
}

fn seal_blockers(manifest: &Manifest, now: u64) -> Result<Vec<&'static str>> {
    let p = &manifest.protocol;
    let mut blockers = Vec::new();
    if [&p.owner, &p.reviewer, &p.archive]
        .iter()
        .any(|value| value.is_none())
        || (manifest.schema_version == 1 && p.monetary_cap.is_none())
        || ((2..=9).contains(&manifest.schema_version) && p.budget.is_none())
    {
        blockers.push(
            "owner, reviewer, durable archive and any version-required monetary budget must be declared",
        );
    }
    if p.owner.is_some() && p.owner == p.reviewer {
        blockers.push("pilot owner and reviewer must be distinct");
    }
    if p.sealed_at
        .zip(p.start_at)
        .zip(p.end_at)
        .is_none_or(|((sealed, start), end)| {
            sealed > now
                || now > start
                || sealed > start
                || end.saturating_sub(start) < u64::from(p.days) * 86_400
        })
    {
        blockers.push("the declared seal and observation window are not ready");
    }
    if !manifest.observations.is_empty() {
        blockers.push("a pilot plan must be sealed before observations are recorded");
    }
    let inputs: BTreeSet<_> = manifest
        .assignments
        .iter()
        .map(|assignment| assignment.input_id.as_str())
        .collect();
    if inputs.len() != p.task_count {
        blockers.push("assigned distinct tasks differ from the declared sample size");
    }
    if manifest.assignments.iter().any(|assignment| {
        assignment.origin != Origin::Real
            || assignment.exclusion.is_some()
            || assignment.expected_issues.is_none()
    }) {
        blockers.push(
            "sealed assignments require real inputs, declared ground truth and no post-hoc exclusion",
        );
    }
    let models: BTreeSet<_> = manifest
        .assignments
        .iter()
        .map(|assignment| assignment.cohort.requested_model.as_str())
        .collect();
    let workflows: BTreeSet<_> = manifest
        .assignments
        .iter()
        .map(|assignment| &assignment.cohort.workflow)
        .collect();
    if models.len() < 2
        || !workflows.contains(&Workflow::ExistingTools)
        || !workflows.contains(&Workflow::Qualitygate)
    {
        blockers.push("the pilot requires at least two model groups and both comparison workflows");
    }
    let expected_pairs: BTreeSet<_> = models
        .iter()
        .flat_map(|model| {
            [&Workflow::ExistingTools, &Workflow::Qualitygate]
                .into_iter()
                .map(move |workflow| (*model, workflow))
        })
        .collect();
    let actual_pairs: BTreeSet<_> = manifest
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.cohort.requested_model.as_str(),
                &assignment.cohort.workflow,
            )
        })
        .collect();
    if actual_pairs != expected_pairs {
        blockers.push("the model-by-workflow comparison matrix is incomplete");
    }
    let mut matrix = BTreeMap::<&str, BTreeSet<String>>::new();
    for assignment in &manifest.assignments {
        if !matrix
            .entry(&assignment.input_id)
            .or_default()
            .insert(planned_cohort(assignment)?)
        {
            blockers.push("an input has a duplicate planned cohort");
            break;
        }
    }
    if matrix
        .values()
        .next()
        .is_some_and(|first| matrix.values().any(|cohorts| cohorts != first))
    {
        blockers.push("each input must use the same planned cohort matrix");
    }
    Ok(blockers)
}

/// Produces an integrity seal over the pre-observation plan. The digest is not
/// an identity signature; external archive and reviewer authority remain caller-owned.
pub fn seal(mut manifest: Manifest, now: u64) -> Result<Manifest> {
    validate(&manifest)?;
    let blockers = seal_blockers(&manifest, now)?;
    if !blockers.is_empty() {
        bail!("Pilot plan is not sealable: {}", blockers.join("; "));
    }
    manifest.plan_seal = Some(PlanSeal {
        algorithm: "sha256".into(),
        digest: plan_digest(&manifest)?,
    });
    validate(&manifest)?;
    Ok(manifest)
}

/// The exact pre-observation plan and governance identity that an external
/// owner signs. Signature and trust-store verification belong to adapters.
pub fn authorization_subject(manifest: &Manifest) -> Result<PlanAuthorizationSubject> {
    validate(manifest)?;
    let p = &manifest.protocol;
    let subject = PlanAuthorizationSubject {
        repository: p.project.clone(),
        pilot_id: manifest.id.clone(),
        plan_seal: manifest
            .plan_seal
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires a plan seal"))?,
        owner: p
            .owner
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires an owner"))?,
        reviewer: p
            .reviewer
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires a reviewer"))?,
        sealed_at: p
            .sealed_at
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires sealed_at"))?,
        start_at: p
            .start_at
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires start_at"))?,
        end_at: p
            .end_at
            .ok_or_else(|| anyhow::anyhow!("Pilot start authorization requires end_at"))?,
        task_count: p.task_count,
        assignment_count: manifest.assignments.len(),
    };
    if subject.owner == subject.reviewer {
        bail!("Pilot owner and reviewer must be distinct");
    }
    Ok(subject)
}

pub fn validate(manifest: &Manifest) -> Result<()> {
    let p = &manifest.protocol;
    let t = &p.thresholds;
    if !(1..=10).contains(&manifest.schema_version)
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
    match (manifest.schema_version, &p.budget, &p.monetary_cap) {
        (1, None, _) => {}
        (2..=9, Some(budget), None) => {
            if budget.currency.len() != 3
                || !budget.currency.bytes().all(|c| c.is_ascii_uppercase())
                || budget.priced_at == 0
                || !(1..=1_000_000_000_000_000).contains(&budget.max_total_micros)
                || !(1..=1_000_000_000_000).contains(&budget.human_hourly_micros)
            {
                bail!("Invalid structured pilot budget");
            }
            text(&budget.source)?;
        }
        (10, None, None) => {}
        _ => bail!(
            "Pilot v1 uses monetary_cap text; v2-v9 require a structured budget; v10 has no monetary budget"
        ),
    }
    match (manifest.schema_version, &p.task_mix) {
        (1 | 2, None) => {}
        (3..=10, Some(mix))
            if !mix.is_empty()
                && mix
                    .values()
                    .all(|count| *count > 0 && *count <= p.task_count)
                && mix
                    .values()
                    .try_fold(0_usize, |total, count| total.checked_add(*count))
                    == Some(p.task_count) => {}
        _ => bail!("Pilot v3+ requires a bounded task mix; v1/v2 cannot declare one"),
    }
    match (manifest.schema_version, p.no_progress_limit) {
        (1..=5, None) => {}
        (6..=10, Some(limit)) if limit > 0 && limit <= p.max_attempts => {}
        _ => bail!(
            "Pilot v6 requires a bounded no-progress limit; older versions cannot declare one"
        ),
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
    for value in [t.full_p95_ratio_max].into_iter().chain(t.cost_ratio_max) {
        if !value.is_finite() || value <= 0.0 || value > 1000.0 {
            bail!("Invalid cost/time ratio threshold");
        }
    }
    if (manifest.schema_version == 10) != t.cost_ratio_max.is_none() {
        bail!("Pilot v1-v9 require cost_ratio_max; v10 omits it");
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
    let mut reports = BTreeSet::new();
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
        match (manifest.schema_version, &a.initial_report) {
            (1..=5, None) => {}
            (6..=10, Some(artifact)) => {
                text(&artifact.path)?;
                digest(&artifact.digest)?;
                if artifact.bytes == 0
                    || artifact.bytes > 16 * 1024 * 1024
                    || !reports.insert(&artifact.digest)
                {
                    bail!("Invalid or repeated initial report artifact");
                }
            }
            _ => bail!(
                "Pilot v6 requires one initial report per assignment; older versions cannot declare one"
            ),
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
    if let Some(expected_mix) = &p.task_mix {
        let mut by_input = BTreeMap::<&str, &Assignment>::new();
        for assignment in &manifest.assignments {
            by_input.entry(&assignment.input_id).or_insert(assignment);
        }
        let mut observed_mix = BTreeMap::<TaskKind, usize>::new();
        let mut task_ids = BTreeSet::new();
        let mut task_digests = BTreeSet::new();
        for assignment in by_input.values() {
            *observed_mix
                .entry(assignment.task_kind.clone())
                .or_default() += 1;
            if !task_ids.insert(assignment.task_id.as_str()) {
                bail!("Distinct pilot inputs cannot reuse a task ID");
            }
            if !task_digests.insert(assignment.task_digest.as_str()) {
                bail!("Distinct pilot inputs cannot reuse a task contract digest");
            }
        }
        if &observed_mix != expected_mix {
            bail!("Pilot task mix differs from the predeclared independent-task strata");
        }
    }
    if manifest.schema_version < 4 {
        if !manifest.sources.is_empty() {
            bail!("Pilot task source records require schema v4");
        }
    } else {
        if manifest.sources.len() != p.task_count {
            bail!("Pilot v4 needs one source record per independent task");
        }
        let mut source_inputs = BTreeSet::new();
        let mut source_ids = BTreeSet::new();
        let mut source_paths = BTreeSet::new();
        let mut source_digests = BTreeSet::new();
        let mut total_bytes = 0_u64;
        for source in &manifest.sources {
            text(&source.input_id)?;
            validate_text(&source.source_id, 256).map_err(anyhow::Error::msg)?;
            text(&source.path)?;
            digest(&source.digest)?;
            if source.source_id.chars().any(char::is_whitespace)
                || source.bytes == 0
                || source.bytes > 64 * 1024
                || source.selected_at == 0
                || p.sealed_at.is_none_or(|sealed| source.selected_at > sealed)
            {
                bail!("Invalid pilot task source identity, size or selection time");
            }
            total_bytes = total_bytes
                .checked_add(source.bytes)
                .ok_or_else(|| anyhow::anyhow!("Pilot task source inventory overflows"))?;
            if total_bytes > 8 * 1024 * 1024 {
                bail!("Pilot task source inventory exceeds 8 MiB");
            }
            if !source_inputs.insert(source.input_id.as_str())
                || !source_ids.insert(source.source_id.as_str())
                || !source_paths.insert(source.path.as_str())
                || !source_digests.insert(source.digest.as_str())
            {
                bail!("Pilot independent tasks cannot reuse source identity or artifact");
            }
        }
        if source_inputs != task_inputs.keys().copied().collect() {
            bail!("Pilot task sources differ from assigned independent inputs");
        }
    }
    if manifest.schema_version < 5 {
        if !manifest.run_order.is_empty() {
            bail!("Pilot run order requires schema v5");
        }
    } else {
        validate_run_order(manifest, &assigned)?;
    }
    let mut observed = BTreeSet::new();
    let mut start_sequences = BTreeSet::new();
    for o in &manifest.observations {
        if !assigned.contains_key(o.assignment_id.as_str()) || !observed.insert(&o.assignment_id) {
            bail!("Unknown or repeated observed assignment");
        }
        if o.observed_at == 0 || o.attempts.len() > 10 || o.findings.len() > 4096 {
            bail!("Invalid observation time or inventory");
        }
        match (manifest.schema_version, o.start_sequence) {
            (1..=4, Some(_)) => bail!("Observed start sequence requires schema v5"),
            (5..=10, Some(sequence))
                if sequence == 0
                    || usize::from(sequence) > manifest.run_order.len()
                    || !start_sequences.insert(sequence) =>
            {
                bail!("Invalid or repeated observed start sequence");
            }
            _ => {}
        }
        match (manifest.schema_version, &o.model_evidence) {
            (1..=6, None) => {}
            (7, Some(evidence)) => {
                let artifact = &evidence.artifact;
                let capture = &evidence.capture;
                text(&artifact.path)?;
                digest(&artifact.digest)?;
                if artifact.bytes == 0
                    || artifact.bytes > 64 * 1024
                    || !reports.insert(&artifact.digest)
                    || capture.attempt_number.is_some()
                    || capture.captured_at == 0
                    || capture.captured_at > o.observed_at
                    || p.start_at.is_some_and(|start| capture.captured_at < start)
                    || p.end_at.is_some_and(|end| capture.captured_at > end)
                {
                    bail!("Invalid pilot model capture artifact or time");
                }
                let assignment = assigned[o.assignment_id.as_str()];
                if capture.assignment_id != o.assignment_id
                    || capture.agent_version != assignment.cohort.agent_version
                    || capture.harness_digest != assignment.cohort.harness_digest
                    || capture.requested_model != assignment.cohort.requested_model
                    || capture.reasoning_effort != assignment.cohort.reasoning_effort
                    || capture.actual_model != assignment.cohort.actual_model
                {
                    bail!("Pilot model capture differs from its sealed assignment");
                }
                for value in [
                    &capture.assignment_id,
                    &capture.agent_version,
                    &capture.requested_model,
                    &capture.reasoning_effort,
                ] {
                    text(value)?;
                }
                digest(&capture.harness_digest)?;
                match (
                    &capture.status,
                    &capture.actual_model,
                    &capture.unknown_reason,
                ) {
                    (ModelIdentityStatus::Reported, Some(_), None) => {}
                    (ModelIdentityStatus::Unknown, None, Some(reason)) => text(reason)?,
                    _ => bail!("Pilot model identity must be reported or explicitly unknown"),
                }
            }
            (7, None) => {}
            (8..=10, None) => {}
            _ => bail!("Pilot run-level model evidence requires schema v7"),
        }
        if [o.review_active_ms, o.review_comments, o.rework_rounds]
            .into_iter()
            .flatten()
            .any(|v| v > 1_000_000_000)
        {
            bail!("Human observations exceed bounds");
        }
        let mut previous_end = None;
        for (i, a) in o.attempts.iter().enumerate() {
            if a.number as usize != i + 1
                || !["quick", "full"].contains(&a.profile.as_str())
                || a.elapsed_ms > 86_400_000
                || a.check_elapsed_ms.is_some_and(|ms| ms > a.elapsed_ms)
            {
                bail!("Invalid attempt sequence, profile or duration");
            }
            digest(&a.snapshot_digest)?;
            match (manifest.schema_version, &a.model_evidence) {
                (1..=7, None) | (8..=10, None) => {}
                (8..=10, Some(evidence)) => {
                    let artifact = &evidence.artifact;
                    let capture = &evidence.capture;
                    text(&artifact.path)?;
                    digest(&artifact.digest)?;
                    if artifact.bytes == 0
                        || artifact.bytes > 64 * 1024
                        || !reports.insert(&artifact.digest)
                        || capture.attempt_number != Some(a.number)
                        || capture.captured_at == 0
                        || capture.captured_at > o.observed_at
                        || p.start_at.is_some_and(|start| capture.captured_at < start)
                        || p.end_at.is_some_and(|end| capture.captured_at > end)
                    {
                        bail!("Invalid pilot attempt model capture artifact, number or time");
                    }
                    let assignment = assigned[o.assignment_id.as_str()];
                    if capture.assignment_id != o.assignment_id
                        || capture.agent_version != assignment.cohort.agent_version
                        || capture.harness_digest != assignment.cohort.harness_digest
                        || capture.requested_model != assignment.cohort.requested_model
                        || capture.reasoning_effort != assignment.cohort.reasoning_effort
                    {
                        bail!("Pilot attempt model capture differs from its sealed assignment");
                    }
                    for value in [
                        &capture.assignment_id,
                        &capture.agent_version,
                        &capture.requested_model,
                        &capture.reasoning_effort,
                    ] {
                        text(value)?;
                    }
                    digest(&capture.harness_digest)?;
                    match (
                        &capture.status,
                        &capture.actual_model,
                        &capture.unknown_reason,
                    ) {
                        (ModelIdentityStatus::Reported, Some(model), None) => text(model)?,
                        (ModelIdentityStatus::Unknown, None, Some(reason)) => text(reason)?,
                        _ => bail!("Pilot model identity must be reported or explicitly unknown"),
                    }
                }
                _ => bail!("Pilot attempt model evidence requires schema v8"),
            }
            match (manifest.schema_version, &a.execution_evidence) {
                (1..=8, None) | (9 | 10, None) => {}
                (9 | 10, Some(evidence)) => {
                    let artifact = &evidence.artifact;
                    let capture = &evidence.capture;
                    text(&artifact.path)?;
                    digest(&artifact.digest)?;
                    digest(&capture.harness_digest)?;
                    digest(&capture.snapshot_digest)?;
                    if let Some(report) = &capture.report_digest {
                        digest(report)?;
                    }
                    let assignment = assigned[o.assignment_id.as_str()];
                    let window_start = p.start_at.and_then(|time| time.checked_mul(1000));
                    let window_end = p
                        .end_at
                        .and_then(|time| time.checked_mul(1000))
                        .and_then(|time| time.checked_add(999));
                    let observed_end = o
                        .observed_at
                        .checked_mul(1000)
                        .and_then(|time| time.checked_add(999));
                    if artifact.bytes == 0
                        || artifact.bytes > 64 * 1024
                        || !reports.insert(&artifact.digest)
                        || capture.assignment_id != o.assignment_id
                        || capture.attempt_number != a.number
                        || capture.harness_digest != assignment.cohort.harness_digest
                        || capture.status != a.status
                        || capture.snapshot_digest != a.snapshot_digest
                        || capture.report_digest.as_deref()
                            != a.report.as_ref().map(|report| report.digest.as_str())
                        || capture.started_at_ms == 0
                        || capture.ended_at_ms.checked_sub(capture.started_at_ms)
                            != Some(a.elapsed_ms)
                        || window_start.is_none_or(|start| capture.started_at_ms < start)
                        || window_end.is_none_or(|end| capture.ended_at_ms > end)
                        || observed_end.is_none_or(|end| capture.ended_at_ms > end)
                        || previous_end.is_some_and(|end| capture.started_at_ms < end)
                    {
                        bail!("Invalid pilot execution capture, duration or chronology");
                    }
                    text(&capture.assignment_id)?;
                    previous_end = Some(capture.ended_at_ms);
                }
                _ => bail!("Pilot attempt execution evidence requires schema v9"),
            }
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
        if manifest.schema_version >= 8 {
            let actual: BTreeSet<_> = o
                .attempts
                .iter()
                .filter_map(|attempt| attempt.model_evidence.as_ref())
                .filter_map(|evidence| evidence.capture.actual_model.as_deref())
                .collect();
            let complete_reported = !o.attempts.is_empty()
                && o.attempts.iter().all(|attempt| {
                    attempt.model_evidence.as_ref().is_some_and(|evidence| {
                        evidence.capture.status == ModelIdentityStatus::Reported
                    })
                });
            let expected = (complete_reported && actual.len() == 1)
                .then(|| *actual.iter().next().expect("one reported actual model"));
            if assigned[o.assignment_id.as_str()]
                .cohort
                .actual_model
                .as_deref()
                != expected
            {
                bail!("Pilot actual model must match complete attempt captures");
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
    if manifest.schema_version >= 8
        && manifest.assignments.iter().any(|assignment| {
            !observed.contains(&assignment.id) && assignment.cohort.actual_model.is_some()
        })
    {
        bail!("Unobserved pilot assignments cannot claim an actual model");
    }
    if let Some(seal) = &manifest.plan_seal {
        if seal.algorithm != "sha256" || !valid_digest(&seal.digest) {
            bail!("Invalid pilot plan seal");
        }
        if plan_digest(manifest)? != seal.digest {
            bail!("Pilot plan differs from its pre-observation seal");
        }
    }
    Ok(())
}

fn validate_run_order(manifest: &Manifest, assigned: &BTreeMap<&str, &Assignment>) -> Result<()> {
    if manifest.run_order.len() != assigned.len() || manifest.run_order.len() > 512 {
        bail!("Pilot v5 needs one ordered position per assignment");
    }
    let mut seen = BTreeSet::new();
    let mut previous = None;
    let mut pairs = BTreeMap::<(TaskKind, &str, &str), [Option<usize>; 2]>::new();
    for (position, id) in manifest.run_order.iter().enumerate() {
        let assignment = assigned
            .get(id.as_str())
            .ok_or_else(|| anyhow::anyhow!("Pilot run order contains an unknown assignment"))?;
        if !seen.insert(id.as_str()) {
            bail!("Pilot run order repeats an assignment");
        }
        let workflow = match assignment.cohort.workflow {
            Workflow::ExistingTools => 0,
            Workflow::Qualitygate => 1,
        };
        if previous == Some(workflow) {
            bail!("Pilot run order must alternate workflows");
        }
        previous = Some(workflow);
        let slots = pairs
            .entry((
                assignment.task_kind.clone(),
                assignment.cohort.requested_model.as_str(),
                assignment.input_id.as_str(),
            ))
            .or_default();
        if slots[workflow].replace(position).is_some() {
            bail!("Pilot run order repeats a workflow for one input and model");
        }
    }
    let mut strata = BTreeMap::<(TaskKind, &str), [usize; 2]>::new();
    for ((kind, model, _), [existing, qualitygate]) in pairs {
        let (Some(existing), Some(qualitygate)) = (existing, qualitygate) else {
            bail!("Pilot run order lacks a workflow pair for one input and model");
        };
        let first = usize::from(qualitygate < existing);
        strata.entry((kind, model)).or_default()[first] += 1;
    }
    if strata
        .values()
        .any(|counts| counts[0].abs_diff(counts[1]) > 1)
    {
        bail!("Pilot run order is not counterbalanced within task kind and model strata");
    }
    Ok(())
}
