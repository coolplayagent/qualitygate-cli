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
    filtered(&inventory, language, source, category)
}

fn filtered(
    inventory: &Inventory,
    language: Option<&str>,
    source: &str,
    category: Option<&str>,
) -> Result<Value> {
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
        let assignments = super::categories::assignments(&policy, &categories, id, entry);
        if category.is_some_and(|name| !assignments.contains(&name)) {
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
        json!({"schema_version":1,"rules":rows,"mandatory":inventory.mandatory(&categories),
        "mandatory_checks":inventory.mandatory_checks(),"policy_digest":inventory.policy_digest,
        "language":language,"source":source,"category":category,
        "project_rules_directory":inventory.directory,"source_reviews":inventory.reviews,"review_trust":"local_candidate"}),
    )
}

/// Context always carries the entire mandatory baseline, independent of filters.
pub fn context(root: &Path, path: &str, category: Option<&str>) -> Result<Value> {
    context_rows(list_filtered(root, path, None, "all", category)?)
}

/// Read frozen Git bytes directly, without materializing an unrelated source tree.
pub fn context_from_files<'a>(
    path: &str,
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    category: Option<&str>,
) -> Result<Value> {
    use sha2::{Digest, Sha256};
    let files: std::collections::BTreeMap<_, _> = files.into_iter().collect();
    let bytes = files
        .get(path)
        .ok_or_else(|| crate::domain::prerequisites::PrerequisiteIssue::new(
            crate::domain::prerequisites::FailureCode::PolicySnapshotMissing,
            crate::domain::prerequisites::Phase::Policy, "Selected policy snapshot has no configuration")
            .resource(path).instruction("Select a policy reference containing the configuration; initializing the worktree does not alter historical commits."))?;
    let config = super::parse(bytes)?;
    let all = Config {
        rulesets: RULESETS.iter().map(|name| (*name).into()).collect(),
        ..Config::default()
    };
    let builtins = Catalog::load(&all, std::iter::empty())?;
    let directory = config
        .custom_rules
        .clone()
        .unwrap_or_else(|| PROJECT_RULES_DIR.into());
    let project_config = Config {
        custom_rules: Some(directory.clone()),
        ..Config::default()
    };
    let has_projects = files.keys().any(|name| {
        name.starts_with(&format!("{directory}/"))
            && (name.ends_with(".yaml") || name.ends_with(".yml"))
    });
    let projects = if has_projects || config.custom_rules.is_some() {
        super::project_inventory::parse(
            &project_config,
            files.iter().map(|(name, bytes)| (*name, *bytes)),
            super::parallel::jobs(),
        )?
    } else {
        Default::default()
    };
    let mut inventory = Inventory::from_catalogs(Some(config), builtins, projects, directory)?;
    inventory.policy_digest = Some(format!("sha256:{:x}", Sha256::digest(bytes)));
    context_rows(filtered(&inventory, None, "all", category)?)
}

fn context_rows(mut result: Value) -> Result<Value> {
    result["selected"] = result["rules"].take();
    result
        .as_object_mut()
        .expect("inventory object")
        .remove("rules");
    Ok(result)
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
        for name in super::categories::assignments(&policy, &categories, id, entry) {
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
        "category":row["category"],"categories":row["categories"],"implementation":builtin["implementation"].as_str().unwrap_or("project-dsl"),
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
    policy_digest: Option<String>,
}

impl Inventory {
    fn load(root: &Path, path: &str, discover_projects: bool) -> Result<Self> {
        if let Some(mut active) = super::policy_active::load(root)? {
            super::policy_active::navigation(root, path, &mut active.config)?;
            let directory = active
                .config
                .custom_rules
                .clone()
                .unwrap_or_else(|| PROJECT_RULES_DIR.into());
            let mut inventory = Self::from_catalogs(
                Some(active.config),
                active.frozen.builtins,
                active.frozen.projects,
                directory,
            )?;
            inventory.policy_digest = Some(active.reference);
            return Ok(inventory);
        }
        let policy_path = crate::paths::confined(root, path.as_ref())?;
        let mut policy_digest = None;
        let policy = match std::fs::metadata(policy_path) {
            Ok(_) => {
                let bytes = super::read_candidate_bytes(root, path.as_ref())?;
                use sha2::{Digest, Sha256};
                policy_digest = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));
                let config: Config = super::parse_yaml(&bytes)
                    .and_then(|config| {
                        super::validation::layout(&config, false)?;
                        Ok(config)
                    })
                    .map_err(|error| {
                        crate::domain::prerequisites::PrerequisiteIssue::new(
                            crate::domain::prerequisites::FailureCode::ConfigInvalid,
                            crate::domain::prerequisites::Phase::Policy,
                            "Qualitygate configuration is invalid",
                        )
                        .resource(path)
                        .wrap(error)
                    })?;
                Some(config)
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && path == super::CONFIG_FILE =>
            {
                None
            }
            Err(error) => {
                let missing = error.kind() == std::io::ErrorKind::NotFound;
                return Err(crate::domain::prerequisites::PrerequisiteIssue::new(
                    if missing {
                        crate::domain::prerequisites::FailureCode::RepositoryNotInitialized
                    } else {
                        crate::domain::prerequisites::FailureCode::InputUnreadable
                    },
                    crate::domain::prerequisites::Phase::Policy,
                    format!("Cannot read configuration: {path}"),
                )
                .resource(path)
                .instruction(if missing {
                    "Run qualitygate init using the selected --config."
                } else {
                    "Check configuration access permissions."
                })
                .wrap(error.into()));
            }
        };
        let mut inventory = Self::for_policy(root, policy, discover_projects)?;
        inventory.policy_digest = policy_digest;
        Ok(inventory)
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
        Self::from_catalogs(policy, builtins, projects, directory)
    }

    pub fn from_catalogs(
        policy: Option<Config>,
        builtins: Catalog,
        projects: std::collections::BTreeMap<String, Entry>,
        directory: String,
    ) -> Result<Self> {
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
            policy_digest: None,
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

    fn mandatory(
        &self,
        categories: &std::collections::BTreeMap<String, super::categories::Category>,
    ) -> Vec<Value> {
        self.active
            .entries
            .iter()
            .filter(|(id, entry)| {
                self.configuration(id, entry)
                    .is_some_and(|rule| rule.enabled && rule.required)
            })
            .map(|(id, entry)| self.row(id, entry, categories))
            .collect()
    }

    fn mandatory_checks(&self) -> Vec<&super::CommandCheck> {
        self.effective
            .iter()
            .flat_map(|config| &config.checks)
            .filter(|check| check.required)
            .collect()
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
            "lifecycle":policy.rule_lifecycle.get(id),
            "category":super::categories::assigned(policy, categories, id, entry),
            "categories":super::categories::assignments(policy, categories, id, entry),
            "source":if entry.custom.is_some() { "project" } else { "builtin" },
            "configuration":self.configuration(id, entry),"enabled":self.enabled(id, entry),
            "overrides_builtin":entry.custom.is_some() && self.builtins.entries.contains_key(id)})
    }
}
