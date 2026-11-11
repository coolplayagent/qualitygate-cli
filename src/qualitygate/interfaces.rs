//! Command-line parsing and deterministic report presentation.

pub use crate::agent_runtime::agent_run;
pub(crate) use crate::agent_runtime::obligation_args as agent_contract;
pub use crate::cli_dispatch::cli;
pub(crate) use crate::cli_dispatch::value_parsers;
pub(crate) use crate::pilot_acceptance::pilot_commands as pilot;
pub(crate) use crate::policy_lifecycle::policy_commands as policy;
pub(crate) use crate::policy_lifecycle::rule_lifecycle;
pub(crate) use crate::report_evidence::render;
pub(crate) use crate::snapshot_preflight::doctor_summary;
pub(crate) use crate::snapshot_preflight::readiness_args as doctor;

pub use crate::report_evidence::failure::*;

#[cfg(test)]
#[path = "contract_tests/interfaces_failure.rs"]
mod failure_tests;

#[cfg(test)]
#[path = "contract_tests/interfaces_judgment_pilot.rs"]
mod judgment_pilot_tests;
#[cfg(test)]
#[path = "contract_tests/interfaces_judgment.rs"]
mod judgment_tests;
#[cfg(test)]
#[path = "contract_tests/interfaces_policy_workflow.rs"]
mod policy_workflow_tests;
