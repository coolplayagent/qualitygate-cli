//! Coordinates policy selection, snapshot execution and evidence-bound reports.

pub use crate::agent_runtime::host_controller as agent_host;
pub use crate::agent_runtime::obligation_projection as agent_contract;
pub use crate::check_inputs::initialization_preflight;
pub use crate::check_planning::check_request::CheckOptions;
pub use crate::check_preflight::capabilities;
pub use crate::check_preflight::doctor_output;
pub use crate::check_preflight::readiness_check as doctor;
pub use crate::check_preflight::repository_root;
pub(crate) use crate::compatibility_verification::paired_builds as compatibility;
pub(crate) use crate::constraint_coverage::coverage_evaluation as constraint_coverage;
pub use crate::delivery_export::verified_delivery as delivery;
pub(crate) use crate::external_acceptance::evidence;
pub(crate) use crate::external_acceptance::external;
pub(crate) use crate::external_acceptance::history_acquisition as git_trailers;
pub(crate) use crate::external_acceptance::manual;
pub(crate) use crate::gate_execution::commands;
pub(crate) use crate::gate_execution::python_install;
pub(crate) use crate::gate_execution::tool_evidence;
pub use crate::judgment_review::assessment_workflow as judgment;
pub use crate::pilot_acceptance::pilot_workflow as pilot;
pub use crate::policy_lifecycle::active_authentication as policy_active;
pub use crate::policy_lifecycle::candidate_acquisition as policy_candidates;
pub use crate::policy_lifecycle::policy_guidance;
pub(crate) use crate::policy_lifecycle::policy_loading as policy;
pub use crate::policy_lifecycle::promotion_workflow as policy_promotion;
pub use crate::policy_lifecycle::rollback_workflow as policy_rollback;
pub use crate::policy_lifecycle::validation_workflow as policy_validation;
pub use crate::repair_feedback::feedback_rendering as feedback;
pub(crate) use crate::report_evidence::generated_reports;
pub(crate) use crate::report_evidence::project_reports;
pub(crate) use crate::report_evidence::test_counts;
pub use crate::rule_catalog::rule_context;
pub use crate::selfcheck::fixture_execution as selfcheck;

#[cfg(test)]
#[path = "contract_tests/application_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "contract_tests/application_orchestration.rs"]
mod tests;

pub use crate::gate_execution::workflow::*;

#[cfg(test)]
#[path = "contract_tests/application_policy.rs"]
mod project_policy_tests;

pub(crate) use crate::domain::*;
#[cfg(test)]
pub(crate) use crate::{config, snapshot};
#[cfg(test)]
pub(crate) use std::sync::Arc;
