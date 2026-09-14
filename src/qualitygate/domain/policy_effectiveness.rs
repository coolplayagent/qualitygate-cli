//! Observed paired outcomes; downstream claims remain separate from gate execution.

use super::policy_evaluation::{Conclusion, EvaluationRun};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Measures {
    pub selected_rule_checks: usize,
    pub applicable_rule_checks: usize,
    pub completed_checks: usize,
    pub findings: usize,
    pub oracle_mismatches: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effectiveness {
    pub evaluation_ref: String,
    pub candidate_policy: String,
    pub baseline_policy: String,
    pub update_quality: Conclusion,
    pub started_at: u64,
    pub ended_at: u64,
    pub complete_pairs: usize,
    pub incomplete_pairs: usize,
    pub baseline: Measures,
    pub candidate: Measures,
    pub oracle_improvement: Option<i64>,
    pub gate_use: &'static str,
    pub downstream_benefit: Option<f64>,
    pub context_tokens: Option<u64>,
    pub review_effort_seconds: Option<u64>,
    pub false_positives: Option<u64>,
}

pub fn measure(reference: String, run: &EvaluationRun) -> Effectiveness {
    let mut baseline = Measures::default();
    let mut candidate = Measures::default();
    let mut complete_pairs = 0;
    for case in &run.cases {
        if case.baseline.complete && case.candidate.complete {
            complete_pairs += 1;
        }
        for (sum, observed) in [
            (&mut baseline, &case.baseline),
            (&mut candidate, &case.candidate),
        ] {
            sum.selected_rule_checks += observed.selected_rules;
            sum.applicable_rule_checks += observed.activated_rules;
            sum.completed_checks += observed.completed_checks;
            sum.findings += observed.findings;
            sum.oracle_mismatches += observed.mismatches.len();
            sum.duration_ms = sum.duration_ms.saturating_add(observed.duration_ms);
        }
    }
    let oracle_improvement = (run.conclusion != Conclusion::Incomplete
        && complete_pairs == run.cases.len())
    .then_some(baseline.oracle_mismatches as i64 - candidate.oracle_mismatches as i64);
    Effectiveness {
        evaluation_ref: reference,
        candidate_policy: run.candidate_policy.clone(),
        baseline_policy: run.baseline_policy.clone(),
        update_quality: run.conclusion,
        started_at: run.started_at,
        ended_at: run.ended_at,
        complete_pairs,
        incomplete_pairs: run.cases.len() - complete_pairs,
        baseline,
        candidate,
        oracle_improvement,
        gate_use: "Observed check execution in protected cases; subsequent agent use is unknown",
        downstream_benefit: None,
        context_tokens: None,
        review_effort_seconds: None,
        false_positives: None,
    }
}
