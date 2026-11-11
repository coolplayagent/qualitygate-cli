//! Check planning entry points.

use crate::config::*;

use anyhow::{Context, Result, bail};
use std::path::Path;

pub const CONFIG_FILE: &str = "qualitygate.yaml";
pub const MAX_CONFIG_BYTES: usize = 1_048_576;

/// Validate mapping uniqueness before deserializing typed maps, whose visitors
/// would otherwise accept a later value for the same YAML key.
pub(crate) fn parse_yaml<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let _: serde_norway::Value = yaml_budget::value(bytes)?;
    Ok(serde_norway::from_slice(bytes)?)
}

pub fn parse(bytes: &[u8]) -> Result<Config> {
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Configuration exceeds {MAX_CONFIG_BYTES} bytes");
    }
    runtime::requirements(bytes)?;
    let config: Config = runtime::decode(bytes).context("Invalid qualitygate YAML")?;
    validation::layout(&config, false)?;
    if config.custom_rules.is_none() {
        catalog::Catalog::load(&config, std::iter::empty())?.resolve(&config)?;
    }
    Ok(config)
}

pub fn read(root: &Path, file: &Path) -> Result<Config> {
    use std::io::Read;

    let file = crate::paths::confined(root, file)?;
    let metadata = std::fs::metadata(&file).map_err(|error| read_error(&file, error))?;
    validate_file_metadata(&file, &metadata)?;
    let input = std::fs::File::open(&file).map_err(|error| read_error(&file, error))?;
    validate_file_metadata(
        &file,
        &input.metadata().map_err(|error| read_error(&file, error))?,
    )?;
    let mut bytes = Vec::new();
    input
        .take(MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| read_error(&file, error))?;
    parse(&bytes)
}

pub(crate) fn validate_file_metadata(file: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    if !metadata.is_file() {
        bail!(
            "Cannot read {}: configuration must be a regular file",
            crate::paths::display(file)
        );
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        bail!("Configuration exceeds {MAX_CONFIG_BYTES} bytes");
    }
    Ok(())
}

pub(crate) fn read_error(file: &Path, error: std::io::Error) -> anyhow::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        crate::domain::PolicyInputError::LocalConfigurationMissing {
            path: crate::paths::display(file).to_string(),
        }
        .into()
    } else {
        anyhow::Error::new(error).context(format!("Cannot read {}", crate::paths::display(file)))
    }
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
