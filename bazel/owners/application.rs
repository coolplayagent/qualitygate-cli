//! Bazel-only entry point for the application capability boundary.

pub use qualitygate_adapters::adapters;
pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_env::env;
pub use qualitygate_net::net;
pub use qualitygate_paths::paths;
pub use qualitygate_runner::runner;
pub use qualitygate_snapshot::snapshot;

#[path = "../../src/qualitygate/application.rs"]
pub mod application;

#[path = "../../src/qualitygate/agent_runtime"]
pub mod agent_runtime {
    #[path = "agent_host_contract.rs"]
    pub mod agent_host_contract;
    #[path = "agent_host_io.rs"]
    pub mod agent_host_io;
    #[path = "host_controller.rs"]
    pub mod host_controller;
    #[path = "obligation_projection.rs"]
    pub mod obligation_projection;
    pub use qualitygate_config::agent_runtime::obligation_validation;
    pub use qualitygate_domain::agent_runtime::host_decisions;
    pub use qualitygate_domain::agent_runtime::obligation_contracts;
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

#[path = "../../src/qualitygate/compatibility_verification"]
pub mod compatibility_verification {
    pub use qualitygate_adapters::compatibility_verification::verifier;
    #[path = "compatibility_inputs.rs"]
    pub mod compatibility_inputs;
    #[path = "paired_builds.rs"]
    pub mod paired_builds;
    pub use qualitygate_config::compatibility_verification::compatibility_spec;
}

#[path = "../../src/qualitygate/constraint_coverage"]
pub mod constraint_coverage {
    #[path = "coverage_evaluation.rs"]
    pub mod coverage_evaluation;
    pub use qualitygate_config::constraint_coverage::constraint_inventory;
    pub use qualitygate_config::constraint_coverage::constraints;
    pub use qualitygate_domain::constraint_coverage::constraint;
}

pub use qualitygate_domain::content_digest;

#[path = "../../src/qualitygate/external_acceptance"]
pub mod external_acceptance {
    pub use qualitygate_adapters::external_acceptance::provenance_verification;
    pub use qualitygate_adapters::external_acceptance::signature_verification;
    pub use qualitygate_adapters::external_acceptance::trailer_analysis;
    #[path = "evidence.rs"]
    pub mod evidence;
    #[path = "external.rs"]
    pub mod external;
    #[path = "history_acquisition.rs"]
    pub mod history_acquisition;
    #[path = "manual.rs"]
    pub mod manual;
    #[path = "provenance_preparation.rs"]
    pub mod provenance_preparation;
    pub use qualitygate_config::external_acceptance::lineage_validation;
    pub use qualitygate_config::external_acceptance::trust_store;
    pub use qualitygate_domain::external_acceptance::case_lineage;
    pub use qualitygate_domain::external_acceptance::manual_contracts;
    pub use qualitygate_domain::external_acceptance::provenance_contracts;
}

#[path = "../../src/qualitygate/gate_execution"]
pub mod gate_execution {
    #[path = "commands.rs"]
    pub mod commands;
    #[path = "python_install.rs"]
    pub mod python_install;
    #[path = "rule_execution.rs"]
    pub mod rule_execution;
    #[path = "tool_evidence.rs"]
    pub mod tool_evidence;
    #[path = "workflow.rs"]
    pub mod workflow;
    pub use qualitygate_domain::gate_execution::check_scope;
    pub use qualitygate_domain::gate_execution::gate;
}

#[path = "../../src/qualitygate/judgment_review"]
pub mod judgment_review {
    #[path = "assessment_workflow/mod.rs"]
    pub mod assessment_workflow;
    pub use qualitygate_config::judgment_review::policy_loading;
    pub use qualitygate_domain::judgment_review::assessment_contracts;
}

#[path = "../../src/qualitygate/maven_analysis"]
pub mod maven_analysis {
    pub use qualitygate_adapters::maven_analysis::dependency_usage;
    pub use qualitygate_adapters::maven_analysis::project_model;
}

#[path = "../../src/qualitygate/pilot_acceptance"]
pub mod pilot_acceptance {
    pub use qualitygate_adapters::pilot_acceptance::acceptance;
    pub use qualitygate_adapters::pilot_acceptance::authorization;
    #[path = "pilot_workflow.rs"]
    pub mod pilot_workflow;
    pub use qualitygate_config::pilot_acceptance::inventory_loading;
    pub use qualitygate_domain::pilot_acceptance::pilot_contracts;
}

#[path = "../../src/qualitygate/policy_lifecycle"]
pub mod policy_lifecycle {
    pub use qualitygate_adapters::policy_lifecycle::approval;
    pub use qualitygate_adapters::policy_lifecycle::rollback;
    #[path = "active_authentication.rs"]
    pub mod active_authentication;
    #[path = "candidate_acquisition.rs"]
    pub mod candidate_acquisition;
    #[path = "policy_guidance.rs"]
    pub mod policy_guidance;
    #[path = "policy_loading.rs"]
    pub mod policy_loading;
    #[path = "promotion_workflow.rs"]
    pub mod promotion_workflow;
    #[path = "rollback_workflow.rs"]
    pub mod rollback_workflow;
    #[path = "validation_workflow.rs"]
    pub mod validation_workflow;
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
}

#[path = "../../src/qualitygate/python_analysis"]
pub mod python_analysis {
    pub use qualitygate_adapters::python_analysis::package_metadata;
    pub use qualitygate_adapters::python_analysis::project_model;
}

#[path = "../../src/qualitygate/repair_feedback"]
pub mod repair_feedback {
    #[path = "feedback_rendering.rs"]
    pub mod feedback_rendering;
    #[path = "verified_delivery.rs"]
    pub mod verified_delivery;
    pub use qualitygate_domain::repair_feedback::delivery_contracts;
    pub use qualitygate_domain::repair_feedback::feedback_projection;
}

#[path = "../../src/qualitygate/report_evidence"]
pub mod report_evidence {
    pub use qualitygate_adapters::report_evidence::diagnostic_budget;
    pub use qualitygate_adapters::report_evidence::tool_reports;
    #[path = "coverage_gate.rs"]
    pub mod coverage_gate;
    #[path = "generated_reports.rs"]
    pub mod generated_reports;
    #[path = "project_reports.rs"]
    pub mod project_reports;
    #[path = "report_gate.rs"]
    pub mod report_gate;
    #[path = "report_sources.rs"]
    pub mod report_sources;
    #[path = "test_counts.rs"]
    pub mod test_counts;
    pub use qualitygate_config::report_evidence::decision_schema;
    pub use qualitygate_domain::report_evidence::coverage;
    pub use qualitygate_domain::report_evidence::decision_envelope;
    pub use qualitygate_domain::report_evidence::ratchet;
    pub use qualitygate_domain::report_evidence::report;
    pub use qualitygate_domain::report_evidence::verification;
}

#[path = "../../src/qualitygate/resource_selection"]
pub mod resource_selection {
    #[path = "snapshot_selection.rs"]
    pub mod snapshot_selection;
    pub use qualitygate_config::resource_selection::exclusions;
    pub use qualitygate_config::resource_selection::trigger_selection;
    pub use qualitygate_domain::resource_selection::resource_exclusions;
    pub use qualitygate_domain::resource_selection::selection_contracts;
    pub use qualitygate_domain::resource_selection::snapshot_budget;
}

#[path = "../../src/qualitygate/rule_catalog"]
pub mod rule_catalog {
    #[path = "rule_context.rs"]
    pub mod rule_context;
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

#[path = "../../src/qualitygate/rule_evaluation"]
pub mod rule_evaluation {
    pub use qualitygate_adapters::rule_evaluation::builtin_conventions;
    pub use qualitygate_adapters::rule_evaluation::builtin_rules;
    pub use qualitygate_adapters::rule_evaluation::custom_rules;
    pub use qualitygate_adapters::rule_evaluation::evaluation_facts;
    pub use qualitygate_adapters::rule_evaluation::language_syntax;
    pub use qualitygate_adapters::rule_evaluation::markers;
    pub use qualitygate_adapters::rule_evaluation::native_syntax;
    pub use qualitygate_adapters::rule_evaluation::operators_v2;
    pub use qualitygate_adapters::rule_evaluation::operators_v3;
    pub use qualitygate_adapters::rule_evaluation::project_checks;
    pub use qualitygate_adapters::rule_evaluation::shell_conventions;
    pub use qualitygate_adapters::rule_evaluation::structure_rules;
}

#[path = "../../src/qualitygate/selfcheck"]
pub mod selfcheck {
    #[path = "fixture_execution.rs"]
    pub mod fixture_execution;
    #[path = "policy_oracles.rs"]
    pub mod policy_oracles;
    #[path = "selfcheck_evidence.rs"]
    pub mod selfcheck_evidence;
    #[path = "selfcheck_io.rs"]
    pub mod selfcheck_io;
    #[path = "selfcheck_policy_io.rs"]
    pub mod selfcheck_policy_io;
    pub use qualitygate_config::selfcheck::fixture_catalog;
    pub use qualitygate_config::selfcheck::policy_fixtures;
    pub use qualitygate_domain::selfcheck::fixture_results;
}

#[path = "../../src/qualitygate/snapshot_preflight"]
pub mod snapshot_preflight {
    #[path = "acquisition.rs"]
    pub mod acquisition;
    #[path = "capabilities.rs"]
    pub mod capabilities;
    #[path = "doctor_context.rs"]
    pub mod doctor_context;
    #[path = "doctor_output.rs"]
    pub mod doctor_output;
    #[path = "doctor_plan.rs"]
    pub mod doctor_plan;
    #[path = "readiness_check.rs"]
    pub mod readiness_check;
    #[path = "repository_root.rs"]
    pub mod repository_root;
    pub use qualitygate_config::snapshot_preflight::preflight_schema;
    pub use qualitygate_domain::snapshot_preflight::readiness_contracts;
}

#[path = "../../src/qualitygate/source_analysis"]
pub mod source_analysis {
    pub use qualitygate_adapters::source_analysis::analysis_workers;
    pub use qualitygate_adapters::source_analysis::entity_changes;
    pub use qualitygate_adapters::source_analysis::language_bindings;
    pub use qualitygate_adapters::source_analysis::language_shell;
    pub use qualitygate_adapters::source_analysis::syntax;
}

#[path = "../../src/qualitygate/test_effectiveness"]
pub mod test_effectiveness {
    #[path = "paired_execution.rs"]
    pub mod paired_execution;
    pub use qualitygate_config::test_effectiveness::execution_spec;
    pub use qualitygate_domain::test_effectiveness::counterexample_decisions;
}

#[path = "../../src/qualitygate/xml_parsing"]
pub mod xml_parsing {
    pub use qualitygate_adapters::xml_parsing::document;
    pub use qualitygate_domain::xml_parsing::xml_budget;
}
