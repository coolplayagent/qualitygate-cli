//! Validated candidate-policy transactions with atomic publication and conflict detection.

use super::{Config, categories, rule_query::Inventory};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "snake_case")]
pub enum Mutation {
    Lifecycle {
        id: String,
        record: crate::domain::rule_lifecycle::RuleLifecycle,
    },
    Enable(String),
    Disable(String),
    Configure {
        id: String,
        parameters: Vec<String>,
        severity: Option<crate::domain::Severity>,
        required: Option<bool>,
    },
    Assign {
        id: String,
        category: String,
    },
    Unassign {
        id: String,
        category: String,
    },
    Create {
        name: String,
        description: String,
    },
    Rename {
        name: String,
        new_name: String,
    },
    Delete {
        name: String,
        force: bool,
    },
}

struct Lock {
    path: PathBuf,
    _file: std::fs::File,
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn lock(root: &Path, path: &Path) -> Result<Lock> {
    let name = crate::paths::from_native(path)?;
    let path = crate::paths::confined(root, Path::new(&format!("{name}.lock")))?;
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
        .context("Cannot acquire candidate configuration lock; another writer may be active (a crashed writer leaves its .lock for explicit recovery)")?;
    let mut lock = Lock { path, _file: file };
    lock._file
        .write_all(b"qualitygate candidate configuration transaction\n")?;
    Ok(lock)
}

fn typed(value: &Value) -> Result<Config> {
    let config: Config = serde_json::from_value(value.clone())?;
    super::validation::layout(&config, false)?;
    Ok(config)
}

/// The returned before/after hashes and operation describe this one candidate edit.
/// No source review or trusted-policy approval is manufactured by this operation.
pub fn update(root: &Path, path: &Path, mutation: Mutation) -> Result<Value> {
    let root = dunce::canonicalize(root)?;
    if matches!(
        mutation,
        Mutation::Enable(_)
            | Mutation::Disable(_)
            | Mutation::Configure { .. }
            | Mutation::Lifecycle { .. }
    ) && super::policy_store::Store::optional(&root)?.is_some()
    {
        bail!(
            "Semantic edits in a governed repository require policy candidate rules with evidence and independent validation"
        );
    }
    let target = crate::paths::confined(&root, path)?;
    let _lock = lock(&root, path)?;
    let original = super::rule_authoring::read_file(&root, &crate::paths::from_native(path)?)?;
    let yaml: serde_norway::Value = super::parse_yaml(&original)?;
    let mut value = serde_json::to_value(yaml)?;
    let config = typed(&value)?;
    // Load every package once so a discovered, unselected built-in can be managed.
    // Project definitions remain explicitly selected through custom_rules.
    let inventory = Inventory::for_policy(&root, Some(config.clone()), true)?;
    let result = edit_value(&mut value, &config, &inventory, &mutation)?;
    let candidate = typed(&value)?;
    inventory.selected(&candidate).resolve(&candidate)?;
    let bytes = serde_norway::to_string(&value)?.into_bytes();
    if bytes.len() > super::MAX_CONFIG_BYTES {
        bail!("Configuration exceeds 1 MiB");
    }
    // Retain exact bytes on semantic no-ops, including comments and formatting.
    let changed =
        serde_json::to_value(super::parse_yaml::<serde_norway::Value>(&original)?)? != value;
    if changed {
        publish(&root, path, &target, &original, &bytes)?;
    }
    Ok(
        json!({"schema_version":1,"status":"candidate","changed":changed,"mutation":mutation,
        "before_digest":digest(&original),"after_digest":digest(if changed { &bytes } else { &original }),
        "result":result,"review_trust":"local_candidate"}),
    )
}

fn publish(root: &Path, path: &Path, target: &Path, original: &[u8], bytes: &[u8]) -> Result<()> {
    let mut temporary =
        tempfile::NamedTempFile::new_in(target.parent().context("Configuration has no parent")?)?;
    temporary
        .as_file()
        .set_permissions(std::fs::metadata(target)?.permissions())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    // Reject non-cooperating edits observed since the transaction read.
    if crate::paths::confined(root, path)? != target
        || super::rule_authoring::read_file(root, &crate::paths::from_native(path)?)? != original
    {
        bail!("Configuration changed during transaction; retry against current bytes");
    }
    temporary.persist(target)?;
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn object<'a>(value: &'a mut Value, name: &str) -> Result<&'a mut serde_json::Map<String, Value>> {
    if value[name].is_null() {
        value[name] = json!({});
    }
    value[name]
        .as_object_mut()
        .with_context(|| format!("{name} must be a mapping"))
}

pub(super) fn edit_value(
    value: &mut Value,
    config: &Config,
    inventory: &Inventory,
    mutation: &Mutation,
) -> Result<Value> {
    let mut registry = categories::registry(config);
    match mutation {
        Mutation::Create { name, description } => {
            if registry.contains_key(name) {
                bail!("Category already exists: {name}");
            }
            registry.insert(
                name.clone(),
                categories::Category {
                    description: description.clone(),
                    origin: None,
                },
            );
        }
        Mutation::Rename { name, new_name } => {
            if registry.contains_key(new_name) {
                bail!("Category already exists: {new_name}");
            }
            let category = registry
                .remove(name)
                .with_context(|| format!("Unknown category: {name}"))?;
            registry.insert(new_name.clone(), category);
            for (id, membership) in &config.rule_categories {
                let names: Vec<_> = membership
                    .names()
                    .iter()
                    .map(|current| {
                        if current == name {
                            new_name.clone()
                        } else {
                            current.clone()
                        }
                    })
                    .collect();
                object(value, "rule_categories")?.insert(id.clone(), membership_value(names));
            }
        }
        Mutation::Delete { name, force } => {
            if registry.remove(name).is_none() {
                bail!("Unknown category: {name}");
            }
            if !force
                && config
                    .rule_categories
                    .values()
                    .any(|membership| membership.contains(name))
            {
                bail!("Category {name} has explicit assignments; use --force to restore defaults");
            }
            for (id, membership) in &config.rule_categories {
                let names: Vec<_> = membership
                    .names()
                    .iter()
                    .filter(|current| *current != name)
                    .cloned()
                    .collect();
                if names.is_empty() {
                    object(value, "rule_categories")?.remove(id);
                } else {
                    object(value, "rule_categories")?.insert(id.clone(), membership_value(names));
                }
            }
        }
        Mutation::Assign { id, category } | Mutation::Unassign { id, category } => {
            if !inventory.builtins.entries.contains_key(id) && !inventory.projects.contains_key(id)
            {
                bail!("Unknown rule: {id}");
            }
            if !registry.contains_key(category) {
                bail!("Unknown category: {category}");
            }
            let mut names = config
                .rule_categories
                .get(id)
                .map(|membership| membership.names().to_vec())
                .unwrap_or_default();
            if matches!(mutation, Mutation::Unassign { .. }) {
                if !names.contains(category) {
                    bail!("Rule {id} has no explicit membership in {category}");
                }
                names.retain(|name| name != category);
            } else if !names.contains(category) {
                names.push(category.clone());
            }
            if names.is_empty() {
                object(value, "rule_categories")?.remove(id);
            } else {
                object(value, "rule_categories")?
                    .insert(id.clone(), membership_value(names.clone()));
            }
            return Ok(json!({"id":id,"categories":names}));
        }
        Mutation::Lifecycle { id, record } => {
            use crate::domain::rule_lifecycle::RuleState;
            record.validate().map_err(anyhow::Error::msg)?;
            let change = match record.state {
                RuleState::Revalidate => Mutation::Enable(id.clone()),
                RuleState::Demoted => Mutation::Configure {
                    id: id.clone(),
                    parameters: Vec::new(),
                    severity: Some(crate::domain::Severity::Warning),
                    required: Some(false),
                },
                RuleState::Deprecated => Mutation::Configure {
                    id: id.clone(),
                    parameters: Vec::new(),
                    severity: None,
                    required: Some(
                        inventory
                            .selected(config)
                            .resolve(config)?
                            .rules
                            .get(id)
                            .with_context(|| format!("Unknown selected rule: {id}"))?
                            .required,
                    ),
                },
                RuleState::Retired | RuleState::Revoked => Mutation::Disable(id.clone()),
            };
            let result = edit_value(value, config, inventory, &change)?;
            object(value, "rule_lifecycle")?.insert(id.clone(), serde_json::to_value(record)?);
            return Ok(json!({"id":id,"lifecycle":record,"configuration":result["configuration"]}));
        }
        Mutation::Enable(id) | Mutation::Disable(id) | Mutation::Configure { id, .. } => {
            let active = inventory.selected(config);
            let entry = active
                .entries
                .get(id)
                .or_else(|| inventory.projects.get(id))
                .or_else(|| inventory.builtins.entries.get(id))
                .with_context(|| format!("Unknown rule: {id}"))?;
            if entry.custom.is_some() && config.custom_rules.is_none() {
                value["custom_rules"] = json!(super::catalog::PROJECT_RULES_DIR);
            }
            if !["core", "shared", "custom"].contains(&entry.package.as_str())
                && !config.rulesets.contains(&entry.package)
            {
                let mut packages = config.rulesets.clone();
                packages.push(entry.package.clone());
                value["rulesets"] = json!(packages);
            }
            let rule = object(value, "rules")?
                .entry(id)
                .or_insert_with(|| json!({"enabled":false}));
            if let Mutation::Configure {
                parameters,
                severity,
                required,
                ..
            } = mutation
            {
                if parameters.is_empty() && severity.is_none() && required.is_none() {
                    bail!("Configure needs --param, --severity or --required");
                }
                if !parameters.is_empty() {
                    let builtin = entry.builtin.as_ref().context("Project rules declare assertions in their definition; parameters are unsupported")?;
                    let mut settings = serde_json::to_value(entry.defaults().parameters)?;
                    if let Some(current) = rule["parameters"].as_object() {
                        settings
                            .as_object_mut()
                            .expect("parameter defaults map")
                            .extend(current.clone());
                    }
                    super::parameters::apply(&mut settings, parameters, &builtin.implementation)?;
                    // Persist only touched top-level parameters; keep inherited defaults live.
                    for edit in parameters {
                        let name = edit
                            .split_once('=')
                            .expect("validated edit")
                            .0
                            .split('.')
                            .next()
                            .expect("parameter key");
                        object(rule, "parameters")?.insert(name.into(), settings[name].clone());
                    }
                }
                if let Some(severity) = severity {
                    rule["severity"] = json!(severity);
                }
                if let Some(required) = required {
                    rule["required"] = json!(required);
                }
            } else {
                rule["enabled"] = json!(matches!(mutation, Mutation::Enable(_)));
            }
            let result = rule.clone();
            if matches!(mutation, Mutation::Enable(_))
                && let Some(profiles) = value.get_mut("profiles").and_then(Value::as_object_mut)
            {
                for profile in profiles.values_mut() {
                    let include = profile["include"]
                        .as_array_mut()
                        .context("Profile include must be an array")?;
                    if !include.iter().any(|included| included == id) {
                        include.push(json!(id));
                    }
                }
            }
            return Ok(json!({"id":id,"configuration":result}));
        }
    }
    value["categories"] = serde_json::to_value(&registry)?;
    match mutation {
        Mutation::Create { name, .. } | Mutation::Rename { new_name: name, .. } => {
            let category = &registry[name];
            Ok(
                json!({"name":name,"description":category.description,"custom":category.origin.is_none()}),
            )
        }
        Mutation::Delete { name, force } => Ok(json!({"deleted":name,"force":force})),
        _ => unreachable!("rule mutations return their own result"),
    }
}

fn membership_value(names: Vec<String>) -> Value {
    if names.len() == 1 {
        json!(names[0])
    } else {
        json!(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimistic_conflict_preserves_the_other_writers_bytes() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join(super::super::CONFIG_FILE);
        let other = b"schema_version: 1\nrules: {}\n";
        std::fs::write(&target, other).unwrap();
        let before: Vec<_> = std::fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let error = publish(
            root.path(),
            Path::new(super::super::CONFIG_FILE),
            &target,
            b"old bytes",
            b"candidate bytes",
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed during transaction"));
        assert_eq!(std::fs::read(&target).unwrap(), other);
        let after: Vec<_> = std::fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(before, after);
    }
}
