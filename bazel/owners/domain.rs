//! Bazel-only entry point for the domain capability boundary.

#[path = "../../src/qualitygate/domain.rs"]
pub mod domain;

#[path = "../../src/qualitygate/agent_runtime"]
pub mod agent_runtime {
    #[path = "host_decisions.rs"]
    pub mod host_decisions;
    #[path = "obligation_contracts.rs"]
    pub mod obligation_contracts;
}

#[path = "../../src/qualitygate/check_planning"]
pub mod check_planning {
    #[path = "project.rs"]
    pub mod project;
    #[path = "runtime_contracts.rs"]
    pub mod runtime_contracts;
}

#[path = "../../src/qualitygate/constraint_coverage"]
pub mod constraint_coverage {
    #[path = "constraint.rs"]
    pub mod constraint;
}

#[path = "../../src/qualitygate/content_digest.rs"]
pub mod content_digest;

#[path = "../../src/qualitygate/external_acceptance"]
pub mod external_acceptance {
    #[path = "case_lineage.rs"]
    pub mod case_lineage;
    #[path = "manual_contracts.rs"]
    pub mod manual_contracts;
    #[path = "provenance_contracts.rs"]
    pub mod provenance_contracts;
}

#[path = "../../src/qualitygate/gate_execution"]
pub mod gate_execution {
    #[path = "check_scope.rs"]
    pub mod check_scope;
    #[path = "gate.rs"]
    pub mod gate;
}

#[path = "../../src/qualitygate/judgment_review"]
pub mod judgment_review {
    #[path = "assessment_contracts.rs"]
    pub mod assessment_contracts;
}

#[path = "../../src/qualitygate/pilot_acceptance"]
pub mod pilot_acceptance {
    #[path = "pilot_contracts/mod.rs"]
    pub mod pilot_contracts;
}

#[path = "../../src/qualitygate/policy_lifecycle"]
pub mod policy_lifecycle {
    #[path = "effectiveness_measures.rs"]
    pub mod effectiveness_measures;
    #[path = "evolution.rs"]
    pub mod evolution;
    #[path = "policy_approval.rs"]
    pub mod policy_approval;
    #[path = "policy_evaluation.rs"]
    pub mod policy_evaluation;
    #[path = "policy_input.rs"]
    pub mod policy_input;
    #[path = "rollback_contracts.rs"]
    pub mod rollback_contracts;
    #[path = "source_review.rs"]
    pub mod source_review;
}

#[path = "../../src/qualitygate/repair_feedback"]
pub mod repair_feedback {
    #[path = "delivery_contracts.rs"]
    pub mod delivery_contracts;
    #[path = "feedback_projection.rs"]
    pub mod feedback_projection;
}

#[path = "../../src/qualitygate/report_evidence"]
pub mod report_evidence {
    #[path = "coverage.rs"]
    pub mod coverage;
    #[path = "decision_envelope.rs"]
    pub mod decision_envelope;
    #[path = "ratchet.rs"]
    pub mod ratchet;
    #[path = "report.rs"]
    pub mod report;
    #[path = "verification.rs"]
    pub mod verification;
}

#[path = "../../src/qualitygate/resource_selection"]
pub mod resource_selection {
    #[path = "resource_exclusions.rs"]
    pub mod resource_exclusions;
    #[path = "selection_contracts.rs"]
    pub mod selection_contracts;
    #[path = "snapshot_budget.rs"]
    pub mod snapshot_budget;
}

#[path = "../../src/qualitygate/rule_catalog"]
pub mod rule_catalog {
    #[path = "category.rs"]
    pub mod category;
    #[path = "language.rs"]
    pub mod language;
    #[path = "normative.rs"]
    pub mod normative;
    #[path = "operator.rs"]
    pub mod operator;
    #[path = "operator_v3.rs"]
    pub mod operator_v3;
    #[path = "rule_guidance.rs"]
    pub mod rule_guidance;
    #[path = "rule_lifecycle.rs"]
    pub mod rule_lifecycle;
    #[path = "rule_parameters.rs"]
    pub mod rule_parameters;
    #[path = "rule_validation.rs"]
    pub mod rule_validation;
}

#[path = "../../src/qualitygate/selfcheck"]
pub mod selfcheck {
    #[path = "fixture_results.rs"]
    pub mod fixture_results;
}

#[path = "../../src/qualitygate/snapshot_preflight"]
pub mod snapshot_preflight {
    #[path = "readiness_contracts.rs"]
    pub mod readiness_contracts;
}

#[path = "../../src/qualitygate/test_effectiveness"]
pub mod test_effectiveness {
    #[path = "counterexample_decisions.rs"]
    pub mod counterexample_decisions;
}

#[path = "../../src/qualitygate/xml_parsing"]
pub mod xml_parsing {
    #[path = "xml_budget.rs"]
    pub mod xml_budget;
}
