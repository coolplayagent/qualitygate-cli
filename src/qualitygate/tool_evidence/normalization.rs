//! One bounded conversion boundary shared by the gate and the offline bridge.

use super::{contracts::*, decimal::Decimal};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceipt {
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub stderr: String,
    pub tool_id: String,
    pub tool_version: String,
    pub scope_files: Vec<String>,
    #[serde(default)]
    pub findings_exit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizeContext {
    pub spec: OutputSpec,
    pub producer: String,
    pub snapshot_digest: String,
    pub artifact_digest: String,
    pub inventory: Inventory,
    pub coverage: Option<CoverageEvidence>,
    pub execution: Option<ExecutionReceipt>,
}

/// Receipt authenticity is supplied by the application from its actual run.
/// A JSON caller must use `normalize_unbound`, which cannot emit complete facts.
pub fn normalize(raw: &[u8], context: &NormalizeContext) -> Result<ToolFacts> {
    let deadline = Instant::now() + Duration::from_secs(30);
    context.spec.validate()?;
    validate_text(&context.producer)?;
    ensure!(
        crate::content_digest::is_sha256(&context.snapshot_digest),
        "Invalid snapshot identity"
    );
    ensure!(
        raw.len() <= 16 * 1024 * 1024,
        "Raw tool report exceeds 16 MiB"
    );
    ensure!(
        crate::content_digest::sha256(raw) == context.artifact_digest,
        "Raw report digest mismatch"
    );
    validate_inventory(context, deadline)?;
    if let Some(receipt) = &context.execution {
        validate_invocation(&context.spec, &receipt.argv)?;
        let native_findings = context.spec.adapter == ToolFormat::Crapper
            && receipt.exit_code == 2
            && receipt.argv.iter().any(|v| v == "--threshold");
        ensure!(
            receipt.exit_code == 0
                || native_findings
                || context.spec.adapter == ToolFormat::ToolFacts && receipt.findings_exit,
            "Tool did not complete the accepted collection invocation"
        );
        ensure!(
            receipt.tool_id == context.spec.tool_id
                && receipt.tool_version == context.spec.tool_version,
            "Executed tool identity mismatch"
        );
        ensure!(
            set(&receipt.scope_files) == set(&context.inventory.files),
            "Executed scope does not match captured inventory"
        );
        if context.spec.adapter != ToolFormat::ToolFacts {
            // Known tools write one success receipt. Anything else can be a
            // parse warning or an incomplete acquisition and must remain visible.
            ensure!(
                receipt.stderr.lines().all(|line| line.trim().is_empty()
                    || line.starts_with("Wrote ")
                    || native_findings && line.starts_with("CRAP threshold exceeded:")),
                "Native analyzer reported warnings or incomplete acquisition"
            );
        }
    }
    let records = match context.spec.adapter {
        ToolFormat::Crapper => super::crapper::parse(raw, context, deadline)?,
        ToolFormat::Dryer => super::dryer::parse(raw, context, deadline)?,
        ToolFormat::ToolFacts => import_json(raw, context)?,
    };
    let facts = ToolFacts {
        protocol_version: 1,
        schema: context.spec.schema,
        schema_version: 1,
        producer: context.producer.clone(),
        snapshot_digest: context.snapshot_digest.clone(),
        tool_id: context.spec.tool_id.clone(),
        tool_version: context.spec.tool_version.clone(),
        artifact_digest: context.artifact_digest.clone(),
        scope: context.spec.scope.clone(),
        inventory: context.inventory.clone(),
        complete: context.execution.is_some(),
        minimum_lines: context.spec.minimum_lines,
        minimum_nodes: context.spec.minimum_nodes,
        similarity_floor: context.spec.similarity_floor.clone(),
        metric_definitions: context.spec.metric_definitions(),
        clone_measurement: (context.spec.schema == FactSchema::ClonePair)
            .then(|| context.spec.clone_measurement()),
        records,
    };
    validate_records(&facts, context)?;
    if context.spec.adapter == ToolFormat::Crapper
        && let Some(receipt) = &context.execution
        && let Some(at) = receipt.argv.iter().position(|value| value == "--threshold")
    {
        let threshold = Decimal::parse(
            receipt
                .argv
                .get(at + 1)
                .context("Missing crapper threshold")?,
        )?;
        let exceeded = facts.records.iter().any(|record| matches!(record, FactRecord::Metric(metric) if metric.metric == "crap" && metric.value > threshold));
        ensure!(
            exceeded == (receipt.exit_code == 2),
            "Crapper findings exit contradicts retained numeric observations"
        );
    }
    ensure!(
        Instant::now() < deadline,
        "Fact normalization exceeded deadline"
    );
    ensure!(
        serde_json::to_vec(&facts)?.len() <= MAX_FACT_BYTES,
        "Normalized facts exceed 32 MiB"
    );
    Ok(facts)
}

pub(crate) struct RecordBudget {
    bytes: usize,
    deadline: Instant,
}

impl RecordBudget {
    pub(crate) fn new(deadline: Instant, context: &NormalizeContext) -> Result<Self> {
        let bytes = serde_json::to_vec(&(&context.inventory, context.spec.metric_definitions()))?
            .len()
            + 64 * 1024;
        ensure!(bytes <= MAX_FACT_BYTES, "Fact envelope exceeds byte budget");
        Ok(Self { bytes, deadline })
    }
    pub(crate) fn retain(
        &mut self,
        records: &mut Vec<FactRecord>,
        record: FactRecord,
    ) -> Result<()> {
        ensure!(
            Instant::now() < self.deadline,
            "Tool normalization deadline exceeded"
        );
        ensure!(
            records.len() < MAX_RECORDS,
            "Normalized record count exceeds bound"
        );
        // Each record already has bounded fields. Charge before retaining it;
        // inventory and descriptors were charged when creating this budget.
        self.bytes = self
            .bytes
            .checked_add(serde_json::to_vec(&record)?.len() + 1)
            .context("Normalized byte count overflow")?;
        ensure!(
            self.bytes <= MAX_FACT_BYTES,
            "Normalized facts exceed byte budget"
        );
        records.push(record);
        Ok(())
    }
}

pub fn normalize_unbound(raw: &[u8], context: &NormalizeContext) -> Result<ToolFacts> {
    let mut context = context.clone();
    context.execution = None;
    normalize(raw, &context)
}

fn set(values: &[String]) -> BTreeSet<&String> {
    values.iter().collect()
}

fn validate_inventory(context: &NormalizeContext, deadline: Instant) -> Result<()> {
    let inventory = &context.inventory;
    ensure!(
        inventory.files.len() <= MAX_RECORDS && inventory.entities.len() <= MAX_RECORDS,
        "Inventory exceeds bound"
    );
    ensure!(
        set(&inventory.files).len() == inventory.files.len()
            && set(&inventory.files) == set(&context.spec.paths),
        "Source inventory differs from declared scope"
    );
    let mut names = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let files = set(&inventory.files);
    let mut text_bytes = 0usize;
    for entry in &inventory.entities {
        ensure!(
            Instant::now() < deadline,
            "Inventory validation deadline exceeded"
        );
        text_bytes = text_bytes
            .checked_add(
                entry.namespace.len()
                    + entry.name.len()
                    + entry.location.file.len()
                    + entry.location.symbol.len(),
            )
            .context("Inventory byte count overflow")?;
        ensure!(
            text_bytes <= MAX_FACT_BYTES,
            "Inventory text exceeds fact budget"
        );
        validate_text(&entry.namespace)?;
        validate_text(&entry.name)?;
        validate_location(&entry.location)?;
        ensure!(
            files.contains(&entry.location.file),
            "Entity belongs to an uncaptured file"
        );
        ensure!(
            names.insert((&entry.namespace, &entry.name)) && locations.insert(&entry.location),
            "Ambiguous function identity or location"
        );
    }
    Ok(())
}

fn import_json(raw: &[u8], context: &NormalizeContext) -> Result<Vec<FactRecord>> {
    let data: ToolFacts =
        serde_json::from_slice(raw).context("Invalid closed tool-facts protocol")?;
    ensure!(
        data.protocol_version == 1 && data.schema_version == 1,
        "Unsupported tool-facts version"
    );
    ensure!(data.complete, "Producer reported incomplete facts");
    ensure!(
        data.schema == context.spec.schema && data.scope == context.spec.scope,
        "Tool-facts schema or scope mismatch"
    );
    ensure!(
        data.inventory == context.inventory,
        "Tool-facts captured inventory mismatch"
    );
    ensure!(
        data.producer == context.producer && data.snapshot_digest == context.snapshot_digest,
        "Tool-facts producer or snapshot mismatch"
    );
    ensure!(
        data.tool_id == context.spec.tool_id && data.tool_version == context.spec.tool_version,
        "Tool-facts tool identity mismatch"
    );
    ensure!(
        data.minimum_lines == context.spec.minimum_lines
            && data.minimum_nodes == context.spec.minimum_nodes
            && data.similarity_floor == context.spec.similarity_floor,
        "Tool-facts acquisition filters mismatch"
    );
    ensure!(
        data.metric_definitions == context.spec.metric_definitions(),
        "Tool-facts metric inventory mismatch"
    );
    ensure!(
        data.clone_measurement
            == (context.spec.schema == FactSchema::ClonePair)
                .then(|| context.spec.clone_measurement()),
        "Tool-facts clone measurement mismatch"
    );
    // Its envelope cannot hash itself. The retained raw bytes are bound by the
    // application and replace the untrusted producer's artifact_digest claim.
    Ok(data.records)
}

fn validate_records(facts: &ToolFacts, context: &NormalizeContext) -> Result<()> {
    ensure!(
        facts.records.len() <= MAX_RECORDS,
        "Normalized record count exceeds bound"
    );
    let locations: BTreeSet<_> = context
        .inventory
        .entities
        .iter()
        .map(|e| &e.location)
        .collect();
    let definitions: BTreeSet<_> = facts.metric_definitions.iter().cloned().collect();
    let mut ids = BTreeSet::new();
    let mut observations = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for record in &facts.records {
        let id = match record {
            FactRecord::Metric(metric) => {
                ensure!(
                    facts.schema == FactSchema::FunctionMetric
                        && locations.contains(&metric.location),
                    "Metric location/schema is outside independent inventory"
                );
                metric.value.validate()?;
                let definition = MetricDefinition {
                    metric: metric.metric.clone(),
                    unit: metric.unit.clone(),
                    measurement: metric.measurement.clone(),
                };
                ensure!(
                    definitions.contains(&definition),
                    "Unknown metric measurement or unit"
                );
                if metric.unit == "count" {
                    ensure!(
                        metric.value.as_i64()? >= 0,
                        "Count must be a nonnegative i64"
                    );
                }
                ensure!(
                    observations.insert((&metric.location, &metric.metric)),
                    "Duplicate metric observation"
                );
                &metric.id
            }
            FactRecord::ClonePair(pair) => {
                ensure!(
                    facts.schema == FactSchema::ClonePair
                        && locations.contains(&pair.left)
                        && locations.contains(&pair.right),
                    "Clone location/schema is outside independent inventory"
                );
                ensure!(
                    pair.left < pair.right && pairs.insert((&pair.left, &pair.right)),
                    "Clone pair is self-referential, repeated or noncanonical"
                );
                pair.similarity.validate()?;
                ensure!(
                    pair.similarity >= facts.similarity_floor
                        && pair.similarity <= Decimal::from_i64(1),
                    "Clone similarity outside acquired range"
                );
                ensure!(
                    pair.unit == "ratio" && pair.measurement == context.spec.clone_measurement(),
                    "Unknown clone measurement or unit"
                );
                ensure!(
                    pair.left_nodes >= i64::from(facts.minimum_nodes)
                        && pair.right_nodes >= i64::from(facts.minimum_nodes),
                    "Clone node count outside acquired scope"
                );
                ensure!(
                    pair.left.end_line - pair.left.start_line + 1 >= facts.minimum_lines as usize
                        && pair.right.end_line - pair.right.start_line + 1
                            >= facts.minimum_lines as usize,
                    "Clone size outside acquired scope"
                );
                &pair.id
            }
        };
        validate_text(id)?;
        ensure!(ids.insert(id), "Duplicate fact identity");
    }
    if facts.schema == FactSchema::FunctionMetric {
        let expected = locations
            .len()
            .checked_mul(definitions.len())
            .context("Metric inventory overflow")?;
        ensure!(
            observations.len() == expected,
            "Metric report does not cover every entity and declared metric"
        );
    } else if facts.minimum_nodes == 0 && facts.similarity_floor == Decimal::from_i64(0) {
        let count = locations
            .iter()
            .filter(|location| {
                location.end_line - location.start_line + 1 >= facts.minimum_lines as usize
            })
            .count();
        let expected = count
            .checked_mul(count.saturating_sub(1))
            .context("Clone inventory overflow")?
            / 2;
        ensure!(
            pairs.len() == expected,
            "Unfiltered clone facts omit independently expected pairs"
        );
    }
    Ok(())
}

/// Native integration intentionally accepts exact file arguments only. Selection
/// by directories, implicit installation, changed files and hidden filters cannot
/// supply the declared inventory contract.
pub fn validate_invocation(spec: &OutputSpec, argv: &[String]) -> Result<()> {
    ensure!(!argv.is_empty(), "Missing analyzer command");
    if spec.adapter == ToolFormat::ToolFacts {
        return Ok(());
    }
    let mut paths = Vec::new();
    let mut options = BTreeMap::new();
    let mut index = 1;
    while index < argv.len() {
        let arg = &argv[index];
        if arg == "--use-existing-coverage" || arg == "--edn" {
            ensure!(
                options.insert(arg.as_str(), "true").is_none(),
                "Repeated native analyzer flag"
            );
            index += 1;
        } else if ["--threshold", "--min-lines", "--min-nodes"].contains(&arg.as_str()) {
            let value = argv
                .get(index + 1)
                .context("Native analyzer option is missing a value")?;
            ensure!(
                options.insert(arg.as_str(), value).is_none(),
                "Repeated native analyzer option"
            );
            index += 2;
        } else {
            validate_path(arg)?;
            ensure!(
                spec.paths.contains(arg),
                "Native analyzer argument is outside declared source inventory"
            );
            paths.push(arg.clone());
            index += 1;
        }
    }
    ensure!(
        paths.len() == spec.paths.len() && set(&paths) == set(&spec.paths),
        "Native analyzer requires each declared file exactly once"
    );
    match spec.adapter {
        ToolFormat::Crapper => {
            ensure!(
                options.get("--use-existing-coverage") == Some(&"true")
                    && options
                        .keys()
                        .all(|key| ["--use-existing-coverage", "--threshold"].contains(key)),
                "Crapper requires existing verified coverage with no extra selection options"
            );
            if let Some(threshold) = options.get("--threshold") {
                Decimal::parse(threshold)?;
            }
            ensure!(
                !spec.from_stdout,
                "Crapper stdout is human text, not its EDN report"
            );
        }
        ToolFormat::Dryer => {
            let floor = Decimal::parse(
                options
                    .get("--threshold")
                    .context("Explicit dryer threshold is required")?,
            )?;
            let lines: u32 = options
                .get("--min-lines")
                .context("Explicit dryer min-lines is required")?
                .parse()?;
            let nodes: u32 = options
                .get("--min-nodes")
                .context("Explicit dryer min-nodes is required")?
                .parse()?;
            ensure!(
                floor == spec.similarity_floor
                    && lines == spec.minimum_lines
                    && nodes == spec.minimum_nodes,
                "Dryer options differ from declared acquisition filters"
            );
            ensure!(
                !options.contains_key("--use-existing-coverage"),
                "Unsupported dryer option"
            );
            ensure!(
                !spec.from_stdout || options.contains_key("--edn"),
                "Dryer stdout needs --edn"
            );
        }
        ToolFormat::ToolFacts => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
#[path = "normalization_tests.rs"]
mod tests;
