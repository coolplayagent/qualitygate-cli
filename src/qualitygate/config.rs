//! Strict configuration contracts, plan validation and repository discovery.

pub use crate::check_planning::discovery;
pub(crate) use crate::check_planning::initialization;
pub(crate) use crate::check_planning::model;
pub(crate) use crate::check_planning::parallel;
pub(crate) use crate::check_planning::plan;
pub(crate) use crate::check_planning::project_inventory;
pub use crate::check_planning::runtime_validation as runtime;
pub(crate) use crate::check_planning::validation;
pub(crate) use crate::check_planning::yaml_budget;
pub use crate::compatibility_verification::compatibility_spec as compatibility;
pub use crate::constraint_coverage::constraint_inventory;
pub(crate) use crate::constraint_coverage::constraints;
pub use crate::external_acceptance::lineage_validation as case_provenance;
pub use crate::external_acceptance::trust_store as attestation;
pub use crate::judgment_review::policy_loading as judgment;
pub use crate::pilot_acceptance::inventory_loading as pilot;
pub use crate::policy_lifecycle::active_package as policy_active;
pub use crate::policy_lifecycle::candidate_store as policy_candidates;
pub use crate::policy_lifecycle::effectiveness_history as policy_effectiveness;
pub use crate::policy_lifecycle::policy_acceptance;
pub use crate::policy_lifecycle::policy_store;
pub use crate::policy_lifecycle::promotion_store as policy_promotion;
pub use crate::policy_lifecycle::rollback_store as policy_rollback;
pub use crate::policy_lifecycle::source_reviews;
pub use crate::policy_lifecycle::validation_store as policy_validation;
pub use crate::report_evidence::decision_schema;
pub use crate::resource_selection::exclusions;
pub use crate::resource_selection::trigger_selection as change_selection;
pub use crate::rule_catalog::catalog;
pub use crate::rule_catalog::categories;
pub use crate::rule_catalog::parameters;
pub use crate::rule_catalog::project_rules;
pub use crate::rule_catalog::rule_authoring;
pub use crate::rule_catalog::rule_management;
pub use crate::rule_catalog::rule_query;
pub use crate::rule_catalog::rule_schema;
pub use crate::selfcheck::fixture_catalog as selfcheck;
pub use crate::selfcheck::policy_fixtures as selfcheck_policy;
pub use crate::snapshot_preflight::preflight_schema;
pub use crate::test_effectiveness::execution_spec as test_effectiveness;
pub use initialization::{Initialization, initialize_at};
pub use model::*;
pub use plan::{Plan, parse_task};

pub use crate::check_planning::loading::*;

pub use crate::agent_runtime::obligation_validation as agent_contract;
#[cfg(test)]
#[path = "contract_tests/config_loading.rs"]
mod tests;

#[cfg(test)]
#[path = "contract_tests/config_contracts.rs"]
mod contract_scenario_tests;

#[cfg(test)]
#[path = "contract_tests/config_inputs.rs"]
mod input_scenarios;
