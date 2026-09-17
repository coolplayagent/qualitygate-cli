use super::Manifest;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn audit(manifest: &Manifest) -> (Value, &'static str) {
    let planned: BTreeMap<_, _> = manifest
        .run_order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index + 1))
        .collect();
    let mut observed = Vec::new();
    let mut deviations = Vec::new();
    let mut declared = BTreeSet::new();
    for observation in &manifest.observations {
        if let Some(sequence) = observation.start_sequence {
            let planned_position = planned[observation.assignment_id.as_str()];
            if usize::from(sequence) != planned_position {
                deviations.push(json!({"assignment_id":observation.assignment_id,
                    "planned_position":planned_position,"observed_position":sequence}));
            }
            observed.push((sequence, observation.assignment_id.as_str()));
            declared.insert(observation.assignment_id.as_str());
        }
    }
    observed.sort_unstable();
    let missing: Vec<_> = manifest
        .run_order
        .iter()
        .filter(|id| !declared.contains(id.as_str()))
        .collect();
    let status = if !deviations.is_empty() {
        "deviated"
    } else if !missing.is_empty() {
        "incomplete"
    } else {
        "matched"
    };
    (
        json!({"status":status,"planned":manifest.run_order,
            "observed":observed.iter().map(|(sequence, id)|json!({
                "start_sequence":sequence,"assignment_id":id
            })).collect::<Vec<_>>(),
            "missing":missing,"deviations":deviations}),
        status,
    )
}
