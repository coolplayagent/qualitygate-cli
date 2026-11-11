//! Bazel-only entry point for the adapters capability boundary.

pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_paths::paths;
pub use qualitygate_snapshot::snapshot;

#[path = "../../src/qualitygate/adapters.rs"]
pub mod adapters;

#[path = "../../src/qualitygate/agent_runtime"]
pub mod agent_runtime {
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
    #[path = "verifier.rs"]
    pub mod verifier;
    pub use qualitygate_config::compatibility_verification::compatibility_spec;
}

#[path = "../../src/qualitygate/constraint_coverage"]
pub mod constraint_coverage {
    pub use qualitygate_config::constraint_coverage::constraint_inventory;
    pub use qualitygate_config::constraint_coverage::constraints;
    pub use qualitygate_domain::constraint_coverage::constraint;
}

pub use qualitygate_domain::content_digest;

#[path = "../../src/qualitygate/external_acceptance"]
pub mod external_acceptance {
    #[path = "provenance_verification.rs"]
    pub mod provenance_verification;
    #[path = "signature_verification.rs"]
    pub mod signature_verification;
    #[path = "trailer_analysis.rs"]
    pub mod trailer_analysis;
    pub use qualitygate_config::external_acceptance::lineage_validation;
    pub use qualitygate_config::external_acceptance::trust_store;
    pub use qualitygate_domain::external_acceptance::case_lineage;
    pub use qualitygate_domain::external_acceptance::manual_contracts;
    pub use qualitygate_domain::external_acceptance::provenance_contracts;
}

#[path = "../../src/qualitygate/gate_execution"]
pub mod gate_execution {
    pub use qualitygate_domain::gate_execution::check_scope;
    pub use qualitygate_domain::gate_execution::gate;
}

#[path = "../../src/qualitygate/judgment_review"]
pub mod judgment_review {
    pub use qualitygate_config::judgment_review::policy_loading;
    pub use qualitygate_domain::judgment_review::assessment_contracts;
}

#[path = "../../src/qualitygate/maven_analysis"]
pub mod maven_analysis {
    #[path = "dependency_usage.rs"]
    pub mod dependency_usage;
    #[path = "project_model.rs"]
    pub mod project_model;
}

#[path = "../../src/qualitygate/pilot_acceptance"]
pub mod pilot_acceptance {
    #[path = "acceptance.rs"]
    pub mod acceptance;
    #[path = "authorization.rs"]
    pub mod authorization;
    pub use qualitygate_config::pilot_acceptance::inventory_loading;
    pub use qualitygate_domain::pilot_acceptance::pilot_contracts;
}

#[path = "../../src/qualitygate/policy_lifecycle"]
pub mod policy_lifecycle {
    #[path = "approval.rs"]
    pub mod approval;
    #[path = "rollback.rs"]
    pub mod rollback;
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
    #[path = "package_metadata.rs"]
    pub mod package_metadata;
    #[path = "project_model.rs"]
    pub mod project_model;
}

#[path = "../../src/qualitygate/repair_feedback"]
pub mod repair_feedback {
    pub use qualitygate_domain::repair_feedback::delivery_contracts;
    pub use qualitygate_domain::repair_feedback::feedback_projection;
}

#[path = "../../src/qualitygate/report_evidence"]
pub mod report_evidence {
    #[path = "diagnostic_budget.rs"]
    pub mod diagnostic_budget;
    #[path = "tool_reports/mod.rs"]
    pub mod tool_reports;
    pub use qualitygate_config::report_evidence::decision_schema;
    pub use qualitygate_domain::report_evidence::coverage;
    pub use qualitygate_domain::report_evidence::decision_envelope;
    pub use qualitygate_domain::report_evidence::ratchet;
    pub use qualitygate_domain::report_evidence::report;
    pub use qualitygate_domain::report_evidence::verification;
}

#[path = "../../src/qualitygate/resource_selection"]
pub mod resource_selection {
    pub use qualitygate_config::resource_selection::exclusions;
    pub use qualitygate_config::resource_selection::trigger_selection;
    pub use qualitygate_domain::resource_selection::resource_exclusions;
    pub use qualitygate_domain::resource_selection::selection_contracts;
    pub use qualitygate_domain::resource_selection::snapshot_budget;
}

#[path = "../../src/qualitygate/rule_catalog"]
pub mod rule_catalog {
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
    #[path = "builtin_conventions.rs"]
    pub mod builtin_conventions;
    #[path = "builtin_rules.rs"]
    pub mod builtin_rules;
    #[path = "custom_rules/mod.rs"]
    pub mod custom_rules;
    #[path = "evaluation_facts.rs"]
    pub mod evaluation_facts;
    #[path = "language_syntax.rs"]
    pub mod language_syntax;
    #[path = "markers.rs"]
    pub mod markers;
    #[path = "native_syntax.rs"]
    pub mod native_syntax;
    #[path = "operators_v2.rs"]
    pub mod operators_v2;
    #[path = "operators_v3.rs"]
    pub mod operators_v3;
    #[path = "project_checks.rs"]
    pub mod project_checks;
    #[path = "shell_conventions.rs"]
    pub mod shell_conventions;
    #[path = "structure_rules.rs"]
    pub mod structure_rules;
}

#[path = "../../src/qualitygate/selfcheck"]
pub mod selfcheck {
    pub use qualitygate_config::selfcheck::fixture_catalog;
    pub use qualitygate_config::selfcheck::policy_fixtures;
    pub use qualitygate_domain::selfcheck::fixture_results;
}

#[path = "../../src/qualitygate/snapshot_preflight"]
pub mod snapshot_preflight {
    pub use qualitygate_config::snapshot_preflight::preflight_schema;
    pub use qualitygate_domain::snapshot_preflight::readiness_contracts;
}

#[path = "../../src/qualitygate/source_analysis"]
pub mod source_analysis {
    #[path = "analysis_workers.rs"]
    pub mod analysis_workers;
    #[path = "entity_changes.rs"]
    pub mod entity_changes;
    #[path = "language_bindings.rs"]
    pub mod language_bindings;
    #[path = "language_shell.rs"]
    pub mod language_shell;
    #[path = "syntax/mod.rs"]
    pub mod syntax;
}

#[path = "../../src/qualitygate/test_effectiveness"]
pub mod test_effectiveness {
    pub use qualitygate_config::test_effectiveness::execution_spec;
    pub use qualitygate_domain::test_effectiveness::counterexample_decisions;
}

#[path = "../../src/qualitygate/xml_parsing"]
pub mod xml_parsing {
    #[path = "document.rs"]
    pub mod document;
    pub use qualitygate_domain::xml_parsing::xml_budget;
}
