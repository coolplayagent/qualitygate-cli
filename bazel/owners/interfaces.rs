//! Bazel-only entry point for the interfaces capability boundary.

pub use qualitygate_application::application;
pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_snapshot::snapshot;

#[path = "../../src/qualitygate/interfaces.rs"]
pub mod interfaces;

#[path = "../../src/qualitygate/agent_runtime"]
pub mod agent_runtime {
    pub use qualitygate_application::agent_runtime::agent_host_contract;
    pub use qualitygate_application::agent_runtime::agent_host_io;
    pub use qualitygate_application::agent_runtime::host_controller;
    pub use qualitygate_application::agent_runtime::obligation_projection;
    pub use qualitygate_config::agent_runtime::obligation_validation;
    pub use qualitygate_domain::agent_runtime::host_decisions;
    pub use qualitygate_domain::agent_runtime::obligation_contracts;
    #[path = "agent_run.rs"]
    pub mod agent_run;
    #[path = "obligation_args.rs"]
    pub mod obligation_args;
}

#[path = "../../src/qualitygate/check_planning"]
pub mod check_planning {
    pub use qualitygate_config::check_planning::dependencies;
    pub use qualitygate_config::check_planning::discovery;
    pub use qualitygate_config::check_planning::initialization;
    pub use qualitygate_config::check_planning::loading;
    pub use qualitygate_config::check_planning::model;
    pub use qualitygate_config::check_planning::parallel;
    pub use qualitygate_config::check_planning::plan;
    pub use qualitygate_config::check_planning::project_inventory;
    pub use qualitygate_config::check_planning::python;
    pub use qualitygate_config::check_planning::runtime_validation;
    pub use qualitygate_config::check_planning::validation;
    pub use qualitygate_config::check_planning::yaml_budget;
    pub use qualitygate_config::check_planning::yaml_scalar_work;
    pub use qualitygate_domain::check_planning::project;
    pub use qualitygate_domain::check_planning::runtime_contracts;
}

#[path = "../../src/qualitygate/cli_dispatch"]
pub mod cli_dispatch {
    #[path = "cli/mod.rs"]
    pub mod cli;
    #[path = "value_parsers.rs"]
    pub mod value_parsers;
}

#[path = "../../src/qualitygate/compatibility_verification"]
pub mod compatibility_verification {
    pub use qualitygate_application::compatibility_verification::compatibility_inputs;
    pub use qualitygate_application::compatibility_verification::paired_builds;
    pub use qualitygate_config::compatibility_verification::compatibility_spec;
}

#[path = "../../src/qualitygate/constraint_coverage"]
pub mod constraint_coverage {
    pub use qualitygate_application::constraint_coverage::constraint_coverage;
    pub use qualitygate_config::constraint_coverage::constraint_inventory;
    pub use qualitygate_config::constraint_coverage::constraints;
    pub use qualitygate_domain::constraint_coverage::constraint;
}

pub use qualitygate_domain::content_digest;

#[path = "../../src/qualitygate/external_acceptance"]
pub mod external_acceptance {
    pub use qualitygate_application::external_acceptance::evidence;
    pub use qualitygate_application::external_acceptance::external;
    pub use qualitygate_application::external_acceptance::history_acquisition;
    pub use qualitygate_application::external_acceptance::manual;
    pub use qualitygate_application::external_acceptance::provenance_preparation;
    pub use qualitygate_config::external_acceptance::lineage_validation;
    pub use qualitygate_config::external_acceptance::trust_store;
    pub use qualitygate_domain::external_acceptance::case_lineage;
    pub use qualitygate_domain::external_acceptance::manual_contracts;
    pub use qualitygate_domain::external_acceptance::provenance_contracts;
}

#[path = "../../src/qualitygate/gate_execution"]
pub mod gate_execution {
    pub use qualitygate_application::gate_execution::commands;
    pub use qualitygate_application::gate_execution::python_install;
    pub use qualitygate_application::gate_execution::rule_execution;
    pub use qualitygate_application::gate_execution::tool_evidence;
    pub use qualitygate_application::gate_execution::workflow;
    pub use qualitygate_domain::gate_execution::check_scope;
    pub use qualitygate_domain::gate_execution::gate;
}

#[path = "../../src/qualitygate/judgment_review"]
pub mod judgment_review {
    pub use qualitygate_application::judgment_review::assessment_workflow;
    pub use qualitygate_config::judgment_review::policy_loading;
    pub use qualitygate_domain::judgment_review::assessment_contracts;
}

#[path = "../../src/qualitygate/pilot_acceptance"]
pub mod pilot_acceptance {
    pub use qualitygate_application::pilot_acceptance::pilot_workflow;
    pub use qualitygate_config::pilot_acceptance::inventory_loading;
    pub use qualitygate_domain::pilot_acceptance::pilot_contracts;
    #[path = "pilot_commands.rs"]
    pub mod pilot_commands;
}

#[path = "../../src/qualitygate/policy_lifecycle"]
pub mod policy_lifecycle {
    pub use qualitygate_application::policy_lifecycle::active_authentication;
    pub use qualitygate_application::policy_lifecycle::candidate_acquisition;
    pub use qualitygate_application::policy_lifecycle::policy_guidance;
    pub use qualitygate_application::policy_lifecycle::policy_loading;
    pub use qualitygate_application::policy_lifecycle::promotion_workflow;
    pub use qualitygate_application::policy_lifecycle::rollback_workflow;
    pub use qualitygate_application::policy_lifecycle::validation_workflow;
    pub use qualitygate_config::policy_lifecycle::active_package;
    pub use qualitygate_config::policy_lifecycle::candidate_store;
    pub use qualitygate_config::policy_lifecycle::effectiveness_history;
    pub use qualitygate_config::policy_lifecycle::policy_acceptance;
    pub use qualitygate_config::policy_lifecycle::policy_artifacts;
    pub use qualitygate_config::policy_lifecycle::policy_store;
    pub use qualitygate_config::policy_lifecycle::promotion_store;
    pub use qualitygate_config::policy_lifecycle::rollback_store;
    pub use qualitygate_config::policy_lifecycle::source_reviews;
    pub use qualitygate_config::policy_lifecycle::validation_store;
    pub use qualitygate_domain::policy_lifecycle::effectiveness_measures;
    pub use qualitygate_domain::policy_lifecycle::evolution;
    pub use qualitygate_domain::policy_lifecycle::policy_approval;
    pub use qualitygate_domain::policy_lifecycle::policy_evaluation;
    pub use qualitygate_domain::policy_lifecycle::policy_input;
    pub use qualitygate_domain::policy_lifecycle::rollback_contracts;
    pub use qualitygate_domain::policy_lifecycle::source_review;
    #[path = "policy_commands.rs"]
    pub mod policy_commands;
    #[path = "rule_lifecycle.rs"]
    pub mod rule_lifecycle;
}

#[path = "../../src/qualitygate/repair_feedback"]
pub mod repair_feedback {
    pub use qualitygate_application::repair_feedback::feedback_rendering;
    pub use qualitygate_application::repair_feedback::verified_delivery;
    pub use qualitygate_domain::repair_feedback::delivery_contracts;
    pub use qualitygate_domain::repair_feedback::feedback_projection;
}

#[path = "../../src/qualitygate/report_evidence"]
pub mod report_evidence {
    pub use qualitygate_application::report_evidence::coverage_gate;
    pub use qualitygate_application::report_evidence::generated_reports;
    pub use qualitygate_application::report_evidence::project_reports;
    pub use qualitygate_application::report_evidence::report_gate;
    pub use qualitygate_application::report_evidence::report_sources;
    pub use qualitygate_application::report_evidence::test_counts;
    pub use qualitygate_config::report_evidence::decision_schema;
    pub use qualitygate_domain::report_evidence::coverage;
    pub use qualitygate_domain::report_evidence::decision_envelope;
    pub use qualitygate_domain::report_evidence::ratchet;
    pub use qualitygate_domain::report_evidence::report;
    pub use qualitygate_domain::report_evidence::verification;
    #[path = "failure.rs"]
    pub mod failure;
    #[path = "render.rs"]
    pub mod render;
}

#[path = "../../src/qualitygate/resource_selection"]
pub mod resource_selection {
    pub use qualitygate_application::resource_selection::snapshot_selection;
    pub use qualitygate_config::resource_selection::exclusions;
    pub use qualitygate_config::resource_selection::trigger_selection;
    pub use qualitygate_domain::resource_selection::resource_exclusions;
    pub use qualitygate_domain::resource_selection::selection_contracts;
    pub use qualitygate_domain::resource_selection::snapshot_budget;
}

#[path = "../../src/qualitygate/rule_catalog"]
pub mod rule_catalog {
    pub use qualitygate_application::rule_catalog::rule_context;
    pub use qualitygate_config::rule_catalog::builtin_compatibility;
    pub use qualitygate_config::rule_catalog::builtin_validation;
    pub use qualitygate_config::rule_catalog::catalog;
    pub use qualitygate_config::rule_catalog::catalog_assets;
    pub use qualitygate_config::rule_catalog::categories;
    pub use qualitygate_config::rule_catalog::custom_validation;
    pub use qualitygate_config::rule_catalog::parameters;
    pub use qualitygate_config::rule_catalog::project_rules;
    pub use qualitygate_config::rule_catalog::rule_authoring;
    pub use qualitygate_config::rule_catalog::rule_management;
    pub use qualitygate_config::rule_catalog::rule_query;
    pub use qualitygate_config::rule_catalog::rule_schema;
    pub use qualitygate_domain::rule_catalog::category;
    pub use qualitygate_domain::rule_catalog::language;
    pub use qualitygate_domain::rule_catalog::normative;
    pub use qualitygate_domain::rule_catalog::operator;
    pub use qualitygate_domain::rule_catalog::operator_v3;
    pub use qualitygate_domain::rule_catalog::rule_guidance;
    pub use qualitygate_domain::rule_catalog::rule_lifecycle;
    pub use qualitygate_domain::rule_catalog::rule_parameters;
    pub use qualitygate_domain::rule_catalog::rule_validation;
}

#[path = "../../src/qualitygate/selfcheck"]
pub mod selfcheck {
    pub use qualitygate_application::selfcheck::fixture_execution;
    pub use qualitygate_application::selfcheck::policy_oracles;
    pub use qualitygate_application::selfcheck::selfcheck_evidence;
    pub use qualitygate_application::selfcheck::selfcheck_io;
    pub use qualitygate_application::selfcheck::selfcheck_policy_io;
    pub use qualitygate_config::selfcheck::fixture_catalog;
    pub use qualitygate_config::selfcheck::policy_fixtures;
    pub use qualitygate_domain::selfcheck::fixture_results;
}

#[path = "../../src/qualitygate/snapshot_preflight"]
pub mod snapshot_preflight {
    pub use qualitygate_application::snapshot_preflight::acquisition;
    pub use qualitygate_application::snapshot_preflight::capabilities;
    pub use qualitygate_application::snapshot_preflight::doctor_context;
    pub use qualitygate_application::snapshot_preflight::doctor_output;
    pub use qualitygate_application::snapshot_preflight::doctor_plan;
    pub use qualitygate_application::snapshot_preflight::readiness_check;
    pub use qualitygate_application::snapshot_preflight::repository_root;
    pub use qualitygate_config::snapshot_preflight::preflight_schema;
    pub use qualitygate_domain::snapshot_preflight::readiness_contracts;
    #[path = "doctor_summary.rs"]
    pub mod doctor_summary;
    #[path = "readiness_args.rs"]
    pub mod readiness_args;
}

#[path = "../../src/qualitygate/test_effectiveness"]
pub mod test_effectiveness {
    pub use qualitygate_application::test_effectiveness::paired_execution;
    pub use qualitygate_config::test_effectiveness::execution_spec;
    pub use qualitygate_domain::test_effectiveness::counterexample_decisions;
}

#[path = "../../src/qualitygate/xml_parsing"]
pub mod xml_parsing {
    pub use qualitygate_domain::xml_parsing::xml_budget;
}
