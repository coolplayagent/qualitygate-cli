//! Loads bounded configuration and builds validated dependency-ordered check plans.
//!

pub(crate) mod check_request;
pub(crate) mod command_applicability;
pub(crate) mod dependencies;
pub mod discovery;
pub(crate) mod initialization;
pub(crate) mod loading;
pub(crate) mod model;
pub(crate) mod parallel;
pub(crate) mod plan;
pub(crate) mod project;
pub(crate) mod project_inventory;
pub(crate) mod python;
pub mod runtime_contracts;
pub mod runtime_validation;
pub mod selection_contracts;
pub mod snapshot_inputs;
pub(crate) mod tool_facts;
pub mod trigger_selection;
pub(crate) mod validation;
pub(crate) mod yaml_budget;
pub(crate) mod yaml_scalar_work;
