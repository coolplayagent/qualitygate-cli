//! Diagnoses snapshot acquisition, tool prerequisites and execution readiness.
//!

pub(crate) mod acquisition;
pub mod capabilities;
pub(crate) mod doctor_context;
pub mod doctor_output;
pub(crate) mod doctor_plan;
pub(crate) mod doctor_summary;
pub mod preflight_schema;
pub(crate) mod readiness_args;
pub mod readiness_check;
pub mod readiness_contracts;
pub mod repository_root;
