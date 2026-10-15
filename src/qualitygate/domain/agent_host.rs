//! Pure host decisions. The caller must own the receipts and execution channel.
use super::{Report, agent_contract::*, constraint, delivery::Delivery, evolution::valid_digest};
use serde::{Deserialize, Serialize};

pub const CONTROLS: &[Control] = &[
    Control::PromptInjection,
    Control::PolicyIntegrity,
    Control::FinalFullCheck,
    Control::ImmutableDelivery,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub session: String,
    pub attempt: u8,
    pub event: Event,
    pub contract_digest: String,
    pub prompt_digest: String,
    pub input_digest: String,
    pub role: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub allowed: bool,
    pub gaps: Vec<String>,
    pub obligations: Vec<ObligationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObligationResult {
    pub id: String,
    pub required: bool,
    pub status: String,
    pub reasons: Vec<String>,
}

pub fn preflight(projection: &Projection) -> Vec<String> {
    let mut gaps = projection.gaps.clone();
    if !projection.ready {
        gaps.push("Current agent contract is not ready for injection".into());
    }
    for b in &projection.obligations {
        if b.obligation.required
            && b.status == Status::Ready
            && let Enforcement::Host { control } = b.obligation.enforcement
            && (!CONTROLS.contains(&control) || b.obligation.triggers.contains(&Event::BeforeTool))
        {
            gaps.push(format!(
                "Obligation {} requires an unavailable host control/event: {control:?}",
                b.obligation.id
            ));
        }
    }
    gaps
}

pub fn authorize(
    current: &Projection,
    injected: &Projection,
    receipt: &Receipt,
    report: &Report,
    delivery: &Delivery,
) -> Decision {
    let mut gaps = preflight(current);
    if current.contract_digest != injected.contract_digest
        || receipt.contract_digest != injected.contract_digest
        || receipt.prompt_digest != injected.prompt_digest
        || receipt.role != current.role
        || receipt.task_id != current.task_id
        || receipt.session.is_empty()
        || receipt.attempt == 0
        || !valid_digest(&receipt.input_digest)
        || !valid_digest(&receipt.prompt_digest)
        || !valid_digest(&receipt.contract_digest)
    {
        gaps.push("Prompt receipt does not match the current consuming-agent contract".into());
    }
    if report.schema_version != 1
        || !report.gate.complete
        || report.gate.decision != super::Decision::Pass
        || report.profile != "full"
        || !["delivery", "task"].contains(&report.scope.as_str())
        || !report.plan.pending_delivery_checks.is_empty()
        || report.plan.task_id != current.task_id
        || !report.policy.changes.is_empty()
        || !current.policy.changes.is_empty()
        || report.policy.config_digest != current.policy.config_digest
        || report.policy.rules_digest != current.policy.rules_digest
        || report.policy.task_contract_digest != current.policy.task_contract_digest
        || report.policy.task_contract_source != current.policy.task_contract_source
        || report.policy.resolved_commit != current.policy.resolved_commit
    {
        gaps.push(
            "Delivery needs complete passing full evidence for the current policy/task".into(),
        );
    }
    let recomputed = super::evaluate(&report.checks, &report.plan.required_checks, &[]);
    if !recomputed.complete || recomputed.decision != super::Decision::Pass {
        gaps.push("Report checks do not establish a complete passing gate".into());
    }
    for identity in [&current.snapshot, &delivery.snapshot] {
        if identity.content_digest != report.snapshot.content_digest
            || identity.verification_digest != report.snapshot.verification_digest
            || identity.base != report.snapshot.base
            || identity.head != report.snapshot.head
            || identity.mode != report.snapshot.mode
        {
            gaps.push("Final report, current projection and delivery snapshot differ".into());
        }
    }
    if delivery.schema_version != 1 || delivery.format != "qualitygate-snapshot-zip-v1" {
        gaps.push("Unsupported immutable delivery object".into());
    }
    if !valid_digest(&delivery.artifact.digest)
        || delivery.artifact.bytes == 0
        || !report
            .snapshot
            .verification_digest
            .as_deref()
            .is_some_and(valid_digest)
    {
        gaps.push("Delivery needs a valid object digest and verification-scope binding".into());
    }
    let mut obligations = Vec::new();
    for b in &current.obligations {
        let o = &b.obligation;
        let (status, reasons) = if b.status != Status::Ready {
            ("not_projected", b.reasons.clone())
        } else {
            match &o.enforcement {
                Enforcement::Instruction => ("delivered_unverified", Vec::new()),
                Enforcement::Checks { checks } => {
                    let (status, reasons) = constraint::execution(checks, &report.checks);
                    if matches!(
                        status,
                        constraint::CoverageStatus::Verified
                            | constraint::CoverageStatus::NotApplicable
                    ) {
                        ("checks_verified", reasons)
                    } else {
                        ("evidence_gap_or_violation", reasons)
                    }
                }
                Enforcement::Host { control }
                    if CONTROLS.contains(control) && !o.triggers.contains(&Event::BeforeTool) =>
                {
                    ("host_control_satisfied", Vec::new())
                }
                Enforcement::Host { .. } => (
                    "unsupported_host_control",
                    vec!["No controlled entry point for this obligation".into()],
                ),
            }
        };
        if o.required
            && ![
                "delivered_unverified",
                "checks_verified",
                "host_control_satisfied",
            ]
            .contains(&status)
        {
            gaps.push(format!("Obligation {}: {status}", o.id));
        }
        obligations.push(ObligationResult {
            id: o.id.clone(),
            required: o.required,
            status: status.into(),
            reasons,
        });
    }
    // Never report a successful host observation when any global binding failed.
    if !gaps.is_empty() {
        for o in &mut obligations {
            if o.status == "host_control_satisfied" {
                o.status = "delivery_blocked".into();
            }
            if o.status == "delivered_unverified" {
                o.status = "delivery_not_established".into();
            }
        }
    }
    Decision {
        allowed: gaps.is_empty(),
        gaps,
        obligations,
    }
}

#[cfg(test)]
#[path = "agent_host_tests.rs"]
mod tests;
