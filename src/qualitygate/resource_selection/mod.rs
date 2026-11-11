//! Selects changed resources and enforces exclusion and snapshot acquisition budgets.
//!

pub mod exclusions;
pub mod resource_exclusions;
pub mod selection_contracts;
pub mod snapshot_budget;
pub(crate) mod snapshot_selection;
pub mod trigger_selection;
