//! Audited crapper EDN v1 observations; coverage acquisition belongs to the caller.

use super::{
    contracts::*,
    decimal::Decimal,
    edn::{self, take},
    normalization::NormalizeContext,
};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

pub(crate) fn parse(
    raw: &[u8],
    context: &NormalizeContext,
    deadline: Instant,
) -> Result<Vec<FactRecord>> {
    let coverage = context
        .coverage
        .as_ref()
        .context("Crapper requires independently collected coverage evidence")?;
    ensure!(
        context.spec.coverage_producer.as_deref() == Some(&coverage.producer)
            && coverage.snapshot_digest == context.snapshot_digest,
        "Coverage producer or snapshot binding mismatch"
    );
    ensure!(
        !coverage.artifact_digests.is_empty() && !coverage.inputs.is_empty(),
        "Coverage has no independently retained file evidence"
    );
    ensure!(
        coverage
            .artifact_digests
            .iter()
            .all(|d| crate::content_digest::is_sha256(d))
            && coverage
                .inputs
                .values()
                .all(|d| coverage.artifact_digests.contains(d)),
        "Coverage artifact identity is invalid"
    );
    for path in coverage.inputs.keys() {
        validate_path(path)?;
    }
    let mut root = edn::parse(raw)?.map()?;
    let entries = take(&mut root, "entries")?.list()?;
    ensure!(root.is_empty(), "Unknown crapper root field");
    let inventory: BTreeMap<_, _> = context
        .inventory
        .entities
        .iter()
        .map(|e| ((e.namespace.as_str(), e.name.as_str()), e))
        .collect();
    let definitions = context.spec.metric_definitions();
    let mut records = Vec::new();
    let mut budget = super::normalization::RecordBudget::new(deadline, context)?;
    let mut seen = BTreeSet::new();
    for entry in entries {
        let mut entry = entry.map()?;
        let name = take(&mut entry, "name")?.string()?;
        let namespace = take(&mut entry, "namespace")?.string()?;
        let expected = inventory
            .get(&(namespace.as_str(), name.as_str()))
            .context("Crapper function is missing from independent source inventory")?;
        ensure!(
            seen.insert((namespace.clone(), name.clone())),
            "Duplicate crapper function identity"
        );
        let location = &expected.location;
        let lines = coverage
            .lines
            .get(&location.file)
            .context("Coverage is missing a source file; missing does not mean zero")?;
        ensure!(
            lines.keys().all(|line| *line > 0)
                && lines
                    .range(location.start_line..=location.end_line)
                    .next()
                    .is_some(),
            "Coverage has no explicit observations for this function"
        );
        let complexity = Decimal::parse(&take(&mut entry, "complexity")?.number()?)?;
        ensure!(
            complexity.as_i64()? >= 1,
            "Complexity must be a positive i64"
        );
        let covered = Decimal::parse(&take(&mut entry, "coverage")?.number()?)?;
        ensure!(
            covered.scale <= 4
                && covered >= Decimal::from_i64(0)
                && covered <= Decimal::from_i64(100),
            "Invalid crapper coverage percentage"
        );
        let crap = Decimal::parse(&take(&mut entry, "crap")?.number()?)?;
        ensure!(
            crap.scale <= 4 && crap >= complexity,
            "Invalid crapper risk observation"
        );
        ensure!(entry.is_empty(), "Unknown crapper entry field");
        for (value, definition) in [complexity, covered, crap].into_iter().zip(&definitions) {
            ensure!(
                records.len() < MAX_RECORDS,
                "Normalized crapper records exceed bound"
            );
            let id = crate::content_digest::sha256(&serde_json::to_vec(&(
                location,
                &definition.metric,
            ))?);
            budget.retain(
                &mut records,
                FactRecord::Metric(MetricFact {
                    id,
                    location: location.clone(),
                    metric: definition.metric.clone(),
                    value,
                    unit: definition.unit.clone(),
                    measurement: definition.measurement.clone(),
                }),
            )?;
        }
    }
    ensure!(
        seen.len() == inventory.len(),
        "Crapper did not report the complete function inventory"
    );
    Ok(records)
}
