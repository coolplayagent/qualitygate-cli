//! Strict configuration contracts, plan validation and repository discovery.

mod builtin_validation;
pub mod catalog;
mod constraints;
mod custom_validation;
mod model;
mod plan;
mod validation;
pub use model::*;
pub use plan::{Plan, parse_task};

use anyhow::{Context, Result, bail};
use std::path::Path;

pub const CONFIG_FILE: &str = "qualitygate.yaml";
pub const MAX_CONFIG_BYTES: usize = 1_048_576;

pub fn parse(bytes: &[u8]) -> Result<Config> {
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Configuration exceeds {MAX_CONFIG_BYTES} bytes");
    }
    let config: Config = serde_norway::from_slice(bytes).context("Invalid qualitygate YAML")?;
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
    use std::io::Write;
    let path = crate::paths::confined(root, configuration)?;
    if path.exists() {
        return read(root, configuration);
    }
    let mut config = Config::default();
    for (manifest, language) in [
        ("Cargo.toml", "rust"),
        ("pom.xml", "java"),
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("package.json", "typescript"),
        ("go.mod", "go"),
    ] {
        if root.join(manifest).is_file() && !config.languages.iter().any(|value| value == language)
        {
            config.languages.push(language.into());
        }
    }
    config
        .rules
        .insert("line-ending".into(), RuleSetting::default());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(serde_norway::to_string(&config)?.as_bytes())?;
    file.sync_all()?;
    Ok(config)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
