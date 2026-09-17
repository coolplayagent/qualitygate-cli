//! Pure, bounded full-pilot monetary accounting for v2 plans.
use super::{Manifest, PilotBudgetAssessment, ThresholdStatus};
use anyhow::{Result, anyhow};
use std::collections::BTreeMap;

fn add(total: &mut u64, value: u64) -> Result<()> {
    *total = total
        .checked_add(value)
        .ok_or_else(|| anyhow!("Pilot cost total overflow"))?;
    Ok(())
}

pub(super) fn assess(manifest: &Manifest) -> Result<Option<PilotBudgetAssessment>> {
    let Some(budget) = &manifest.protocol.budget else {
        return Ok(None);
    };
    let observations: BTreeMap<_, _> = manifest
        .observations
        .iter()
        .map(|value| (value.assignment_id.as_str(), value))
        .collect();
    let (mut model, mut infra, mut human, mut unknown) = (0_u64, 0_u64, 0_u64, 0_usize);
    for assignment in &manifest.assignments {
        let Some(observation) = observations.get(assignment.id.as_str()) else {
            unknown += 1;
            continue;
        };
        match observation.review_active_ms {
            Some(ms) => {
                let product = u128::from(ms) * u128::from(budget.human_hourly_micros);
                let charge = product.div_ceil(3_600_000);
                add(&mut human, u64::try_from(charge)?)?;
            }
            None => unknown += 1,
        }
        if observation.attempts.is_empty() {
            unknown += 1;
        }
        for attempt in &observation.attempts {
            match &attempt.cost {
                Some(cost)
                    if cost.currency == budget.currency && cost.priced_at == budget.priced_at =>
                {
                    if let Some(value) = cost.model_micros {
                        add(&mut model, value)?;
                    } else {
                        unknown += 1;
                    }
                    if let Some(value) = cost.infrastructure_micros {
                        add(&mut infra, value)?;
                    } else {
                        unknown += 1;
                    }
                }
                _ => unknown += 1,
            }
        }
    }
    let mut total = model;
    add(&mut total, infra)?;
    add(&mut total, human)?;
    let status = if total > budget.max_total_micros {
        ThresholdStatus::NotMet
    } else if unknown > 0 || manifest.assignments.is_empty() {
        ThresholdStatus::Unknown
    } else {
        ThresholdStatus::Met
    };
    Ok(Some(PilotBudgetAssessment {
        currency: budget.currency.clone(),
        priced_at: budget.priced_at,
        max_total_micros: budget.max_total_micros,
        known_model_micros: model,
        known_infrastructure_micros: infra,
        known_human_micros: human,
        known_total_micros: total,
        unknown_inputs: unknown,
        status,
    }))
}
