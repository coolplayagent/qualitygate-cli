//! Strict configuration contracts, plan validation and repository discovery.

pub mod attestation;
mod builtin_validation;
pub mod catalog;
pub mod compatibility;
mod constraints;
mod custom_validation;
pub mod discovery;
mod initialization;
mod model;
mod plan;
pub mod project_rules;
mod python;
pub mod source_reviews;
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
    use std::io::Write;
    let config = read(root, path)?;
    let catalog = catalog::read(root, &config)?;
    let mut config = catalog.resolve(&config)?;
    let entry = catalog
        .entries
        .get(rule_id)
        .with_context(|| format!("Unknown rule: {rule_id}"))?;
    config
        .rules
        .entry(rule_id.into())
        .or_insert_with(|| entry.defaults())
        .enabled = true;
    for profile in config.profiles.values_mut() {
        if !profile.include.iter().any(|id| id == rule_id) {
            profile.include.push(rule_id.into());
        }
    }
    let data = serde_norway::to_string(&config)?;
    parse(data.as_bytes())?;
    let target = crate::paths::confined(root, path)?;
    let mut temporary = tempfile::NamedTempFile::new_in(
        target
            .parent()
            .context("Configuration has no parent directory")?,
    )?;
    temporary
        .as_file()
        .set_permissions(std::fs::metadata(&target)?.permissions())?;
    temporary.write_all(data.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(target)?;
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
