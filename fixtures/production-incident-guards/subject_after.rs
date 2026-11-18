// Repaired subject. Validate before publishing and recheck generation at commit.
use super::{Plan, State};
use std::collections::BTreeSet;

pub fn publish_features(active: &mut Vec<u8>, candidate: &[u8], limit: usize) {
    if candidate.len() <= limit
        && candidate.iter().collect::<BTreeSet<_>>().len() == candidate.len()
    {
        *active = candidate.to_vec();
    }
}

pub fn apply_plan(state: &mut State, plan: Plan) {
    if plan.eligible_at_prepare
        && plan.generation > state.active
        && state.retained.contains(&plan.generation)
    {
        state.active = plan.generation;
    }
}
