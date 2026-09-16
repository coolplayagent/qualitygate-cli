//! Strict configuration contracts, plan validation and repository discovery.

pub mod attestation;
mod builtin_validation;
pub mod case_provenance;
pub mod catalog;
mod catalog_assets;
pub mod categories;
pub mod compatibility;
mod constraints;
mod custom_validation;
pub mod discovery;
mod initialization;
mod model;
mod parallel;
pub mod parameters;
pub mod pilot;
mod plan;
pub mod policy_acceptance;
pub mod policy_active;
mod policy_artifacts;
pub mod policy_candidates;
pub mod policy_effectiveness;
pub mod policy_promotion;
pub mod policy_rollback;
pub mod policy_store;
pub mod policy_validation;
mod project_inventory;
pub mod project_rules;
mod python;
pub mod rule_authoring;
pub mod rule_management;
pub mod rule_query;
pub mod rule_schema;
pub mod selfcheck;
pub mod selfcheck_policy;
pub mod source_reviews;
pub mod test_effectiveness;
mod validation;
pub use initialization::{Initialization, initialize_at};
pub use model::*;
pub use plan::{Plan, parse_task};

use anyhow::{Context, Result, bail};
use std::path::Path;

pub const CONFIG_FILE: &str = "qualitygate.yaml";
pub const MAX_CONFIG_BYTES: usize = 1_048_576;

/// Validate mapping uniqueness before deserializing typed maps, whose visitors
/// would otherwise accept a later value for the same YAML key.
pub(super) fn parse_yaml<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let _: serde_norway::Value = serde_norway::from_slice(bytes)?;
    Ok(serde_norway::from_slice(bytes)?)
}

pub fn parse(bytes: &[u8]) -> Result<Config> {
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Configuration exceeds {MAX_CONFIG_BYTES} bytes");
    }
    let config: Config = parse_yaml(bytes).context("Invalid qualitygate YAML")?;
    validation::layout(&config, false)?;
    if config.custom_rules.is_none() {
        catalog::Catalog::load(&config, std::iter::empty())?.resolve(&config)?;
    }
    Ok(config)
}

pub fn read(root: &Path, file: &Path) -> Result<Config> {
    let file = crate::paths::confined(root, file)?;
    let metadata = std::fs::metadata(&file)
        .with_context(|| format!("Cannot read {}; run qualitygate init first", file.display()))?;
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        bail!("Configuration exceeds {MAX_CONFIG_BYTES} bytes");
    }
    parse(&std::fs::read(file)?)
}

/// Initializes a candidate policy without overwriting any existing configuration.
pub fn init(root: &Path) -> Result<Config> {
    init_at(root, Path::new(CONFIG_FILE))
}

pub fn init_at(root: &Path, configuration: &Path) -> Result<Config> {
    Ok(initialize_at(root, configuration, false)?.config)
}

/// Enables a candidate rule without changing the policy trust decision.
pub fn enable_rule(root: &Path, path: &Path, rule_id: &str) -> Result<()> {
    rule_management::update(
        root,
        path,
        rule_management::Mutation::Enable(rule_id.into()),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
