//! Authenticates external attestations, manual acceptance and provenance evidence.
//!

pub mod case_lineage;
pub(crate) mod evidence;
pub(crate) mod external;
pub(crate) mod history_acquisition;
pub mod lineage_validation;
pub(crate) mod manual;
pub(crate) mod manual_contracts;
pub(crate) mod provenance_contracts;
pub(crate) mod provenance_preparation;
pub mod provenance_verification;
pub mod signature_verification;
pub mod trailer_analysis;
pub mod trust_store;
