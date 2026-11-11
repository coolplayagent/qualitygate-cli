//! Bazel-only composition crate that preserves the public Cargo module paths.

pub use qualitygate_adapters::adapters;
pub use qualitygate_application::application;
pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_env::env;
pub use qualitygate_interfaces::interfaces;
pub use qualitygate_net::net;
pub use qualitygate_paths::paths;
pub use qualitygate_runner::runner;
pub use qualitygate_snapshot::snapshot;

pub mod agent_runtime {
    pub use qualitygate_application::agent_runtime::host_controller;
    pub use qualitygate_application::agent_runtime::obligation_projection;
    pub use qualitygate_config::agent_runtime::obligation_validation;
    pub use qualitygate_domain::agent_runtime::host_decisions;
    pub use qualitygate_domain::agent_runtime::obligation_contracts;
    pub use qualitygate_interfaces::agent_runtime::agent_run;
}

pub mod check_planning {
    pub use qualitygate_config::check_planning::discovery;
    pub use qualitygate_config::check_planning::runtime_validation;
    pub use qualitygate_domain::check_planning::runtime_contracts;
}

pub mod cli_dispatch {
    pub use qualitygate_interfaces::cli_dispatch::cli;
}

pub mod compatibility_verification {
    pub use qualitygate_adapters::compatibility_verification::verifier;
    pub use qualitygate_config::compatibility_verification::compatibility_spec;
}

pub mod constraint_coverage {
    pub use qualitygate_config::constraint_coverage::constraint_inventory;
    pub use qualitygate_domain::constraint_coverage::constraint;
}

pub use qualitygate_domain::content_digest;

pub mod external_acceptance {
    pub use qualitygate_adapters::external_acceptance::provenance_verification;
    pub use qualitygate_adapters::external_acceptance::signature_verification;
    pub use qualitygate_adapters::external_acceptance::trailer_analysis;
    pub use qualitygate_config::external_acceptance::lineage_validation;
    pub use qualitygate_config::external_acceptance::trust_store;
    pub use qualitygate_domain::external_acceptance::case_lineage;
}

pub mod gate_execution {
    pub use qualitygate_domain::gate_execution::check_scope;
}

pub mod judgment_review {
    pub use qualitygate_application::judgment_review::assessment_workflow;
    pub use qualitygate_config::judgment_review::policy_loading;
    pub use qualitygate_domain::judgment_review::assessment_contracts;
}

pub mod maven_analysis {
    pub use qualitygate_adapters::maven_analysis::dependency_usage;
    pub use qualitygate_adapters::maven_analysis::project_model;
}

pub mod pilot_acceptance {
    pub use qualitygate_adapters::pilot_acceptance::acceptance;
    pub use qualitygate_adapters::pilot_acceptance::authorization;
    pub use qualitygate_application::pilot_acceptance::pilot_workflow;
    pub use qualitygate_config::pilot_acceptance::inventory_loading;
    pub use qualitygate_domain::pilot_acceptance::pilot_contracts;
}

pub mod policy_lifecycle {
    pub use qualitygate_adapters::policy_lifecycle::approval;
    pub use qualitygate_adapters::policy_lifecycle::rollback;
    pub use qualitygate_application::policy_lifecycle::active_authentication;
    pub use qualitygate_application::policy_lifecycle::candidate_acquisition;
    pub use qualitygate_application::policy_lifecycle::policy_guidance;
    pub use qualitygate_application::policy_lifecycle::promotion_workflow;
    pub use qualitygate_application::policy_lifecycle::rollback_workflow;
    pub use qualitygate_application::policy_lifecycle::validation_workflow;
    pub use qualitygate_config::policy_lifecycle::active_package;
    pub use qualitygate_config::policy_lifecycle::candidate_store;
    pub use qualitygate_config::policy_lifecycle::effectiveness_history;
    pub use qualitygate_config::policy_lifecycle::policy_acceptance;
    pub use qualitygate_config::policy_lifecycle::policy_store;
    pub use qualitygate_config::policy_lifecycle::promotion_store;
    pub use qualitygate_config::policy_lifecycle::rollback_store;
    pub use qualitygate_config::policy_lifecycle::source_reviews;
    pub use qualitygate_config::policy_lifecycle::validation_store;
    pub use qualitygate_domain::policy_lifecycle::effectiveness_measures;
    pub use qualitygate_domain::policy_lifecycle::evolution;
    pub use qualitygate_domain::policy_lifecycle::policy_approval;
    pub use qualitygate_domain::policy_lifecycle::policy_evaluation;
    pub use qualitygate_domain::policy_lifecycle::rollback_contracts;
}

pub mod python_analysis {
    pub use qualitygate_adapters::python_analysis::project_model;
}

pub mod repair_feedback {
    pub use qualitygate_application::repair_feedback::feedback_rendering;
    pub use qualitygate_application::repair_feedback::verified_delivery;
    pub use qualitygate_domain::repair_feedback::delivery_contracts;
    pub use qualitygate_domain::repair_feedback::feedback_projection;
}

pub mod report_evidence {
    pub use qualitygate_adapters::report_evidence::diagnostic_budget;
    pub use qualitygate_adapters::report_evidence::tool_reports;
    pub use qualitygate_config::report_evidence::decision_schema;
    pub use qualitygate_domain::report_evidence::coverage;
    pub use qualitygate_domain::report_evidence::decision_envelope;
    pub use qualitygate_domain::report_evidence::ratchet;
}

pub mod resource_selection {
    pub use qualitygate_config::resource_selection::exclusions;
    pub use qualitygate_config::resource_selection::trigger_selection;
    pub use qualitygate_domain::resource_selection::resource_exclusions;
    pub use qualitygate_domain::resource_selection::selection_contracts;
    pub use qualitygate_domain::resource_selection::snapshot_budget;
}

pub mod rule_catalog {
    pub use qualitygate_application::rule_catalog::rule_context;
    pub use qualitygate_config::rule_catalog::catalog;
    pub use qualitygate_config::rule_catalog::categories;
    pub use qualitygate_config::rule_catalog::parameters;
    pub use qualitygate_config::rule_catalog::project_rules;
    pub use qualitygate_config::rule_catalog::rule_authoring;
    pub use qualitygate_config::rule_catalog::rule_management;
    pub use qualitygate_config::rule_catalog::rule_query;
    pub use qualitygate_config::rule_catalog::rule_schema;
    pub use qualitygate_domain::rule_catalog::language;
    pub use qualitygate_domain::rule_catalog::normative;
    pub use qualitygate_domain::rule_catalog::operator;
    pub use qualitygate_domain::rule_catalog::operator_v3;
    pub use qualitygate_domain::rule_catalog::rule_guidance;
    pub use qualitygate_domain::rule_catalog::rule_lifecycle;
    pub use qualitygate_domain::rule_catalog::rule_parameters;
}

pub mod rule_evaluation {
    pub use qualitygate_adapters::rule_evaluation::builtin_rules;
    pub use qualitygate_adapters::rule_evaluation::custom_rules;
    pub use qualitygate_adapters::rule_evaluation::evaluation_facts;
    pub use qualitygate_adapters::rule_evaluation::operators_v2;
    pub use qualitygate_adapters::rule_evaluation::operators_v3;
}

pub mod selfcheck {
    pub use qualitygate_application::selfcheck::fixture_execution;
    pub use qualitygate_config::selfcheck::fixture_catalog;
    pub use qualitygate_config::selfcheck::policy_fixtures;
    pub use qualitygate_domain::selfcheck::fixture_results;
}

pub mod snapshot_preflight {
    pub use qualitygate_application::snapshot_preflight::capabilities;
    pub use qualitygate_application::snapshot_preflight::doctor_output;
    pub use qualitygate_application::snapshot_preflight::readiness_check;
    pub use qualitygate_application::snapshot_preflight::repository_root;
    pub use qualitygate_config::snapshot_preflight::preflight_schema;
    pub use qualitygate_domain::snapshot_preflight::readiness_contracts;
}

pub mod source_analysis {
    pub use qualitygate_adapters::source_analysis::syntax;
}

pub mod test_effectiveness {
    pub use qualitygate_config::test_effectiveness::execution_spec;
    pub use qualitygate_domain::test_effectiveness::counterexample_decisions;
}

pub mod xml_parsing {
    pub use qualitygate_domain::xml_parsing::xml_budget;
}
