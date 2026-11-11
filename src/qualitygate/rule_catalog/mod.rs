//! Discovers, authors, parameterizes and queries rule definitions and operators.
//!

pub(crate) mod builtin_compatibility;
pub(crate) mod builtin_validation;
pub mod catalog;
pub(crate) mod catalog_assets;
pub mod categories;
pub(crate) mod category;
pub(crate) mod custom_validation;
pub mod language;
pub mod normative;
pub mod operator;
pub mod operator_v3;
pub mod parameters;
pub mod project_rules;
pub mod rule_authoring;
pub mod rule_context;
pub mod rule_guidance;
pub mod rule_lifecycle;
pub mod rule_management;
pub mod rule_parameters;
pub mod rule_query;
pub mod rule_schema;
pub(crate) mod rule_validation;
