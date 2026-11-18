//! Pure protocol and collection contracts; external claims are never authentication.

use super::decimal::Decimal;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CRAPPER_VERSION: &str = "c757ec79a6c15dc815238bbd9c2f4601eb54d3cc";
pub const DRYER_VERSION: &str = "66ff6d21a42c04afcad89c78a80066176d1294b0";
pub const LANGUAGE_PACK_VERSION: &str = "1.25.2";
pub const LINUX_RUST_PARSER_DIGEST: &str =
    "sha256:43b4551eedd58c54397f9571cacdefdac2ec077eaceb00a43977563c605d03f6";
pub const MAX_RECORDS: usize = 50_000;
pub const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;

/// Whether acquisition retains every serialized score accepted by a source.
/// Dryer filters its unrounded binary score before rounding the EDN value to
/// 12 decimal places. Its consumer therefore needs the entire rounding preimage,
/// not just an equal-looking acquisition threshold. Comparisons remain exact.
pub fn clone_floor_covers(
    measurement: &str,
    acquired: &Decimal,
    requested: &Decimal,
) -> Result<bool> {
    for value in [acquired, requested] {
        value.validate()?;
        ensure!(
            value >= &Decimal::from_i64(0) && value <= &Decimal::from_i64(1),
            "Clone selection floor must be a ratio"
        );
    }
    if requested < acquired {
        return Ok(false);
    }
    if measurement != format!("dryer@{DRYER_VERSION}/structural_jaccard_12dp")
        || acquired == &Decimal::from_i64(0)
    {
        return Ok(true);
    }
    // Both ratios fit exactly in 10^18 integer units; no binary float or
    // tolerance is introduced. One serialized quantum is 10^6 such units.
    let units = |value: &Decimal| -> Result<u64> {
        Ok(value.coefficient.parse::<u64>()? * 10u64.pow(18 - u32::from(value.scale)))
    };
    let first_serialized = units(requested)?.div_ceil(1_000_000) * 1_000_000;
    let acquired_units = units(acquired)?;
    Ok(first_serialized
        .checked_sub(500_000)
        .is_some_and(|lower_preimage| acquired_units <= lower_preimage))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactSchema {
    FunctionMetric,
    ClonePair,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolFormat {
    Crapper,
    Dryer,
    ToolFacts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
}

impl Scope {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.project)?;
        if let Some(target) = &self.target {
            validate_text(target)?;
        }
        if let Some(configuration) = &self.configuration {
            validate_text(configuration)?;
            ensure!(
                self.target.is_some(),
                "Configuration needs an explicit target"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct MetricDefinition {
    pub metric: String,
    pub unit: String,
    pub measurement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricFact {
    pub id: String,
    pub location: Location,
    pub metric: String,
    pub value: Decimal,
    pub unit: String,
    pub measurement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClonePairFact {
    pub id: String,
    pub left: Location,
    pub right: Location,
    pub similarity: Decimal,
    pub unit: String,
    pub measurement: String,
    pub left_nodes: i64,
    pub right_nodes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FactRecord {
    Metric(MetricFact),
    ClonePair(ClonePairFact),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFacts {
    pub protocol_version: u32,
    pub schema: FactSchema,
    pub schema_version: u32,
    pub producer: String,
    pub snapshot_digest: String,
    pub tool_id: String,
    pub tool_version: String,
    pub artifact_digest: String,
    pub scope: Scope,
    pub inventory: Inventory,
    pub complete: bool,
    #[serde(default = "one")]
    pub minimum_lines: u32,
    #[serde(default)]
    pub minimum_nodes: u32,
    #[serde(default = "zero")]
    pub similarity_floor: Decimal,
    #[serde(default)]
    pub metric_definitions: Vec<MetricDefinition>,
    #[serde(default)]
    pub clone_measurement: Option<String>,
    pub records: Vec<FactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSpec {
    pub adapter: ToolFormat,
    pub schema: FactSchema,
    pub path: String,
    #[serde(default)]
    pub from_stdout: bool,
    pub tool_id: String,
    pub tool_version: String,
    pub scope: Scope,
    pub paths: Vec<String>,
    #[serde(default)]
    pub crate_root: String,
    #[serde(default)]
    pub crate_name: String,
    #[serde(default)]
    pub coverage_producer: Option<String>,
    #[serde(default)]
    pub metrics: Vec<MetricDefinition>,
    #[serde(default)]
    pub clone_measurement: Option<String>,
    #[serde(default)]
    pub context_path: Option<String>,
    #[serde(default)]
    pub native_runtime: Option<NativeRuntime>,
    #[serde(default = "one")]
    pub minimum_lines: u32,
    #[serde(default)]
    pub minimum_nodes: u32,
    #[serde(default = "zero")]
    pub similarity_floor: Decimal,
}

/// Explicitly provisioned parser input. The gate never installs or downloads it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntime {
    pub parser_path: String,
    pub parser_digest: String,
    pub pack_version: String,
}

impl NativeRuntime {
    pub fn validate(&self) -> Result<()> {
        validate_path(&self.parser_path)?;
        ensure!(
            self.pack_version == LANGUAGE_PACK_VERSION,
            "Unsupported native language-pack version"
        );
        ensure!(
            self.parser_digest == LINUX_RUST_PARSER_DIGEST,
            "Native parser must match the audited Linux x86_64 grammar"
        );
        Ok(())
    }
}

fn one() -> u32 {
    1
}
fn zero() -> Decimal {
    Decimal::from_i64(0)
}

impl OutputSpec {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate()?;
        validate_text(&self.tool_id)?;
        validate_text(&self.tool_version)?;
        validate_path(&self.path)?;
        if let Some(runtime) = &self.native_runtime {
            runtime.validate()?;
            ensure!(
                self.adapter != ToolFormat::ToolFacts,
                "Generic facts cannot declare a native runtime"
            );
            ensure!(
                runtime.parser_path != self.path && !self.paths.contains(&runtime.parser_path),
                "Native parser input collides with report or source scope"
            );
        }
        ensure!(
            !self.paths.is_empty() && self.paths.len() <= MAX_RECORDS,
            "Tool facts need a bounded explicit source inventory"
        );
        let mut seen = BTreeSet::new();
        for path in &self.paths {
            validate_path(path)?;
            ensure!(seen.insert(path), "Repeated tool source path");
        }
        if !self.crate_root.is_empty() {
            validate_path(&self.crate_root)?;
        }
        validate_text(&self.crate_name)?;
        if let Some(path) = &self.context_path {
            validate_path(path)?;
            ensure!(
                path != &self.path && !self.paths.contains(path),
                "Execution context collides with input or output"
            );
            ensure!(
                self.adapter == ToolFormat::ToolFacts,
                "Native adapters cannot consume generic execution context"
            );
        }
        ensure!(self.minimum_lines > 0, "minimum_lines must be positive");
        self.similarity_floor.validate()?;
        ensure!(
            self.similarity_floor >= zero() && self.similarity_floor <= Decimal::from_i64(1),
            "Similarity floor must be a ratio"
        );
        if let Some(id) = &self.coverage_producer {
            validate_text(id)?;
        }
        match self.adapter {
            ToolFormat::Crapper => {
                ensure!(
                    self.schema == FactSchema::FunctionMetric
                        && self.tool_version == CRAPPER_VERSION,
                    "Unsupported crapper schema/version"
                );
                ensure!(
                    self.coverage_producer.is_some(),
                    "Crapper requires independently verified coverage"
                );
            }
            ToolFormat::Dryer => {
                ensure!(
                    self.schema == FactSchema::ClonePair && self.tool_version == DRYER_VERSION,
                    "Unsupported dryer schema/version"
                );
            }
            ToolFormat::ToolFacts => {}
        }
        ensure!(self.metrics.len() <= 64, "Too many metric definitions");
        let mut names = BTreeSet::new();
        for metric in &self.metrics {
            validate_text(&metric.metric)?;
            validate_text(&metric.unit)?;
            validate_text(&metric.measurement)?;
            ensure!(names.insert(&metric.metric), "Duplicate metric definition");
        }
        if self.adapter == ToolFormat::ToolFacts && self.schema == FactSchema::FunctionMetric {
            ensure!(
                !self.metrics.is_empty(),
                "Metric output must declare its measurement inventory"
            );
        }
        if self.adapter == ToolFormat::ToolFacts && self.schema == FactSchema::ClonePair {
            validate_text(
                self.clone_measurement
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Clone output must declare its measurement"))?,
            )?;
        }
        Ok(())
    }

    pub fn metric_definitions(&self) -> Vec<MetricDefinition> {
        if self.adapter != ToolFormat::Crapper {
            return self.metrics.clone();
        }
        [
            ("cyclomatic_complexity", "count", "rust_cc"),
            ("coverage", "percent", "coverage_4dp"),
            ("crap", "score", "crap_4dp"),
        ]
        .into_iter()
        .map(|(metric, unit, kind)| MetricDefinition {
            metric: metric.into(),
            unit: unit.into(),
            measurement: format!("crapper@{CRAPPER_VERSION}/{kind}"),
        })
        .collect()
    }

    pub fn clone_measurement(&self) -> String {
        if self.adapter == ToolFormat::ToolFacts {
            return self.clone_measurement.clone().unwrap_or_default();
        }
        format!("dryer@{DRYER_VERSION}/structural_jaccard_12dp")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InventoryEntry {
    pub namespace: String,
    pub name: String,
    pub location: Location,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub files: Vec<String>,
    pub entities: Vec<InventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageEvidence {
    pub producer: String,
    pub snapshot_digest: String,
    pub artifact_digests: Vec<String>,
    pub lines: BTreeMap<String, BTreeMap<usize, u64>>,
    pub inputs: BTreeMap<String, String>,
}

pub fn validate_text(text: &str) -> Result<()> {
    ensure!(
        !text.trim().is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control),
        "Empty, oversized or control-bearing fact identifier"
    );
    Ok(())
}

pub fn validate_path(path: &str) -> Result<()> {
    validate_text(path)?;
    ensure!(
        !path.starts_with('/')
            && !path.contains(['\\', ':'])
            && path.split('/').all(|p| !matches!(p, "" | "." | "..")),
        "Tool fact path is not a confined repository-relative path"
    );
    Ok(())
}

pub fn validate_location(location: &Location) -> Result<()> {
    validate_path(&location.file)?;
    validate_text(&location.symbol)?;
    ensure!(
        location.start_line > 0 && location.end_line >= location.start_line,
        "Invalid fact source range"
    );
    Ok(())
}

#[cfg(test)]
mod native_runtime_tests {
    use super::*;
    use serde_json::json;

    fn spec() -> OutputSpec {
        serde_json::from_value(json!({"adapter":"dryer", "schema":"clone_pair", "path":"dry.edn", "tool_id":"dryer", "tool_version":DRYER_VERSION, "scope":{"project":"sample"}, "paths":["src/lib.rs"], "crate_name":"sample", "native_runtime":{"parser_path":"vendor/rust.so", "parser_digest":LINUX_RUST_PARSER_DIGEST, "pack_version":LANGUAGE_PACK_VERSION}})).unwrap()
    }

    #[test]
    fn native_runtime_contract_rejects_unknown_versions_and_unbound_parser_paths() {
        spec().validate().unwrap();
        let mut declaration = spec();
        declaration.native_runtime.as_mut().unwrap().pack_version = "1.25.3".into();
        assert!(declaration.validate().is_err());
        let mut declaration = spec();
        declaration.native_runtime.as_mut().unwrap().parser_digest = "sha256:ABCD".into();
        assert!(declaration.validate().is_err());
        let mut declaration = spec();
        declaration.native_runtime.as_mut().unwrap().parser_path = "../rust.so".into();
        assert!(declaration.validate().is_err());
        let mut declaration = spec();
        declaration.native_runtime.as_mut().unwrap().parser_path = declaration.path.clone();
        assert!(declaration.validate().is_err());
        let mut declaration = spec();
        declaration.adapter = ToolFormat::ToolFacts;
        assert!(declaration.validate().is_err());
    }
}

#[cfg(test)]
mod clone_floor_tests {
    use super::*;

    fn covers(acquired: &str, requested: &str) -> bool {
        clone_floor_covers(
            &format!("dryer@{DRYER_VERSION}/structural_jaccard_12dp"),
            &Decimal::parse(acquired).unwrap(),
            &Decimal::parse(requested).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn rounded_dryer_selection_requires_the_complete_decimal_preimage() {
        assert!(!covers("0.526315789474", "0.526315789474"));
        assert!(covers("0.5263157894735", "0.526315789474"));
        assert!(!covers("0.526315789473500001", "0.526315789474"));
        assert!(covers("0.526315789473499999", "0.526315789474"));
        assert!(!covers("0.82", "0.82"));
        assert!(covers("0.819999999999", "0.82"));
        assert!(covers("0.8199999999995", "0.82"));
        assert!(!covers("0.82", "0.80"));
        assert!(covers("0", "0"));
        assert!(covers("0", "0.000000000000000001"));
        assert!(covers("0.8", "0.800000000000000001"));
        assert!(covers("0.9999999999995", "1"));
        assert!(!covers("1", "1"));
    }

    #[test]
    fn generic_measurements_keep_exact_containment_and_invalid_ratios_are_errors() {
        assert!(
            clone_floor_covers(
                "generic-v1",
                &Decimal::parse("0.82").unwrap(),
                &Decimal::parse("0.82").unwrap()
            )
            .unwrap()
        );
        assert!(
            !clone_floor_covers(
                "generic-v1",
                &Decimal::parse("0.82").unwrap(),
                &Decimal::parse("0.8").unwrap()
            )
            .unwrap()
        );
        for invalid in [
            Decimal::from_i64(-1),
            Decimal::from_i64(2),
            Decimal {
                coefficient: "01".into(),
                scale: 1,
            },
        ] {
            assert!(clone_floor_covers("generic-v1", &invalid, &Decimal::from_i64(1)).is_err());
        }
    }
}
