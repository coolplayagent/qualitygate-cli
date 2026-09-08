//! Versioned rule packages and snapshot-independent policy resolution.

use super::{Config, CustomRule, RuleSetting, custom_validation, validation};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Builtin {
    pub id: String,
    pub version: u32,
    pub description: String,
    pub implementation: String,
    #[serde(default)]
    pub language: Vec<String>,
    pub requires_capabilities: Vec<String>,
    pub defaults: RuleSetting,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub origin: String,
    pub package: String,
    pub builtin: Option<Builtin>,
    pub custom: Option<CustomRule>,
}

impl Entry {
    pub fn defaults(&self) -> RuleSetting {
        if let Some(builtin) = &self.builtin {
            builtin.defaults.clone()
        } else {
            let rule = self
                .custom
                .as_ref()
                .expect("catalog entry has a definition");
            RuleSetting {
                required: rule.required,
                severity: rule.severity,
                source: Some(rule.source.clone()),
                ..RuleSetting::default()
            }
        }
    }

    pub fn version(&self) -> u32 {
        self.builtin
            .as_ref()
            .map(|rule| rule.version)
            .or_else(|| self.custom.as_ref().map(|rule| rule.version))
            .expect("catalog entry has a definition")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    pub entries: BTreeMap<String, Entry>,
}

const PACKAGED: &[(&str, &str, &str)] = &[
    (
        "core",
        "line-ending.yaml",
        include_str!("../../../qualitygate/rules/core/line-ending.yaml"),
    ),
    (
        "core",
        "commit-message.yaml",
        include_str!("../../../qualitygate/rules/core/commit-message.yaml"),
    ),
    (
        "core",
        "diff-size.yaml",
        include_str!("../../../qualitygate/rules/core/diff-size.yaml"),
    ),
    (
        "shared",
        "test-naming.yaml",
        include_str!("../../../qualitygate/rules/shared/test-naming.yaml"),
    ),
    (
        "shared",
        "parameterized-tests.yaml",
        include_str!("../../../qualitygate/rules/shared/parameterized-tests.yaml"),
    ),
    (
        "shared",
        "comment-language.yaml",
        include_str!("../../../qualitygate/rules/shared/comment-language.yaml"),
    ),
    (
        "shared",
        "ai-code-traceability.yaml",
        include_str!("../../../qualitygate/rules/shared/ai-code-traceability.yaml"),
    ),
    (
        "lang-java",
        "junit-naming.yaml",
        include_str!("../../../qualitygate/rules/lang-java/junit-naming.yaml"),
    ),
    (
        "lang-python",
        "pytest-naming.yaml",
        include_str!("../../../qualitygate/rules/lang-python/pytest-naming.yaml"),
    ),
];

/// Local discovery for config/list/enable; checks use `load` with immutable files.
pub fn read(root: &std::path::Path, config: &Config) -> Result<Catalog> {
    let mut files = BTreeMap::new();
    if let Some(directory) = &config.custom_rules {
        let mut pending = vec![crate::paths::confined(root, directory.as_ref())?];
        let mut entries = 0;
        let mut total = 0;
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory)
                .with_context(|| format!("Cannot read rule directory {}", directory.display()))?
            {
                let entry = entry?;
                entries += 1;
                if entries > 4096 {
                    bail!("Custom rule directory exceeds discovery budget");
                }
                let relative = entry.path().strip_prefix(root)?.to_path_buf();
                let path = crate::paths::confined(root, &relative)?;
                if entry.file_type()?.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "yaml" || extension == "yml")
                {
                    total += entry.metadata()?.len();
                    if total > super::MAX_CONFIG_BYTES as u64 {
                        bail!("Custom rules exceed 1 MiB");
                    }
                    files.insert(crate::paths::relative(&relative)?, std::fs::read(path)?);
                }
            }
        }
    }
    Catalog::load(
        config,
        files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
}

impl Catalog {
    /// Files must come from the selected policy snapshot, never implicitly from HEAD.
    pub fn load<'a>(
        config: &Config,
        files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    ) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for (package, path, yaml) in PACKAGED {
            if !["core", "shared"].contains(package)
                && !config.rulesets.iter().any(|name| name == package)
            {
                continue;
            }
            let rule: Builtin = serde_norway::from_str(yaml)?;
            entries.insert(
                rule.id.clone(),
                Entry {
                    origin: format!("embedded:qualitygate/rules/{package}/{path}"),
                    package: (*package).into(),
                    builtin: Some(rule),
                    custom: None,
                },
            );
        }
        if let Some(directory) = &config.custom_rules {
            let prefix = format!("{}/", directory.trim_end_matches('/'));
            let mut ids = BTreeSet::new();
            let mut total = 0;
            for (path, bytes) in files {
                if !path.starts_with(&prefix)
                    || !(path.ends_with(".yaml") || path.ends_with(".yml"))
                {
                    continue;
                }
                total += bytes.len();
                if total > super::MAX_CONFIG_BYTES || ids.len() >= 256 {
                    bail!("Custom rule package exceeds 256 files or 1 MiB");
                }
                let rule: CustomRule = serde_norway::from_slice(bytes)
                    .with_context(|| format!("Invalid custom rule: {path}"))?;
                custom_validation::validate(&rule)
                    .with_context(|| format!("Invalid rule {path}"))?;
                if !ids.insert(rule.id.clone()) {
                    bail!("Duplicate custom rule id: {}", rule.id);
                }
                // Selecting a custom definition explicitly replaces a packaged definition
                // with the same ID. Its origin and complete definition remain in evidence.
                entries.insert(
                    rule.id.clone(),
                    Entry {
                        origin: path.into(),
                        package: "custom".into(),
                        builtin: None,
                        custom: Some(rule),
                    },
                );
            }
            if ids.is_empty() {
                bail!("Custom rule directory contains no YAML definitions: {directory}");
            }
        }
        Ok(Self { entries })
    }

    /// Apply definition defaults only when the policy did not explicitly override them.
    pub fn resolve(&self, config: &Config) -> Result<Config> {
        let mut effective = config.clone();
        for (id, setting) in &mut effective.rules {
            let entry = self
                .entries
                .get(id)
                .with_context(|| format!("Unknown or unavailable rule: {id}"))?;
            let defaults = entry.defaults();
            if !setting.specified.contains("required") {
                setting.required = defaults.required;
            }
            if !setting.specified.contains("severity") {
                setting.severity = defaults.severity;
            }
            if entry.custom.is_some() && !setting.parameters.is_empty() {
                bail!(
                    "Custom rule {id} declares assertions in its definition; parameters are unsupported"
                );
            }
            let mut parameters = defaults.parameters;
            parameters.extend(setting.parameters.clone());
            setting.parameters = parameters;
            if let Some(builtin) = &entry.builtin {
                super::builtin_validation::validate(&builtin.implementation, setting)?;
            }
            if setting.source.is_none() {
                setting.source = defaults.source;
            }
        }
        validation::validate(&effective)?;
        Ok(effective)
    }
}
