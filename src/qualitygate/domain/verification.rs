//! Bounded claims shared by repository checks and fixture regression reports.

use super::{CheckResult, Decision, ExecutionStatus, Gate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationBoundary {
    pub conclusion: String,
    pub verified_shapes: Vec<String>,
    pub known_limits: Vec<String>,
    pub unverified_assumptions: Vec<String>,
}

impl VerificationBoundary {
    pub fn for_check(gate: &Gate, checks: &[CheckResult], profile: &str, scoped: bool) -> Self {
        let mut boundary = Self {
            conclusion: Self::conclusion(gate.decision).into(),
            verified_shapes: checks.iter()
                .filter(|check| check.execution.status == ExecutionStatus::Completed)
                .map(|check| format!("{}: {} matched entities; verdict {:?}", check.id, check.matched_entities, check.verdict))
                .collect(),
            known_limits: vec![
                "Results cover only the selected snapshot, policy, configured tools and observed inputs.".into(),
                "Warning findings remain review signals even when the blocking gate is satisfied.".into(),
                "Fixture regression can falsify known behavior; it cannot prove correctness on a real deployment.".into(),
            ],
            unverified_assumptions: vec![
                "Unrepresented frameworks, runtime configuration delivery, reflection and production environments are not verified.".into(),
                "Custom rules and external tool versions require their own representative regression evidence.".into(),
            ],
        };
        if profile == "quick" || scoped {
            boundary
                .known_limits
                .push("Quick/path checks do not establish full delivery readiness.".into());
        }
        if gate.decision == Decision::Pass
            && checks.iter().any(|check| !check.diagnostics.is_empty())
        {
            boundary.conclusion = "在已验证形态下未发现阻塞问题，仍有待检视诊断 / No blocking problems found; review findings remain".into();
        }
        boundary
    }

    pub fn conclusion(decision: Decision) -> &'static str {
        match decision {
            Decision::Pass => "在已验证形态下未发现问题 / No problems found in the verified shapes",
            Decision::Fail => "在已验证形态下发现问题 / Problems found in the verified shapes",
            Decision::Incomplete => {
                "验证未完成，不能形成无问题结论 / Validation incomplete; no absence-of-problems claim"
            }
        }
    }
}
