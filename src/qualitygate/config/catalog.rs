//! Versioned rule packages and snapshot-independent policy resolution.

use super::{Config, CustomRule, RuleSetting, StandardReference, custom_validation, validation};
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
    #[serde(default)]
    pub standard_refs: Vec<StandardReference>,
    #[serde(default)]
    pub lifecycle_inputs: Vec<String>,
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
        "lang-java",
        "used-undeclared.yaml",
        include_str!("../../../qualitygate/rules/lang-java/used-undeclared.yaml"),
    ),
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
        "module-boundary.yaml",
        include_str!("../../../qualitygate/rules/lang-java/module-boundary.yaml"),
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

#[derive(Debug, Deserialize)]
struct StandardRegistry {
    schema_version: u32,
    sources: Vec<StandardSource>,
    #[serde(flatten)]
    _metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StandardSource {
    id: String,
    #[serde(flatten)]
    _metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LifecycleMatrix {
    schema_version: u32,
    inputs: Vec<LifecycleInput>,
    #[serde(flatten)]
    _metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum LifecycleInputStatus {
    Enforced,
    EvidenceContract,
    Planned,
}

#[derive(Debug, Deserialize)]
struct LifecycleInput {
    id: String,
    status: LifecycleInputStatus,
    rule_id: Option<String>,
    lifecycle_stage: String,
    languages: Vec<String>,
    concerns: Vec<String>,
    enforcement: String,
    outcome: String,
    source_ids: Vec<String>,
    evidence: Vec<String>,
    applicability: String,
    critical_adoption: String,
}

#[derive(Debug)]
struct Standards {
    source_ids: BTreeSet<String>,
    inputs: BTreeMap<String, LifecycleInput>,
}

const STANDARD_REGISTRY: &str =
    include_str!("../../../knowledge/best-practices/engineering-standards/registry.yaml");
const LIFECYCLE_MATRIX: &str = include_str!(
    "../../../knowledge/best-practices/engineering-standards/lifecycle-rule-matrix.yaml"
);

fn nonempty_unique(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn allowed(values: &[String], choices: &[&str]) -> bool {
    nonempty_unique(values) && values.iter().all(|value| choices.contains(&value.as_str()))
}

fn supported_enforced_input(input: &LifecycleInput) -> bool {
    ["deterministic-static", "semantic-static"].contains(&input.enforcement.as_str())
        || (input.enforcement == "external-report"
            && input
                .evidence
                .iter()
                .any(|item| item == "selected-snapshot" || item == "snapshot-digest"))
}

fn standards_from(registry_yaml: &str, matrix_yaml: &str) -> Result<Standards> {
    let registry: StandardRegistry =
        serde_norway::from_str(registry_yaml).context("Invalid built-in standards registry")?;
    if registry.schema_version != 2 || registry.sources.is_empty() {
        bail!("Built-in standards registry has an unsupported schema or no sources");
    }
    let mut source_ids = BTreeSet::new();
    for source in registry.sources {
        if source.id.trim().is_empty() || !source_ids.insert(source.id) {
            bail!("Built-in standards registry has duplicate or empty source IDs");
        }
    }

    let matrix: LifecycleMatrix =
        serde_norway::from_str(matrix_yaml).context("Invalid lifecycle rule input matrix")?;
    if matrix.schema_version != 1 || matrix.inputs.is_empty() {
        bail!("Lifecycle rule input matrix has an unsupported schema or no inputs");
    }
    let mut inputs = BTreeMap::new();
    for input in matrix.inputs {
        if input.id.trim().is_empty()
            || input.lifecycle_stage.trim().is_empty()
            || input.enforcement.trim().is_empty()
            || input.outcome.trim().is_empty()
            || input.applicability.trim().is_empty()
            || input.critical_adoption.trim().is_empty()
            || !allowed(
                &input.languages,
                &[
                    "all",
                    "java",
                    "python",
                    "rust",
                    "cpp",
                    "cuda",
                    "typescript",
                    "go",
                ],
            )
            || !allowed(
                &input.concerns,
                &[
                    "coding",
                    "documentation",
                    "testing",
                    "architecture",
                    "dependencies",
                    "security",
                    "performance",
                    "reliability",
                    "reviewability",
                    "quality-gate",
                    "operations",
                ],
            )
            || ![
                "plan",
                "architecture",
                "implementation",
                "review",
                "static-analysis",
                "verification",
                "release",
                "operations",
            ]
            .contains(&input.lifecycle_stage.as_str())
            || ![
                "deterministic-static",
                "semantic-static",
                "external-report",
                "design-evidence",
                "benchmark-evidence",
                "operational-evidence",
            ]
            .contains(&input.enforcement.as_str())
            || !["violation", "warning", "incomplete", "advisory"].contains(&input.outcome.as_str())
            || !nonempty_unique(&input.source_ids)
            || !nonempty_unique(&input.evidence)
            || input.source_ids.iter().any(|id| !source_ids.contains(id))
            || (input.status == LifecycleInputStatus::Enforced
                && (input
                    .rule_id
                    .as_deref()
                    .is_none_or(|rule_id| rule_id.trim().is_empty())
                    || !supported_enforced_input(&input)))
        {
            bail!("Lifecycle rule input matrix has invalid input metadata");
        }
        if inputs.insert(input.id.clone(), input).is_some() {
            bail!("Lifecycle rule input matrix has duplicate input IDs");
        }
    }
    Ok(Standards { source_ids, inputs })
}

#[cfg(test)]
pub(super) fn validate_standard_inputs(registry_yaml: &str, matrix_yaml: &str) -> Result<()> {
    standards_from(registry_yaml, matrix_yaml).map(|_| ())
}

fn validate_builtin_standards(rule: &Builtin, standards: &Standards) -> Result<BTreeSet<String>> {
    if rule.standard_refs.is_empty() || rule.lifecycle_inputs.is_empty() {
        bail!(
            "Built-in rule {} must declare standard references and lifecycle inputs",
            rule.id
        );
    }
    let mut reference_sources = BTreeSet::new();
    for reference in &rule.standard_refs {
        if reference.source_id.trim().is_empty()
            || !nonempty_unique(&reference.controls)
            || !standards.source_ids.contains(&reference.source_id)
            || !reference_sources.insert(reference.source_id.clone())
        {
            bail!(
                "Built-in rule {} references an invalid or unarchived standard source",
                rule.id
            );
        }
    }
    let mut input_sources = BTreeSet::new();
    let mut input_ids = BTreeSet::new();
    for input_id in &rule.lifecycle_inputs {
        let input = standards.inputs.get(input_id).with_context(|| {
            format!(
                "Built-in rule {} references an unknown lifecycle input {input_id}",
                rule.id
            )
        })?;
        if input.status != LifecycleInputStatus::Enforced
            || input.rule_id.as_deref() != Some(rule.id.as_str())
            || !input_ids.insert(input_id.clone())
        {
            bail!(
                "Built-in rule {} has an invalid lifecycle input mapping",
                rule.id
            );
        }
        input_sources.extend(input.source_ids.iter().cloned());
    }
    if reference_sources != input_sources {
        bail!(
            "Built-in rule {} standard references do not match its lifecycle inputs",
            rule.id
        );
    }
    Ok(input_ids)
}

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
                let relative = crate::paths::from_native(entry.path().strip_prefix(root)?)?;
                let path = crate::paths::confined(root, relative.as_ref())?;
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
                    files.insert(relative, std::fs::read(path)?);
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
        let standards = standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX)?;
        let mut entries = BTreeMap::new();
        let mut mapped_inputs = BTreeSet::new();
        for (package, path, yaml) in PACKAGED {
            let rule: Builtin = super::parse_yaml(yaml.as_bytes())?;
            mapped_inputs.extend(validate_builtin_standards(&rule, &standards)?);
            if !["core", "shared"].contains(package)
                && !config.rulesets.iter().any(|name| name == package)
            {
                continue;
            }
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
        for input in standards.inputs.values() {
            if input.status == LifecycleInputStatus::Enforced && !mapped_inputs.contains(&input.id)
            {
                bail!(
                    "Enforced lifecycle input {} is not mapped by a packaged built-in rule",
                    input.id
                );
            }
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
                let rule: CustomRule = super::parse_yaml(bytes)
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
        super::source_reviews::evidence(&effective, self)?;
        Ok(effective)
    }
}
