//! Versioned rule packages and snapshot-independent policy resolution.

use super::{
    Config, CustomRule, RuleSetting, StandardReference, catalog_assets::packaged_rules, validation,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub entries: BTreeMap<String, Entry>,
}

pub const PROJECT_RULES_DIR: &str = "qualitygate/rules";

/// Project rules stay under the selected policy snapshot and require an
/// explicit normalized directory in `custom_rules`.
pub fn project_rules_directory(config: &Config) -> Option<&str> {
    config.custom_rules.as_deref()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardRegistry {
    schema_version: u32,
    reviewed_on: String,
    purpose: String,
    source_policy: SourcePolicy,
    coverage: ArchiveCoverage,
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

/// The deliberately reviewed breadth of this archive. These labels are not
/// policy requirements for a checked repository; they ensure that a future
/// archive edit cannot silently drop one of the research lanes it claims.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveCoverage {
    organizations: Vec<String>,
    languages: Vec<String>,
    lifecycle_stages: Vec<String>,
    concerns: Vec<String>,
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
    controls: Vec<String>,
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
struct LifecycleSupplement {
    schema_version: u32,
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
    source_controls: BTreeMap<String, BTreeSet<String>>,
    inputs: BTreeMap<String, LifecycleInput>,
}

const STANDARD_REGISTRY: &str =
    include_str!("../../../knowledge/best-practices/engineering-standards/registry.yaml");
const LIFECYCLE_MATRIX: &str = include_str!(
    "../../../knowledge/best-practices/engineering-standards/lifecycle-rule-matrix.yaml"
);
const LIFECYCLE_SUPPLEMENTS: &[&str] = &[include_str!(
    "../../../knowledge/best-practices/engineering-standards/lifecycle-rule-matrix-python.yaml"
)];

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
    "shell",
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
    "c",
    "cuda",
    "typescript",
    "go",
    "shell",
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
    "static-gate",
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
const REQUIRED_ARCHIVE_ORGANIZATIONS: &[&str] = &[
    "Alibaba",
    "Google",
    "Huawei Cloud",
    "NVIDIA",
    "AWS",
    "Microsoft Azure",
    "Cloudflare",
    "Meta",
];
const REQUIRED_ARCHIVE_LANGUAGES: &[&str] = &[
    "java",
    "python",
    "rust",
    "cpp",
    "cuda",
    "typescript",
    "go",
    "shell",
];
const REQUIRED_ARCHIVE_CONCERNS: &[&str] = &[
    "coding",
    "architecture",
    "security",
    "performance",
    "static-gate",
    "quality-gate",
];

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

fn exact_labels(values: &[String], required: &[&str]) -> bool {
    values.len() == required.len()
        && values.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == required.iter().copied().collect()
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

fn coverage_is_valid(coverage: &ArchiveCoverage) -> bool {
    nonempty_unique(&coverage.organizations)
        && coverage
            .organizations
            .iter()
            .all(|organization| valid_text(organization, 256))
        && allowed(&coverage.languages, INPUT_LANGUAGES)
        && !coverage.languages.iter().any(|language| language == "all")
        && allowed(&coverage.lifecycle_stages, LIFECYCLE_STAGES)
        && allowed(&coverage.concerns, INPUT_CONCERNS)
}

/// This bundled archive implements the research scope promised by this CLI,
/// rather than accepting a self-consistent but narrower future bibliography.
/// Fixture registries use `standards_from` directly and deliberately do not
/// inherit this product-level contract.
fn required_archive_coverage_is_valid(registry_yaml: &str) -> Result<()> {
    let registry: StandardRegistry =
        serde_norway::from_str(registry_yaml).context("Invalid bundled standards registry")?;
    if !exact_labels(
        &registry.coverage.organizations,
        REQUIRED_ARCHIVE_ORGANIZATIONS,
    ) || !exact_labels(&registry.coverage.languages, REQUIRED_ARCHIVE_LANGUAGES)
        || !exact_membership(&registry.coverage.lifecycle_stages, LIFECYCLE_STAGES)
        || !exact_labels(&registry.coverage.concerns, REQUIRED_ARCHIVE_CONCERNS)
    {
        bail!("Built-in standards registry no longer covers the required research lanes");
    }
    Ok(())
}

/// Declared archive breadth is only meaningful when it reaches the lifecycle
/// matrix. A universal input is useful for repository-wide controls, but it
/// cannot stand in for a declared language lane because it has no language
/// specific applicability or evidence boundary.
fn lifecycle_input_coverage_is_valid(
    coverage: &ArchiveCoverage,
    inputs: &BTreeMap<String, LifecycleInput>,
) -> Result<()> {
    let languages: BTreeSet<_> = inputs
        .values()
        .flat_map(|input| input.languages.iter().map(String::as_str))
        .filter(|language| *language != "all")
        .collect();
    if coverage
        .languages
        .iter()
        .any(|language| !languages.contains(language.as_str()))
    {
        bail!("Lifecycle rule input matrix omits a required archive language");
    }
    let stages: BTreeSet<_> = inputs
        .values()
        .map(|input| input.lifecycle_stage.as_str())
        .collect();
    if coverage
        .lifecycle_stages
        .iter()
        .any(|stage| !stages.contains(stage.as_str()))
    {
        bail!("Lifecycle rule input matrix omits a required archive lifecycle stage");
    }
    let concerns: BTreeSet<_> = inputs
        .values()
        .flat_map(|input| input.concerns.iter().map(String::as_str))
        .collect();
    if coverage
        .concerns
        .iter()
        .any(|concern| !concerns.contains(concern.as_str()))
    {
        bail!("Lifecycle rule input matrix omits a required archive concern");
    }
    Ok(())
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
        && nonempty_unique(&source.controls)
        && source
            .controls
            .iter()
            .all(|control| super::constraints::id(control).is_ok())
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

fn standards_from(
    registry_yaml: &str,
    matrix_yaml: &str,
    supplements: &[&str],
) -> Result<Standards> {
    let registry: StandardRegistry =
        serde_norway::from_str(registry_yaml).context("Invalid built-in standards registry")?;
    if registry.schema_version != 5
        || registry.sources.is_empty()
        || !valid_date(&registry.reviewed_on)
        || !valid_text(&registry.purpose, 1024)
        || !source_policy_is_valid(&registry.source_policy)
        || !coverage_is_valid(&registry.coverage)
        || !nonempty_unique(&registry.notes)
        || registry.notes.iter().any(|note| !valid_text(note, 2048))
    {
        bail!("Built-in standards registry has invalid metadata or no sources");
    }
    let mut source_ids = BTreeSet::new();
    let mut sources = BTreeMap::new();
    for source in registry.sources {
        if !source_is_valid(&source, &registry.source_policy)
            || !source_ids.insert(source.id.clone())
        {
            bail!("Built-in standards registry has duplicate, invalid, or incomplete sources");
        }
        sources.insert(source.id.clone(), source);
    }
    let source_coverage = |values: &[String], select: fn(&StandardSource) -> &[String]| {
        values.iter().all(|value| {
            sources
                .values()
                .any(|source| select(source).iter().any(|candidate| candidate == value))
        })
    };
    if !source_coverage(&registry.coverage.organizations, |source| {
        std::slice::from_ref(&source.organization)
    }) || !source_coverage(&registry.coverage.languages, |source| &source.languages)
        || !source_coverage(&registry.coverage.lifecycle_stages, |source| {
            &source.lifecycle_stages
        })
        || !source_coverage(&registry.coverage.concerns, |source| &source.concerns)
    {
        bail!("Built-in standards registry does not satisfy its declared coverage");
    }

    let mut matrix: LifecycleMatrix =
        serde_norway::from_str(matrix_yaml).context("Invalid lifecycle rule input matrix")?;
    if !matrix_metadata_is_valid(&matrix) || matrix.inputs.is_empty() {
        bail!("Lifecycle rule input matrix has invalid metadata or no inputs");
    }
    for source in supplements {
        let supplement: LifecycleSupplement = serde_norway::from_str(source)
            .context("Invalid lifecycle rule input matrix supplement")?;
        if supplement.schema_version != 1 || supplement.inputs.is_empty() {
            bail!("Lifecycle rule input matrix supplement has invalid metadata or no inputs");
        }
        matrix.inputs.extend(supplement.inputs);
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
    let mapped_organizations: BTreeSet<_> = inputs
        .values()
        .flat_map(|input| input.source_ids.iter())
        .filter_map(|source_id| sources.get(source_id))
        .map(|source| source.organization.as_str())
        .collect();
    if registry
        .coverage
        .organizations
        .iter()
        .any(|organization| !mapped_organizations.contains(organization.as_str()))
    {
        bail!("Lifecycle rule input matrix omits a required archive organization");
    }
    lifecycle_input_coverage_is_valid(&registry.coverage, &inputs)?;
    let source_controls = sources
        .iter()
        .map(|(id, source)| {
            (
                id.clone(),
                source.controls.iter().cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect();
    Ok(Standards {
        source_ids,
        source_controls,
        inputs,
    })
}

#[cfg(test)]
pub(super) fn validate_standard_inputs(registry_yaml: &str, matrix_yaml: &str) -> Result<()> {
    standards_from(registry_yaml, matrix_yaml, &[]).map(|_| ())
}

#[cfg(test)]
pub(super) fn validate_required_archive_coverage(registry_yaml: &str) -> Result<()> {
    required_archive_coverage_is_valid(registry_yaml)
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
        let declared_controls = standards.source_controls.get(&reference.source_id);
        if reference.source_id.trim().is_empty()
            || !nonempty_unique(&reference.controls)
            || !standards.source_ids.contains(&reference.source_id)
            || !declared_controls.is_some_and(|controls| {
                reference
                    .controls
                    .iter()
                    .all(|control| controls.contains(control))
            })
            || !reference_sources.insert(reference.source_id.clone())
        {
            bail!(
                "Built-in rule {} references an invalid, unarchived, or unsupported standard control",
                rule.id
            );
        }
    }
    let mut input_sources = BTreeSet::new();
    let mut input_ids = BTreeSet::new();
    let mut input_languages = BTreeSet::new();
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
        input_languages.extend(input.languages.iter().map(String::as_str));
    }
    if reference_sources != input_sources {
        bail!(
            "Built-in rule {} standard references do not match its lifecycle inputs",
            rule.id
        );
    }
    if !rule.language.is_empty()
        && (!allowed(&rule.language, INPUT_LANGUAGES)
            || rule.language.iter().any(|language| language == "all"))
    {
        bail!(
            "Built-in rule {} declares an invalid lifecycle language scope",
            rule.id
        );
    }
    let rule_languages: BTreeSet<_> = rule.language.iter().map(String::as_str).collect();
    if !rule_languages.is_empty()
        && (input_languages.contains("all") || rule_languages != input_languages)
    {
        bail!(
            "Built-in rule {} language scope does not match its lifecycle inputs",
            rule.id
        );
    }
    Ok(input_ids)
}

#[cfg(test)]
pub(super) fn validate_builtin_mapping(rule_yaml: &str) -> Result<()> {
    let rule: Builtin = super::parse_yaml(rule_yaml.as_bytes())?;
    let standards = standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS)?;
    validate_builtin_standards(&rule, &standards).map(|_| ())
}

/// Local discovery for config/list/enable; checks use `load` with immutable files.
pub fn read(root: &Path, config: &Config) -> Result<Catalog> {
    let files = super::project_inventory::read(root, config)?;
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
        required_archive_coverage_is_valid(STANDARD_REGISTRY)?;
        let standards = standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS)?;
        let mut entries = BTreeMap::new();
        let mut mapped_inputs = BTreeSet::new();
        for packaged in packaged_rules()? {
            let rule: Builtin = super::parse_yaml(&packaged.bytes).with_context(|| {
                format!(
                    "Invalid built-in skill rule: references/rules/{}/{}",
                    packaged.package, packaged.path
                )
            })?;
            mapped_inputs.extend(validate_builtin_standards(&rule, &standards)?);
            if !["core", "shared"].contains(&packaged.package.as_str())
                && !config.rulesets.iter().any(|name| name == &packaged.package)
            {
                continue;
            }
            if entries
                .insert(
                    rule.id.clone(),
                    Entry {
                        origin: format!(
                            "skill:references/rules/{}/{}",
                            packaged.package, packaged.path
                        ),
                        package: packaged.package,
                        builtin: Some(rule),
                        custom: None,
                    },
                )
                .is_some()
            {
                bail!("Built-in skill rule assets contain duplicate rule IDs");
            }
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
        entries.extend(super::project_inventory::parse(
            config,
            files,
            super::parallel::jobs(),
        )?);
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
                super::builtin_validation::validate(
                    &builtin.id,
                    &builtin.implementation,
                    &builtin.language,
                    setting,
                )?;
            }
            if setting.source.is_none() {
                setting.source = defaults.source;
            }
        }
        for (id, lifecycle) in &config.rule_lifecycle {
            use crate::domain::rule_lifecycle::RuleState;
            let rule = effective
                .rules
                .get_mut(id)
                .with_context(|| format!("Unknown lifecycle rule: {id}"))?;
            match lifecycle.state {
                RuleState::Retired | RuleState::Revoked => rule.enabled = false,
                RuleState::Demoted => {
                    rule.required = false;
                    rule.severity = crate::domain::Severity::Warning;
                }
                RuleState::Revalidate | RuleState::Deprecated => {}
            }
        }
        validation::validate(&effective)?;
        super::source_reviews::evidence(&effective, self)?;
        Ok(effective)
    }
}

#[cfg(test)]
mod supplement_tests {
    use super::{LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS, STANDARD_REGISTRY, standards_from};

    #[test]
    fn supplemental_inputs_are_validated_and_cannot_shadow_primary_ids() {
        assert!(standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS).is_ok());
        let invalid_version =
            LIFECYCLE_SUPPLEMENTS[0].replace("schema_version: 1", "schema_version: 2");
        assert!(standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, &[&invalid_version]).is_err());
        let duplicate = LIFECYCLE_SUPPLEMENTS[0]
            .replace("id: py-eval-exec-review", "id: go-sql-injection-review");
        assert!(standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, &[&duplicate]).is_err());
    }
}
