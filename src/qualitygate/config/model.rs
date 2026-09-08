//! YAML schema version 1. Unknown fields are errors, never silently ignored.

use crate::domain::Severity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RULESETS: &[&str] = &["core", "shared", "lang-java", "lang-python"];

fn schema_one() -> u32 {
    1
}

fn yes() -> bool {
    true
}
fn timeout() -> u64 {
    300
}
fn dot() -> String {
    ".".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub rulesets: Vec<String>,
    #[serde(default)]
    pub rules: BTreeMap<String, RuleSetting>,
    #[serde(default)]
    pub checks: Vec<CommandCheck>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default)]
    pub custom_rules: Option<String>,
    #[serde(default)]
    pub verification_assets: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            languages: Vec::new(),
            rulesets: vec!["core".into()],
            rules: BTreeMap::new(),
            checks: Vec::new(),
            profiles: BTreeMap::new(),
            custom_rules: None,
            verification_assets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "RuleOverrides")]
pub struct RuleSetting {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(skip)]
    pub specified: std::collections::BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleOverrides {
    #[serde(default = "yes")]
    enabled: bool,
    required: Option<bool>,
    severity: Option<Severity>,
    #[serde(default)]
    parameters: BTreeMap<String, serde_json::Value>,
    source: Option<Source>,
}

impl From<RuleOverrides> for RuleSetting {
    fn from(value: RuleOverrides) -> Self {
        let mut specified = std::collections::BTreeSet::new();
        if value.required.is_some() {
            specified.insert("required".into());
        }
        if value.severity.is_some() {
            specified.insert("severity".into());
        }
        Self {
            enabled: value.enabled,
            required: value.required.unwrap_or(true),
            severity: value.severity.unwrap_or_default(),
            parameters: value.parameters,
            source: value.source,
            specified,
        }
    }
}

impl Default for RuleSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            required: true,
            severity: Severity::Error,
            parameters: BTreeMap::new(),
            source: None,
            specified: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub document: String,
    pub section: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub include: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandCheck {
    pub id: String,
    #[serde(default)]
    pub kind: CheckKind,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default = "dot")]
    pub cwd: String,
    #[serde(default = "timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub reports: Vec<ReportSpec>,
    #[serde(default)]
    pub expected_exit_code: i32,
    #[serde(default)]
    pub required_args: Vec<String>,
    #[serde(default)]
    pub evidence_file: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    #[default]
    Command,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportSpec {
    pub path: String,
    pub format: ReportFormat,
    #[serde(default)]
    pub mode: IncrementMode,
    #[serde(default)]
    pub baseline: Option<String>,
    #[serde(default)]
    pub minimum_tests: Option<usize>,
    #[serde(default)]
    pub minimum_coverage: Option<f64>,
    #[serde(default)]
    pub coverage_paths: Vec<String>,
    #[serde(default = "yes")]
    pub require_branch_coverage: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Junit,
    Checkstyle,
    Spotbugs,
    Pmd,
    Sarif,
    Lcov,
    Cobertura,
    Jacoco,
    Diagnostics,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncrementMode {
    #[default]
    Full,
    ChangedLines,
    NewDiagnostics,
    AffectedScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContract {
    pub schema_version: u32,
    pub task_id: String,
    pub acceptance: Vec<Acceptance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub id: String,
    pub description: String,
    pub verification: Verification,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub check_id: String,
    #[serde(default)]
    pub kind: CheckKind,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default = "dot")]
    pub cwd: String,
    #[serde(default = "timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub reports: Vec<ReportSpec>,
    #[serde(default)]
    pub evidence_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRule {
    #[serde(default = "schema_one")]
    pub schema_version: u32,
    pub id: String,
    pub version: u32,
    pub source: Source,
    #[serde(default)]
    pub language: Vec<String>,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub applies_to: AppliesTo,
    #[serde(default)]
    pub requires_capabilities: Vec<String>,
    #[serde(default)]
    pub binding: Option<Binding>,
    pub when: Trigger,
    pub then: Assertions,
    pub fix: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub provenance_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub entity: String,
    #[serde(default)]
    pub change: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assertions {
    #[serde(default)]
    pub require_marker: bool,
    #[serde(default)]
    pub require_dependency: Option<Dependency>,
    #[serde(default)]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub forbid_pattern: Option<String>,
    #[serde(default)]
    pub max_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub marker: Marker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    #[serde(default)]
    pub group: Option<String>,
    pub artifact: String,
}
