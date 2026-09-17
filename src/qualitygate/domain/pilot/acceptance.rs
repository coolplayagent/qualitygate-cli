use super::*;
use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const EVALUATOR_V1: &str = "qualitygate-pilot-thresholds-v1";
const EVALUATOR_V2: &str = "qualitygate-pilot-thresholds-v2";

fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(value)?)
    ))
}

fn number(value: &Value, field: &str) -> Result<u64> {
    value[field]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Pilot summary has no numeric {field}"))
}

fn rate_check(
    groups: &[&Value],
    field: &str,
    threshold: f64,
    direction: ThresholdDirection,
) -> Result<ThresholdCheck> {
    let mut numerator = 0_u64;
    let mut denominator = 0_u64;
    let mut unknown = 0_usize;
    for group in groups {
        numerator = numerator.saturating_add(number(&group[field], "numerator")?);
        denominator = denominator.saturating_add(number(&group[field], "denominator")?);
        unknown = unknown.saturating_add(number(&group[field], "unknown")? as usize);
    }
    let observed =
        (denominator > 0 && unknown == 0).then_some(numerator as f64 / denominator as f64);
    let status = match observed {
        Some(value)
            if match direction {
                ThresholdDirection::Minimum => value >= threshold,
                ThresholdDirection::Maximum => value <= threshold,
                ThresholdDirection::AllTrue => unreachable!("rate direction"),
            } =>
        {
            ThresholdStatus::Met
        }
        Some(_) => ThresholdStatus::NotMet,
        None => ThresholdStatus::Unknown,
    };
    Ok(ThresholdCheck {
        metric: field.into(),
        direction,
        threshold: Some(threshold),
        observed,
        numerator: Some(numerator),
        denominator: Some(denominator),
        samples: groups.len(),
        passed: usize::from(status == ThresholdStatus::Met),
        failed: usize::from(status == ThresholdStatus::NotMet),
        unknown,
        status,
    })
}

fn review_fraction(groups: &[&Value], threshold: f64) -> Result<ThresholdCheck> {
    let reviewed = groups.iter().try_fold(0_u64, |sum, group| {
        Ok::<_, anyhow::Error>(sum.saturating_add(number(&group["diagnostics"], "reviewed")?))
    })?;
    let diagnostics = groups.iter().try_fold(0_u64, |sum, group| {
        Ok::<_, anyhow::Error>(sum.saturating_add(number(&group["diagnostics"], "distinct")?))
    })?;
    let observed = (diagnostics > 0).then_some(reviewed as f64 / diagnostics as f64);
    let status = match observed {
        Some(value) if value >= threshold => ThresholdStatus::Met,
        Some(_) => ThresholdStatus::NotMet,
        None => ThresholdStatus::Unknown,
    };
    Ok(ThresholdCheck {
        metric: "review_fraction".into(),
        direction: ThresholdDirection::Minimum,
        threshold: Some(threshold),
        observed,
        numerator: Some(reviewed),
        denominator: Some(diagnostics),
        samples: groups.len(),
        passed: usize::from(status == ThresholdStatus::Met),
        failed: usize::from(status == ThresholdStatus::NotMet),
        unknown: usize::from(status == ThresholdStatus::Unknown),
        status,
    })
}

fn all_true(comparisons: &[Value], field: &str, threshold: Option<f64>) -> ThresholdCheck {
    let values: Vec<_> = comparisons
        .iter()
        .map(|comparison| {
            if field == "matched_inventory" {
                comparison[field].as_bool()
            } else {
                comparison["threshold_observations"][field].as_bool()
            }
        })
        .collect();
    let passed = values.iter().filter(|value| **value == Some(true)).count();
    let failed = values.iter().filter(|value| **value == Some(false)).count();
    let unknown = values.iter().filter(|value| value.is_none()).count();
    let status = if values.is_empty() {
        ThresholdStatus::Unknown
    } else if failed > 0 {
        ThresholdStatus::NotMet
    } else if unknown > 0 {
        ThresholdStatus::Unknown
    } else {
        ThresholdStatus::Met
    };
    ThresholdCheck {
        metric: field.into(),
        direction: ThresholdDirection::AllTrue,
        threshold,
        observed: None,
        numerator: None,
        denominator: None,
        samples: values.len(),
        passed,
        failed,
        unknown,
        status,
    }
}

pub fn threshold_assessment(
    summary: &Value,
    protocol: &Protocol,
) -> Result<PilotThresholdAssessment> {
    let groups = summary["groups"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Pilot summary has no groups"))?;
    let candidates: Vec<_> = groups
        .iter()
        .filter(|group| group["cohort"]["workflow"] == "qualitygate")
        .collect();
    let comparisons = summary["comparisons"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Pilot summary has no comparisons"))?;
    let t = &protocol.thresholds;
    let mut checks = vec![
        rate_check(
            &candidates,
            "detection",
            t.detection_min,
            ThresholdDirection::Minimum,
        )?,
        rate_check(
            &candidates,
            "false_positive",
            t.false_positive_max,
            ThresholdDirection::Maximum,
        )?,
        rate_check(
            &candidates,
            "repair",
            t.repair_min,
            ThresholdDirection::Minimum,
        )?,
        rate_check(
            &candidates,
            "required_check_completion",
            t.completion_min,
            ThresholdDirection::Minimum,
        )?,
        review_fraction(&candidates, protocol.review_fraction_min)?,
        all_true(comparisons, "matched_inventory", None),
        all_true(
            comparisons,
            "review_reduction",
            Some(t.review_reduction_min),
        ),
        all_true(comparisons, "full_p95_ratio", Some(t.full_p95_ratio_max)),
        all_true(comparisons, "cost_ratio", Some(t.cost_ratio_max)),
    ];
    if let Some(budget) = &protocol.budget {
        let assessed: PilotBudgetAssessment = serde_json::from_value(summary["budget"].clone())?;
        if assessed.currency != budget.currency
            || assessed.priced_at != budget.priced_at
            || assessed.max_total_micros != budget.max_total_micros
        {
            bail!("Pilot budget assessment differs from the sealed budget");
        }
        let status = if assessed.known_total_micros > budget.max_total_micros {
            ThresholdStatus::NotMet
        } else if assessed.unknown_inputs > 0 {
            ThresholdStatus::Unknown
        } else {
            ThresholdStatus::Met
        };
        checks.push(ThresholdCheck {
            metric: "total_cost_micros".into(),
            direction: ThresholdDirection::Maximum,
            threshold: Some(budget.max_total_micros as f64),
            observed: Some(assessed.known_total_micros as f64),
            numerator: Some(assessed.known_total_micros),
            denominator: None,
            samples: 1,
            passed: usize::from(status == ThresholdStatus::Met),
            failed: usize::from(status == ThresholdStatus::NotMet),
            unknown: assessed.unknown_inputs,
            status,
        });
    }
    if candidates.is_empty() {
        for check in &mut checks {
            check.status = ThresholdStatus::Unknown;
        }
    }
    let outcome = if checks
        .iter()
        .any(|check| check.status == ThresholdStatus::NotMet)
    {
        ThresholdStatus::NotMet
    } else if checks
        .iter()
        .any(|check| check.status == ThresholdStatus::Unknown)
    {
        ThresholdStatus::Unknown
    } else {
        ThresholdStatus::Met
    };
    Ok(PilotThresholdAssessment {
        schema_version: if protocol.budget.is_some() { 2 } else { 1 },
        evaluator: if protocol.budget.is_some() {
            EVALUATOR_V2.into()
        } else {
            EVALUATOR_V1.into()
        },
        metrics_digest: if protocol.budget.is_some() {
            digest(&(groups, comparisons, &summary["budget"]))?
        } else {
            digest(&(groups, comparisons))?
        },
        outcome,
        checks,
    })
}

pub(super) fn subject_from_summary(
    manifest: &Manifest,
    summary: &Value,
    authorization: &VerifiedPlanAuthorization,
) -> Result<PilotAcceptanceSubject> {
    if summary["complete"] != true || summary["protocol_ready"] != true {
        bail!("Pilot acceptance subject requires complete, protocol-ready evidence");
    }
    let p = &manifest.protocol;
    let owner = p
        .owner
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
    let reviewer = p
        .reviewer
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Missing reviewer"))?;
    if owner == reviewer {
        bail!("Pilot acceptance requires an independent reviewer");
    }
    Ok(PilotAcceptanceSubject {
        repository: p.project.clone(),
        pilot_id: manifest.id.clone(),
        plan_seal: manifest
            .plan_seal
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Missing plan seal"))?,
        owner,
        reviewer,
        observation_end: p.end_at.ok_or_else(|| anyhow::anyhow!("Missing end_at"))?,
        manifest_digest: digest(manifest)?,
        start_authorization_digest: digest(&authorization.record)?,
        start_signer_key_id: authorization.signer_key_id.clone(),
        start_public_key_digest: authorization.public_key_digest.clone(),
        assessment: threshold_assessment(summary, p)?,
    })
}

pub fn acceptance_decision(
    subject: &PilotAcceptanceSubject,
    decision: PilotAcceptanceDecision,
) -> Result<()> {
    if decision == PilotAcceptanceDecision::Accepted
        && subject.assessment.outcome != ThresholdStatus::Met
    {
        bail!("Pilot acceptance cannot approve failed or unknown required thresholds");
    }
    Ok(())
}

pub fn acceptance_subject(
    manifest: &Manifest,
    reports: &Reports,
    now: u64,
    authorization: &VerifiedPlanAuthorization,
) -> Result<PilotAcceptanceSubject> {
    let summary =
        super::metrics::summarize_with_authorization(manifest, reports, now, Some(authorization))?;
    subject_from_summary(manifest, &summary, authorization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn protocol() -> Protocol {
        serde_json::from_value(json!({
            "project":"fixture","sampling":"fixture","owner":"owner","reviewer":"reviewer",
            "archive":"archive","sealed_at":1,"start_at":2,"end_at":604802,
            "task_count":1,"days":7,"max_attempts":3,"max_seconds":1800,
            "review_fraction_min":1.0,"monetary_cap":"USD 10",
            "thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,
                "completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,
                "cost_ratio_max":1.0}
        }))
        .unwrap()
    }

    fn summary() -> Value {
        json!({"groups":[{
            "cohort":{"workflow":"qualitygate"},
            "detection":{"numerator":2,"denominator":2,"unknown":0},
            "false_positive":{"numerator":0,"denominator":2,"unknown":0},
            "repair":{"numerator":2,"denominator":2,"unknown":0},
            "required_check_completion":{"numerator":4,"denominator":4,"unknown":0},
            "diagnostics":{"reviewed":2,"distinct":2}
        }],"comparisons":[{
            "matched_inventory":true,
            "threshold_observations":{"review_reduction":true,"full_p95_ratio":true,"cost_ratio":true}
        }]})
    }

    #[test]
    fn all_required_thresholds_must_be_known_and_met() {
        let protocol = protocol();
        let mut value = summary();
        let met = threshold_assessment(&value, &protocol).unwrap();
        assert_eq!(met.outcome, ThresholdStatus::Met);
        assert_eq!(met.checks.len(), 9);

        value["groups"][0]["detection"]["numerator"] = json!(0);
        assert_eq!(
            threshold_assessment(&value, &protocol).unwrap().outcome,
            ThresholdStatus::NotMet
        );
        value = summary();
        value["comparisons"][0]["threshold_observations"]["cost_ratio"] = Value::Null;
        assert_eq!(
            threshold_assessment(&value, &protocol).unwrap().outcome,
            ThresholdStatus::Unknown
        );
    }

    #[test]
    fn known_comparison_failure_is_not_hidden_by_an_unknown_pair() {
        let protocol = protocol();
        let mut value = summary();
        let mut second = value["comparisons"][0].clone();
        value["comparisons"][0]["threshold_observations"]["review_reduction"] = json!(false);
        second["threshold_observations"]["review_reduction"] = Value::Null;
        value["comparisons"].as_array_mut().unwrap().push(second);
        let result = threshold_assessment(&value, &protocol).unwrap();
        assert_eq!(result.outcome, ThresholdStatus::NotMet);
        assert_eq!(result.checks[6].status, ThresholdStatus::NotMet);
    }

    #[test]
    fn structured_total_budget_is_a_tenth_required_check() {
        let mut protocol = protocol();
        protocol.monetary_cap = None;
        protocol.budget = Some(PilotBudget {
            currency: "USD".into(),
            priced_at: 1,
            source: "fixture tariff".into(),
            max_total_micros: 500,
            human_hourly_micros: 3_600_000,
        });
        let mut value = summary();
        value["budget"] = json!({
            "currency":"USD","priced_at":1,"max_total_micros":500,
            "known_model_micros":100,"known_infrastructure_micros":100,
            "known_human_micros":200,"known_total_micros":400,
            "unknown_inputs":0,"status":"met"
        });
        let result = threshold_assessment(&value, &protocol).unwrap();
        assert_eq!(result.schema_version, 2);
        assert_eq!(result.checks.len(), 10);
        assert_eq!(result.outcome, ThresholdStatus::Met);

        value["budget"]["known_total_micros"] = json!(600);
        assert_eq!(
            threshold_assessment(&value, &protocol).unwrap().outcome,
            ThresholdStatus::NotMet
        );
        value["budget"]["known_total_micros"] = json!(400);
        value["budget"]["unknown_inputs"] = json!(1);
        assert_eq!(
            threshold_assessment(&value, &protocol).unwrap().outcome,
            ThresholdStatus::Unknown
        );
        value["budget"]["currency"] = json!("EUR");
        assert!(threshold_assessment(&value, &protocol).is_err());
    }
}
