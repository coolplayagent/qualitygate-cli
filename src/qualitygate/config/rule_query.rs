//! Read-only inventory, separate from policy selection and project overrides.

use super::{
    Config, RULESETS,
    catalog::{self, Catalog, Entry, PROJECT_RULES_DIR},
};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::path::Path;

fn languages(entry: &Entry) -> &[String] {
    entry
        .builtin
        .as_ref()
        .map(|rule| rule.language.as_slice())
        .or_else(|| entry.custom.as_ref().map(|rule| rule.language.as_slice()))
        .unwrap_or_default()
}

pub fn list(root: &Path, path: &str, language: Option<&str>, source: &str) -> Result<Value> {
    if let Some(language) = language
        && (language.is_empty()
            || language.len() > 64
            || !language.starts_with(|c: char| c.is_ascii_lowercase())
            || !language
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_+-".contains(&c))
            || language == "all")
    {
        bail!("Language must be a lowercase language identifier (for example java or rust)");
    }
    if !["builtin", "project", "all"].contains(&source) {
        bail!("Unknown rule source: {source}");
    }
    let policy_path = crate::paths::confined(root, path.as_ref())?;
    let policy = match std::fs::metadata(policy_path) {
        Ok(_) => Some(super::read(root, path.as_ref())?),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound && path == super::CONFIG_FILE =>
        {
            None
        }
        Err(error) => return Err(error.into()),
    };
    let active = policy
        .as_ref()
        .map(|policy| catalog::read(root, policy))
        .transpose()?;
    let effective = policy
        .as_ref()
        .zip(active.as_ref())
        .map(|(policy, active)| active.resolve(policy))
        .transpose()?;
    let reviews = effective
        .as_ref()
        .zip(active.as_ref())
        .map(|(policy, active)| super::source_reviews::evidence(policy, active))
        .transpose()?
        .unwrap_or_default();
    let mut inventory_config = Config {
        rulesets: RULESETS.iter().map(|name| (*name).into()).collect(),
        ..Config::default()
    };
    let builtins = Catalog::load(&inventory_config, std::iter::empty())?;
    let directory = policy
        .as_ref()
        .and_then(|config| config.custom_rules.as_deref())
        .unwrap_or(PROJECT_RULES_DIR);
    let mut projects = None;
    if source != "builtin" {
        let project_path = crate::paths::confined(root, directory.as_ref())?;
        match std::fs::metadata(project_path) {
            Ok(_) => {
                inventory_config.custom_rules = Some(directory.into());
                projects = Some(catalog::read(root, &inventory_config)?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut rows = Vec::new();
    let entries = builtins
        .entries
        .iter()
        .filter(|_| source != "project")
        .chain(
            projects
                .iter()
                .flat_map(|catalog| catalog.entries.iter())
                .filter(|(_, entry)| entry.custom.is_some()),
        );
    for (id, entry) in entries {
        let scope = languages(entry);
        if language
            .is_some_and(|language| !scope.is_empty() && !scope.iter().any(|name| name == language))
        {
            continue;
        }
        let selected = active
            .as_ref()
            .and_then(|catalog| catalog.entries.get(id))
            .is_some_and(|current| current.origin == entry.origin);
        let configuration = effective
            .as_ref()
            .and_then(|config| config.rules.get(id))
            .filter(|_| selected);
        rows.push(json!({"id":id,"definition":entry,"language":scope,
            "source":if entry.custom.is_some() { "project" } else { "builtin" },
            "configuration":configuration,"enabled":configuration.is_some_and(|setting| setting.enabled),
            "overrides_builtin":entry.custom.is_some() && builtins.entries.contains_key(id)}));
    }
    Ok(
        json!({"schema_version":1,"rules":rows,"language":language,"source":source,
        "project_rules_directory":directory,"source_reviews":reviews,"review_trust":"local_candidate"}),
    )
}
