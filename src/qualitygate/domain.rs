//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

pub(crate) use crate::check_planning::project;
pub use crate::constraint_coverage::constraint;
pub use crate::content_digest as digest;
pub use crate::external_acceptance::case_lineage as case_provenance;
pub(crate) use crate::external_acceptance::manual_contracts as attestation;
pub(crate) use crate::external_acceptance::provenance_contracts as provenance;
pub(crate) use crate::gate_execution::gate;
pub use crate::judgment_review::assessment_contracts as judgment;
pub use crate::pilot_acceptance::pilot_contracts as pilot;
pub use crate::policy_lifecycle::effectiveness_measures as policy_effectiveness;
pub use crate::policy_lifecycle::evolution;
pub use crate::policy_lifecycle::policy_approval;
pub use crate::policy_lifecycle::policy_evaluation;
pub(crate) use crate::policy_lifecycle::policy_input;
pub use crate::policy_lifecycle::rollback_contracts as policy_rollback;
pub(crate) use crate::policy_lifecycle::source_review;
pub use crate::repair_feedback::delivery_contracts as delivery;
pub use crate::repair_feedback::feedback_projection as feedback;
pub use crate::report_evidence::coverage;
pub use crate::report_evidence::decision_envelope;
pub use crate::report_evidence::ratchet;
pub(crate) use crate::report_evidence::report;
pub(crate) use crate::report_evidence::verification;
pub use crate::resource_selection::selection_contracts as change_selection;
pub use crate::resource_selection::snapshot_budget;
pub(crate) use crate::rule_catalog::category;
pub use crate::rule_catalog::language;
pub use crate::rule_catalog::normative;
pub use crate::rule_catalog::operator;
pub use crate::rule_catalog::operator_v3;
pub use crate::rule_catalog::rule_guidance;
pub use crate::rule_catalog::rule_lifecycle;
pub use crate::rule_catalog::rule_parameters;
pub(crate) use crate::rule_catalog::rule_validation;
pub use crate::selfcheck::fixture_results as selfcheck;
pub use crate::xml_parsing::xml_budget;

pub use attestation::*;
pub use category::*;
pub use gate::{Decision, Gate, evaluate};
pub use policy_input::{NextStep, PolicyInputError};
pub use project::*;
pub use provenance::*;
pub use report::*;
pub use rule_validation::*;
pub use source_review::*;
pub use verification::*;

pub use crate::agent_runtime::host_decisions as agent_host;
pub use crate::agent_runtime::obligation_contracts as agent_contract;
pub use crate::check_planning::runtime_contracts as runtime;
pub use crate::gate_execution::check_scope;
pub use crate::snapshot_preflight::readiness_contracts as doctor;
pub use crate::test_effectiveness::counterexample_decisions as test_effectiveness;

pub use crate::resource_selection::resource_exclusions;
