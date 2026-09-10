use super::{Config, RuleSetting, discovery};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{io::Write, path::Path};

#[derive(Debug, Serialize)]
pub struct Initialization {
    pub config: Config,
    pub discovery: discovery::Discovery,
    pub created: bool,
    pub with_checks_applied: bool,
}

/// Discovery is advisory; adoption creates a new candidate and never rewrites an existing policy.
pub fn initialize_at(
    root: &Path,
    configuration: &Path,
    with_checks: bool,
) -> Result<Initialization> {
    let root = dunce::canonicalize(root).context("Repository root does not exist")?;
    let path = crate::paths::confined(&root, configuration)?;
    let existing = if path.exists() {
        Some(super::read(&root, configuration)?)
    } else {
        None
    };
    let discovery = discovery::discover(&root)?;
    if let Some(config) = existing {
        return Ok(Initialization {
            config,
            discovery,
            created: false,
            with_checks_applied: false,
        });
    }
    let mut config = Config {
        languages: discovery
            .languages
            .iter()
            .map(|language| language.language.clone())
            .collect(),
        ..Config::default()
    };
    config
        .rules
        .insert("line-ending".into(), RuleSetting::default());
    if with_checks {
        config.checks = discovery
            .suggested_checks
            .iter()
            .filter(|suggestion| suggestion.status == "ready_for_review")
            .map(|suggestion| suggestion.check.clone())
            .collect();
        config.verification_assets = discovery.inputs.keys().cloned().collect();
        config.verification_assets.extend([
            crate::paths::from_native(configuration)?,
            "tests/**".into(),
            "**/tests/**".into(),
            ".github/workflows/**".into(),
            "**/mvnw".into(),
            "**/mvnw.cmd".into(),
            "**/.mvn/**".into(),
        ]);
        config.verification_assets.sort();
        config.verification_assets.dedup();
    }
    let bytes = serde_norway::to_string(&config)?;
    super::parse(bytes.as_bytes())?;
    let parent = path.parent().context("Configuration has no parent")?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes.as_bytes())?;
    temporary.as_file().sync_all()?;
    // Atomic no-clobber publication also protects a concurrently created policy.
    temporary
        .persist_noclobber(&path)
        .context("Cannot publish candidate without replacing an existing file")?;
    let with_checks_applied = with_checks && !config.checks.is_empty();
    Ok(Initialization {
        config,
        discovery,
        created: true,
        with_checks_applied,
    })
}
