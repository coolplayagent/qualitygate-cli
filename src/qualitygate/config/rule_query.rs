//! Read-only inventory, separate from policy selection and project overrides.

use super::{
    Config, RULESETS,
    catalog::{Catalog, Entry, PROJECT_RULES_DIR},
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
    list_filtered(root, path, language, source, None)
}

pub fn list_filtered(
    root: &Path,
    path: &str,
    language: Option<&str>,
    source: &str,
    category: Option<&str>,
) -> Result<Value> {
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
    let inventory = Inventory::load(root, path, source != "builtin")?;
    let policy = inventory.policy.clone().unwrap_or_default();
    let categories = super::categories::registry(&policy);
    if let Some(name) = category
        && !categories.contains_key(name)
    {
        bail!("Unknown category: {name}");
    }
    let mut rows = Vec::new();
    let entries = inventory
        .builtins
        .entries
        .iter()
        .filter(|_| source != "project")
        .chain(inventory.projects.iter().filter(|_| source != "builtin"));
    for (id, entry) in entries {
        let assignment = super::categories::assigned(&policy, &categories, id, entry);
        if category.is_some_and(|name| assignment != Some(name)) {
            continue;
        }
        let scope = languages(entry);
        if language
            .is_some_and(|language| !scope.is_empty() && !scope.iter().any(|name| name == language))
        {
            continue;
        }
        rows.push(inventory.row(id, entry, &categories));
    }
    Ok(
        json!({"schema_version":1,"rules":rows,"language":language,"source":source,"category":category,
        "project_rules_directory":inventory.directory,"source_reviews":inventory.reviews,"review_trust":"local_candidate"}),
    )
}

pub fn categories(root: &Path, path: &str) -> Result<Value> {
    let inventory = Inventory::load(root, path, true)?;
    let policy = inventory.policy.clone().unwrap_or_default();
    let categories = super::categories::registry(&policy);
    let mut counts = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    // Prefer the active definition for duplicate IDs, otherwise the discovered project rule.
    let mut entries = inventory.builtins.entries.clone();
    entries.extend(inventory.projects.clone());
    entries.extend(inventory.active.entries.clone());
    for (id, entry) in &entries {
        if let Some(name) = super::categories::assigned(&policy, &categories, id, entry) {
            let count = counts.entry(name).or_default();
            count.0 += 1;
            count.1 += usize::from(inventory.enabled(id, entry));
        }
    }
    let rows: Vec<_> = categories.iter().map(|(name, category)| {
        let (rules, enabled) = counts.get(name.as_str()).copied().unwrap_or_default();
        json!({"name":name,"description":category.description,"custom":category.origin.is_none(),"rule_count":rules,"enabled_count":enabled})
    }).collect();
    Ok(json!({"schema_version":1,"categories":rows,"review_trust":"local_candidate"}))
}

pub fn describe(root: &Path, path: &str, id: &str) -> Result<Value> {
    let inventory = Inventory::load(root, path, true)?;
    let entry = inventory
        .active
        .entries
        .get(id)
        .or_else(|| inventory.projects.get(id))
        .or_else(|| inventory.builtins.entries.get(id))
        .ok_or_else(|| anyhow::anyhow!("Unknown rule: {id}"))?;
    let categories = super::categories::registry(&inventory.policy.clone().unwrap_or_default());
    let row = inventory.row(id, entry, &categories);
    let builtin = &row["definition"]["builtin"];
    let custom = &row["definition"]["custom"];
    let definition = if builtin.is_null() { custom } else { builtin };
    let parameters = if let Some(implementation) = builtin["implementation"].as_str() {
        super::parameters::describe(implementation, &builtin["defaults"]["parameters"])?
    } else {
        Vec::new()
    };
    Ok(
        json!({"schema_version":1,"id":id,"version":definition["version"],
        "description": if builtin.is_null() { &custom["fix"] } else { &builtin["description"] },
        "category":row["category"],"implementation":builtin["implementation"].as_str().unwrap_or("project-dsl"),
        "language":row["language"],"requires_capabilities":definition["requires_capabilities"],
        "defaults":if builtin.is_null() { json!({"enabled":true,"severity":custom["severity"],"required":custom["required"]}) } else { builtin["defaults"].clone() },
        "enabled":row["enabled"],"configuration":row["configuration"],"parameters":parameters,
        "standard_refs":builtin["standard_refs"].as_array().cloned().unwrap_or_default(),
        "definition":row["definition"],"source":row["source"],"review_trust":"local_candidate"}),
    )
}

pub(super) struct Inventory {
    pub policy: Option<Config>,
    pub builtins: Catalog,
    pub projects: std::collections::BTreeMap<String, Entry>,
    active: Catalog,
    effective: Option<Config>,
    directory: String,
    reviews: serde_json::Value,
}

impl Inventory {
    fn load(root: &Path, path: &str, discover_projects: bool) -> Result<Self> {
        let policy_path = crate::paths::confined(root, path.as_ref())?;
        let policy = match std::fs::metadata(policy_path) {
            Ok(_) => {
                let bytes = super::rule_authoring::read_file(root, path)?;
                let config: Config = super::parse_yaml(&bytes)?;
                super::validation::layout(&config, false)?;
                Some(config)
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && path == super::CONFIG_FILE =>
            {
                None
            }
            Err(error) => return Err(error.into()),
        };
        Self::for_policy(root, policy, discover_projects)
    }

    pub fn for_policy(
        root: &Path,
        policy: Option<Config>,
        discover_projects: bool,
    ) -> Result<Self> {
        let mut inventory_config = Config {
            rulesets: RULESETS.iter().map(|name| (*name).into()).collect(),
            ..Config::default()
        };
        let builtins = Catalog::load(&inventory_config, std::iter::empty())?;
        let selected_project = policy
            .as_ref()
            .and_then(|config| config.custom_rules.as_deref());
        let directory = selected_project.unwrap_or(PROJECT_RULES_DIR).to_string();
        let mut projects = std::collections::BTreeMap::new();
        if discover_projects || selected_project.is_some() {
            let path = crate::paths::confined(root, directory.as_ref())?;
            match std::fs::metadata(path) {
                Ok(_) => {
                    inventory_config.custom_rules = Some(directory.clone());
                    projects = super::project_inventory::discover(root, &inventory_config)?;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && selected_project.is_none() => {}
                Err(error) => return Err(error.into()),
            }
        }
        let mut inventory = Self {
            policy,
            builtins,
            projects,
            active: Catalog {
                entries: Default::default(),
            },
            effective: None,
            directory,
            reviews: json!({}),
        };
        if let Some(config) = &inventory.policy {
            inventory.active = inventory.selected(config);
            inventory.effective = Some(inventory.active.resolve(config)?);
            inventory.reviews = serde_json::to_value(super::source_reviews::evidence(
                inventory.effective.as_ref().expect("effective policy"),
                &inventory.active,
            )?)?;
        }
        Ok(inventory)
    }

    pub fn selected(&self, config: &Config) -> Catalog {
        let mut entries: std::collections::BTreeMap<_, _> = self
            .builtins
            .entries
            .iter()
            .filter(|(_, entry)| {
                ["core", "shared"].contains(&entry.package.as_str())
                    || config.rulesets.contains(&entry.package)
            })
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect();
        if config.custom_rules.is_some() {
            entries.extend(self.projects.clone());
        }
        Catalog { entries }
    }

    fn configuration(&self, id: &str, entry: &Entry) -> Option<&super::RuleSetting> {
        self.effective
            .as_ref()
            .and_then(|config| config.rules.get(id))
            .filter(|_| {
                self.active
                    .entries
                    .get(id)
                    .is_some_and(|current| current.origin == entry.origin)
            })
    }

    fn enabled(&self, id: &str, entry: &Entry) -> bool {
        self.configuration(id, entry)
            .is_some_and(|rule| rule.enabled)
    }

    fn row(
        &self,
        id: &str,
        entry: &Entry,
        categories: &std::collections::BTreeMap<String, super::categories::Category>,
    ) -> Value {
        let default_policy = Config::default();
        let policy = self.policy.as_ref().unwrap_or(&default_policy);
        json!({"id":id,"definition":entry,"language":languages(entry),
            "category":super::categories::assigned(policy, categories, id, entry),
            "source":if entry.custom.is_some() { "project" } else { "builtin" },
            "configuration":self.configuration(id, entry),"enabled":self.enabled(id, entry),
            "overrides_builtin":entry.custom.is_some() && self.builtins.entries.contains_key(id)})
    }
}
