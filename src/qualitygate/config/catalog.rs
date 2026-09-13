//! Versioned rule packages and snapshot-independent policy resolution.

use super::{Config, CustomRule, RuleSetting, StandardReference, custom_validation, validation};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

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
#[serde(deny_unknown_fields)]
struct StandardRegistry {
    schema_version: u32,
    reviewed_on: String,
    purpose: String,
    source_policy: SourcePolicy,
    sources: Vec<StandardSource>,
    notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcePolicy {
    authority_order: Vec<String>,
    adoption_rule: String,
    freshness_rule: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardSource {
    id: String,
    organization: String,
    title: String,
    authority: String,
    kind: String,
    url: String,
    languages: Vec<String>,
    lifecycle_stages: Vec<String>,
    concerns: Vec<String>,
    summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleMatrix {
    schema_version: u32,
    updated: String,
    title: String,
    policy: LifecyclePolicy,
    taxonomy: LifecycleTaxonomy,
    inputs: Vec<LifecycleInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecyclePolicy {
    missing_required_evidence: String,
    enforced_input_rule: String,
    evidence_contract_rule: String,
    conflict_rule: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleTaxonomy {
    lifecycle_stages: Vec<String>,
    languages: Vec<String>,
    concerns: Vec<String>,
    enforcement: Vec<String>,
    outcomes: Vec<String>,
    statuses: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum LifecycleInputStatus {
    Enforced,
    EvidenceContract,
    Planned,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

const SOURCE_AUTHORITIES: &[&str] = &[
    "repository-policy",
    "task-contract",
    "standards-body",
    "industry-standard",
    "industry-consortium",
    "language-ecosystem",
    "official-company",
    "company-project",
    "project-guide",
    "advisory",
];
const SOURCE_KINDS: &[&str] = &[
    "guide",
    "repository",
    "web",
    "ebook",
    "web-and-pdf",
    "documentation",
    "standard-and-pdf",
    "standard",
    "specification",
];
const ARCHIVE_LANGUAGES: &[&str] = &[
    "all",
    "java",
    "python",
    "rust",
    "cpp",
    "cuda",
    "typescript",
    "go",
    "c",
    "objective-c",
];
const ARCHIVE_CONCERNS: &[&str] = &[
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
    "correctness",
    "privacy",
    "static-gate",
    "supply-chain",
    "governance",
    "change-management",
];
const INPUT_LANGUAGES: &[&str] = &[
    "all",
    "java",
    "python",
    "rust",
    "cpp",
    "cuda",
    "typescript",
    "go",
];
const INPUT_CONCERNS: &[&str] = &[
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
];
const LIFECYCLE_STAGES: &[&str] = &[
    "plan",
    "architecture",
    "implementation",
    "review",
    "static-analysis",
    "verification",
    "release",
    "operations",
];
const ENFORCEMENT_KINDS: &[&str] = &[
    "deterministic-static",
    "semantic-static",
    "external-report",
    "design-evidence",
    "benchmark-evidence",
    "operational-evidence",
];
const OUTCOMES: &[&str] = &["violation", "warning", "incomplete", "advisory"];
const INPUT_STATUSES: &[&str] = &["enforced", "evidence-contract", "planned"];

fn nonempty_unique(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn allowed(values: &[String], choices: &[&str]) -> bool {
    nonempty_unique(values) && values.iter().all(|value| choices.contains(&value.as_str()))
}

fn exact_membership(values: &[String], choices: &[&str]) -> bool {
    allowed(values, choices) && values.len() == choices.len()
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn valid_date(value: &str) -> bool {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<u32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return false,
    };
    (2000..=9999).contains(&year) && (1..=days).contains(&day)
}

fn valid_https_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn source_policy_is_valid(policy: &SourcePolicy) -> bool {
    allowed(&policy.authority_order, SOURCE_AUTHORITIES)
        && policy
            .authority_order
            .first()
            .is_some_and(|value| value == "repository-policy")
        && valid_text(&policy.adoption_rule, 1024)
        && valid_text(&policy.freshness_rule, 1024)
}

fn source_is_valid(source: &StandardSource, policy: &SourcePolicy) -> bool {
    super::constraints::id(&source.id).is_ok()
        && valid_text(&source.organization, 256)
        && valid_text(&source.title, 512)
        && SOURCE_AUTHORITIES.contains(&source.authority.as_str())
        && policy
            .authority_order
            .iter()
            .any(|authority| authority == &source.authority)
        && SOURCE_KINDS.contains(&source.kind.as_str())
        && valid_https_url(&source.url)
        && allowed(&source.languages, ARCHIVE_LANGUAGES)
        && allowed(&source.lifecycle_stages, LIFECYCLE_STAGES)
        && allowed(&source.concerns, ARCHIVE_CONCERNS)
        && valid_text(&source.summary, 2048)
}

fn matrix_metadata_is_valid(matrix: &LifecycleMatrix) -> bool {
    matrix.schema_version == 1
        && valid_date(&matrix.updated)
        && valid_text(&matrix.title, 512)
        && matrix.policy.missing_required_evidence == "incomplete"
        && valid_text(&matrix.policy.enforced_input_rule, 2048)
        && valid_text(&matrix.policy.evidence_contract_rule, 2048)
        && valid_text(&matrix.policy.conflict_rule, 2048)
        && exact_membership(&matrix.taxonomy.lifecycle_stages, LIFECYCLE_STAGES)
        && exact_membership(&matrix.taxonomy.languages, INPUT_LANGUAGES)
        && exact_membership(&matrix.taxonomy.concerns, INPUT_CONCERNS)
        && exact_membership(&matrix.taxonomy.enforcement, ENFORCEMENT_KINDS)
        && exact_membership(&matrix.taxonomy.outcomes, OUTCOMES)
        && exact_membership(&matrix.taxonomy.statuses, INPUT_STATUSES)
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
    if registry.schema_version != 3
        || registry.sources.is_empty()
        || !valid_date(&registry.reviewed_on)
        || !valid_text(&registry.purpose, 1024)
        || !source_policy_is_valid(&registry.source_policy)
        || !nonempty_unique(&registry.notes)
        || registry.notes.iter().any(|note| !valid_text(note, 2048))
    {
        bail!("Built-in standards registry has invalid metadata or no sources");
    }
    let mut source_ids = BTreeSet::new();
    for source in registry.sources {
        if !source_is_valid(&source, &registry.source_policy)
            || !source_ids.insert(source.id.clone())
        {
            bail!("Built-in standards registry has duplicate, invalid, or incomplete sources");
        }
    }

    let matrix: LifecycleMatrix =
        serde_norway::from_str(matrix_yaml).context("Invalid lifecycle rule input matrix")?;
    if !matrix_metadata_is_valid(&matrix) || matrix.inputs.is_empty() {
        bail!("Lifecycle rule input matrix has invalid metadata or no inputs");
    }
    let mut inputs = BTreeMap::new();
    for input in matrix.inputs {
        let outcome_matches_status = match input.status {
            LifecycleInputStatus::Enforced => {
                ["violation", "warning"].contains(&input.outcome.as_str())
            }
            LifecycleInputStatus::EvidenceContract => input.outcome == "incomplete",
            LifecycleInputStatus::Planned => input.outcome == "advisory",
        };
        if super::constraints::id(&input.id).is_err()
            || input
                .rule_id
                .as_ref()
                .is_some_and(|rule_id| super::constraints::id(rule_id).is_err())
            || !valid_text(&input.applicability, 4096)
            || !valid_text(&input.critical_adoption, 4096)
            || !allowed(&input.languages, INPUT_LANGUAGES)
            || !allowed(&input.concerns, INPUT_CONCERNS)
            || !LIFECYCLE_STAGES.contains(&input.lifecycle_stage.as_str())
            || !ENFORCEMENT_KINDS.contains(&input.enforcement.as_str())
            || !OUTCOMES.contains(&input.outcome.as_str())
            || !nonempty_unique(&input.source_ids)
            || !nonempty_unique(&input.evidence)
            || input
                .evidence
                .iter()
                .any(|evidence| super::constraints::id(evidence).is_err())
            || input.source_ids.iter().any(|id| !source_ids.contains(id))
            || !outcome_matches_status
            || (input.status == LifecycleInputStatus::Enforced
                && (input.rule_id.is_none() || !supported_enforced_input(&input)))
            || (input.status != LifecycleInputStatus::Enforced && input.rule_id.is_some())
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
