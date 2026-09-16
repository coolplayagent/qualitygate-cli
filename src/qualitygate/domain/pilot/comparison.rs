use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn key(cohort: &Cohort) -> Cohort {
    let mut key = cohort.clone();
    key.workflow = Workflow::ExistingTools;
    key
}
fn inventory<'a>(assignments: &'a [Assignment], group: &Value) -> BTreeMap<&'a str, usize> {
    let mut result = BTreeMap::new();
    for a in assignments {
        if serde_json::to_value(&a.cohort).expect("cohort") == group["cohort"]
            && serde_json::to_value(&a.origin).expect("origin") == group["origin"]
            && serde_json::to_value(&a.task_kind).expect("kind") == group["task_kind"]
        {
            *result.entry(a.input_id.as_str()).or_default() += 1;
        }
    }
    result
}
pub(super) fn compare(
    groups: &[Value],
    assignments: &[Assignment],
    protocol: &Protocol,
) -> Vec<Value> {
    let mut comparisons = Vec::new();
    for candidate in groups
        .iter()
        .filter(|g| g["cohort"]["workflow"] == "qualitygate")
    {
        let cohort: Cohort =
            serde_json::from_value(candidate["cohort"].clone()).expect("typed cohort");
        let baseline = groups.iter().find(|b| {
            b["cohort"]["workflow"] == "existing_tools"
                && b["origin"] == candidate["origin"]
                && b["task_kind"] == candidate["task_kind"]
                && key(&serde_json::from_value(b["cohort"].clone()).expect("typed cohort"))
                    == key(&cohort)
        });
        let matched = baseline
            .is_some_and(|b| inventory(assignments, b) == inventory(assignments, candidate));
        let ratio = |field: &str, stat: &str| -> Option<f64> {
            if !matched {
                return None;
            }
            let b = baseline?[field][stat].as_f64()?;
            let c = candidate[field][stat].as_f64()?;
            (b > 0.0).then_some(c / b)
        };
        let review = ratio("review_active_ms", "median").map(|r| 1.0 - r);
        let time = ratio("full_ms", "p95");
        let cost = if baseline.is_some_and(|b| {
            b["cost"]["currencies"] == candidate["cost"]["currencies"]
                && b["cost"]["priced_at"] == candidate["cost"]["priced_at"]
        }) {
            ratio("cost", "per_accepted_task_micros")
        } else {
            None
        };
        comparisons.push(json!({"cohort":cohort,"origin":candidate["origin"],"task_kind":candidate["task_kind"],
            "matched_inventory":matched,"review_reduction":review,"full_p95_ratio":time,"cost_ratio":cost,
            "threshold_observations":{"review_reduction":review.map(|r|r>=protocol.thresholds.review_reduction_min),
                "full_p95_ratio":time.map(|r|r<=protocol.thresholds.full_p95_ratio_max),
                "cost_ratio":cost.map(|r|r<=protocol.thresholds.cost_ratio_max)},
            "authority":"descriptive_only","limitation":"Matched declared conditions and task inventory permit descriptive comparison; causal attribution and trial acceptance require independent review"}));
    }
    comparisons
}
