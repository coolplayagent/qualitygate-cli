//! Bazel-only entry point for the config capability boundary.

pub use qualitygate_domain::domain;
pub use qualitygate_env::env;
pub use qualitygate_paths::paths;

#[path = "../../src/qualitygate/config.rs"]
pub mod config;

#[path = "../../src/qualitygate/agent_runtime"]
pub mod agent_runtime {
    #[path = "obligation_validation.rs"]
    pub mod obligation_validation;
    pub use qualitygate_domain::agent_runtime::host_decisions;
    pub use qualitygate_domain::agent_runtime::obligation_contracts;
}

#[path = "../../src/qualitygate/check_planning"]
pub mod check_planning {
    #[path = "dependencies.rs"]
    pub mod dependencies;
    #[path = "discovery/mod.rs"]
    pub mod discovery;
    #[path = "initialization.rs"]
    pub mod initialization;
    #[path = "loading.rs"]
    pub mod loading;
    #[path = "model.rs"]
    pub mod model;
    #[path = "parallel.rs"]
    pub mod parallel;
    #[path = "plan.rs"]
    pub mod plan;
    #[path = "project_inventory.rs"]
    pub mod project_inventory;
    #[path = "python.rs"]
    pub mod python;
    #[path = "runtime_validation.rs"]
    pub mod runtime_validation;
    #[path = "validation.rs"]
    pub mod validation;
    #[path = "yaml_budget.rs"]
    pub mod yaml_budget;
    #[path = "yaml_scalar_work.rs"]
    pub mod yaml_scalar_work;
    pub use qualitygate_domain::check_planning::project;
    pub use qualitygate_domain::check_planning::runtime_contracts;
}

#[path = "../../src/qualitygate/compatibility_verification"]
pub mod compatibility_verification {
    #[path = "compatibility_spec.rs"]
    pub mod compatibility_spec;
}

#[path = "../../src/qualitygate/constraint_coverage"]
pub mod constraint_coverage {
    #[path = "constraint_inventory.rs"]
    pub mod constraint_inventory;
    #[path = "constraints.rs"]
    pub mod constraints;
    pub use qualitygate_domain::constraint_coverage::constraint;
}

pub use qualitygate_domain::content_digest;

#[path = "../../src/qualitygate/external_acceptance"]
pub mod external_acceptance {
    #[path = "lineage_validation.rs"]
    pub mod lineage_validation;
    #[path = "trust_store.rs"]
    pub mod trust_store;
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
    #[path = "policy_loading.rs"]
    pub mod policy_loading;
    pub use qualitygate_domain::judgment_review::assessment_contracts;
}

#[path = "../../src/qualitygate/pilot_acceptance"]
pub mod pilot_acceptance {
    #[path = "inventory_loading.rs"]
    pub mod inventory_loading;
    pub use qualitygate_domain::pilot_acceptance::pilot_contracts;
}

#[path = "../../src/qualitygate/policy_lifecycle"]
pub mod policy_lifecycle {
    #[path = "active_package.rs"]
    pub mod active_package;
    #[path = "candidate_store.rs"]
    pub mod candidate_store;
    #[path = "effectiveness_history.rs"]
    pub mod effectiveness_history;
    #[path = "policy_acceptance.rs"]
    pub mod policy_acceptance;
    #[path = "policy_artifacts.rs"]
    pub mod policy_artifacts;
    #[path = "policy_store.rs"]
    pub mod policy_store;
    #[path = "promotion_store.rs"]
    pub mod promotion_store;
    #[path = "rollback_store.rs"]
    pub mod rollback_store;
    #[path = "source_reviews.rs"]
    pub mod source_reviews;
    #[path = "validation_store.rs"]
    pub mod validation_store;
    pub use qualitygate_domain::policy_lifecycle::effectiveness_measures;
    pub use qualitygate_domain::policy_lifecycle::evolution;
    pub use qualitygate_domain::policy_lifecycle::policy_approval;
    pub use qualitygate_domain::policy_lifecycle::policy_evaluation;
    pub use qualitygate_domain::policy_lifecycle::policy_input;
    pub use qualitygate_domain::policy_lifecycle::rollback_contracts;
    pub use qualitygate_domain::policy_lifecycle::source_review;
}

#[path = "../../src/qualitygate/repair_feedback"]
pub mod repair_feedback {
    pub use qualitygate_domain::repair_feedback::delivery_contracts;
    pub use qualitygate_domain::repair_feedback::feedback_projection;
}

#[path = "../../src/qualitygate/report_evidence"]
pub mod report_evidence {
    #[path = "decision_schema.rs"]
    pub mod decision_schema;
    pub use qualitygate_domain::report_evidence::coverage;
    pub use qualitygate_domain::report_evidence::decision_envelope;
    pub use qualitygate_domain::report_evidence::ratchet;
    pub use qualitygate_domain::report_evidence::report;
    pub use qualitygate_domain::report_evidence::verification;
}

#[path = "../../src/qualitygate/resource_selection"]
pub mod resource_selection {
    #[path = "exclusions.rs"]
    pub mod exclusions;
    #[path = "trigger_selection.rs"]
    pub mod trigger_selection;
    pub use qualitygate_domain::resource_selection::resource_exclusions;
    pub use qualitygate_domain::resource_selection::selection_contracts;
    pub use qualitygate_domain::resource_selection::snapshot_budget;
}

#[path = "../../src/qualitygate/rule_catalog"]
pub mod rule_catalog {
    #[path = "builtin_compatibility.rs"]
    pub mod builtin_compatibility;
    #[path = "builtin_validation.rs"]
    pub mod builtin_validation;
    #[path = "catalog.rs"]
    pub mod catalog;
    #[path = "catalog_assets.rs"]
    pub mod catalog_assets;
    #[path = "categories.rs"]
    pub mod categories;
    #[path = "custom_validation.rs"]
    pub mod custom_validation;
    #[path = "parameters.rs"]
    pub mod parameters;
    #[path = "project_rules.rs"]
    pub mod project_rules;
    #[path = "rule_authoring.rs"]
    pub mod rule_authoring;
    #[path = "rule_management.rs"]
    pub mod rule_management;
    #[path = "rule_query.rs"]
    pub mod rule_query;
    #[path = "rule_schema.rs"]
    pub mod rule_schema;
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
    #[path = "fixture_catalog.rs"]
    pub mod fixture_catalog;
    #[path = "policy_fixtures.rs"]
    pub mod policy_fixtures;
    pub use qualitygate_domain::selfcheck::fixture_results;
}

#[path = "../../src/qualitygate/snapshot_preflight"]
pub mod snapshot_preflight {
    #[path = "preflight_schema.rs"]
    pub mod preflight_schema;
    pub use qualitygate_domain::snapshot_preflight::readiness_contracts;
}

#[path = "../../src/qualitygate/test_effectiveness"]
pub mod test_effectiveness {
    #[path = "execution_spec.rs"]
    pub mod execution_spec;
    pub use qualitygate_domain::test_effectiveness::counterexample_decisions;
}

#[path = "../../src/qualitygate/xml_parsing"]
pub mod xml_parsing {
    pub use qualitygate_domain::xml_parsing::xml_budget;
}
