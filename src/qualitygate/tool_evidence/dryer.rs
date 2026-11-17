//! Preserves both endpoints and the declared acquisition window of dryer pairs.

use super::{
    contracts::*,
    decimal::Decimal,
    edn::{self, Value, take},
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
    let mut root = edn::parse(raw)?.map()?;
    let candidates = take(&mut root, "candidates")?.list()?;
    ensure!(root.is_empty(), "Unknown dryer root field");
    let locations: BTreeMap<_, _> = context
        .inventory
        .entities
        .iter()
        .map(|e| {
            (
                (
                    e.location.file.as_str(),
                    e.location.start_line,
                    e.location.end_line,
                ),
                &e.location,
            )
        })
        .collect();
    let mut records = Vec::new();
    let mut budget = super::normalization::RecordBudget::new(deadline, context)?;
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        let mut candidate = candidate.map()?;
        ensure!(
            take(&mut candidate, "language")?.string()? == "rust",
            "Native dryer adapter supports Rust only"
        );
        let score = Decimal::parse(&take(&mut candidate, "score")?.number()?)?;
        ensure!(
            score.scale <= 12,
            "Dryer precision exceeds audited serialization profile"
        );
        let mut left = location(take(&mut candidate, "left")?, &locations)?;
        let mut right = location(take(&mut candidate, "right")?, &locations)?;
        let mut left_nodes =
            Decimal::parse(&take(&mut candidate, "left-nodes")?.number()?)?.as_i64()?;
        let mut right_nodes =
            Decimal::parse(&take(&mut candidate, "right-nodes")?.number()?)?.as_i64()?;
        ensure!(candidate.is_empty(), "Unknown dryer candidate field");
        if left > right {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut left_nodes, &mut right_nodes);
        }
        ensure!(
            left != right && seen.insert((left.clone(), right.clone())),
            "Repeated or self-referencing clone pair"
        );
        ensure!(
            records.len() < MAX_RECORDS,
            "Normalized dryer records exceed bound"
        );
        let id = crate::content_digest::sha256(&serde_json::to_vec(&(&left, &right))?);
        budget.retain(
            &mut records,
            FactRecord::ClonePair(ClonePairFact {
                id,
                left,
                right,
                similarity: score,
                unit: "ratio".into(),
                measurement: context.spec.clone_measurement(),
                left_nodes,
                right_nodes,
            }),
        )?;
    }
    if context.spec.minimum_nodes == 0 && context.spec.similarity_floor == Decimal::from_i64(0) {
        let count = context
            .inventory
            .entities
            .iter()
            .filter(|e| {
                e.location.end_line - e.location.start_line + 1
                    >= context.spec.minimum_lines as usize
            })
            .count();
        let pairs = count
            .checked_mul(count.saturating_sub(1))
            .context("Clone inventory overflow")?
            / 2;
        ensure!(
            records.len() == pairs,
            "Unfiltered dryer output is missing independently expected pairs"
        );
    }
    Ok(records)
}

fn location(
    value: Value,
    inventory: &BTreeMap<(&str, usize, usize), &Location>,
) -> Result<Location> {
    let mut map = value.map()?;
    let file = take(&mut map, "file")?.string()?;
    let start =
        usize::try_from(Decimal::parse(&take(&mut map, "start-line")?.number()?)?.as_i64()?)?;
    let end = usize::try_from(Decimal::parse(&take(&mut map, "end-line")?.number()?)?.as_i64()?)?;
    ensure!(map.is_empty(), "Unknown dryer location field");
    Ok((*inventory
        .get(&(file.as_str(), start, end))
        .context("Clone endpoint is absent from independent inventory")?)
    .clone())
}
