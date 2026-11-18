// Synthetic pre-fix subject. Candidate data and prepared plans are supplied by the harness.
use super::{Plan, State};

pub fn publish_features(active: &mut Vec<u8>, candidate: &[u8], _limit: usize) {
    *active = candidate.to_vec();
}

pub fn apply_plan(state: &mut State, plan: Plan) {
    if plan.eligible_at_prepare {
        state.active = plan.generation;
    }
}
