//! Descriptive pilot contracts. Caller observations do not grant acceptance.
use super::{Artifact, evolution::Actor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Real,
    Historical,
    Injected,
    ExtractedModule,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    BugFix,
    Refactor,
    Feature,
    Performance,
    Dependency,
    Documentation,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workflow {
    ExistingTools,
    Qualitygate,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Completed,
    Failed,
    TimedOut,
    Abandoned,
    Incomplete,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    Confirmed,
    FalsePositive,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    ExistingTool,
    QualitygateRule,
    EvidenceGuard,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cohort {
    pub agent_version: String,
    pub harness_digest: String,
    pub requested_model: String,
    pub actual_model: Option<String>,
    pub reasoning_effort: String,
    pub workflow: Workflow,
    pub environment_digest: String,
    pub tools_digest: String,
    pub cache: String,
    pub permissions: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub detection_min: f64,
    pub false_positive_max: f64,
    pub repair_min: f64,
    pub completion_min: f64,
    pub review_reduction_min: f64,
    pub full_p95_ratio_max: f64,
    pub cost_ratio_max: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub project: String,
    pub sampling: String,
    pub owner: Option<String>,
    pub reviewer: Option<String>,
    pub archive: Option<String>,
    pub sealed_at: Option<u64>,
    pub start_at: Option<u64>,
    pub end_at: Option<u64>,
    pub task_count: usize,
    pub days: u16,
    pub max_attempts: u16,
    pub max_seconds: u64,
    pub review_fraction_min: f64,
    pub monetary_cap: Option<String>,
    pub thresholds: Thresholds,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub input_id: String,
    pub id: String,
    pub task_id: String,
    pub task_kind: TaskKind,
    pub origin: Origin,
    pub cohort: Cohort,
    pub base: String,
    pub initial_snapshot: String,
    pub config_digest: String,
    pub task_digest: String,
    pub required_checks: Vec<String>,
    pub expected_issues: Option<Vec<String>>,
    pub eligible_repair: bool,
    pub exclusion: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub currency: String,
    pub priced_at: u64,
    pub source: String,
    pub model_micros: Option<u64>,
    pub infrastructure_micros: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
    pub cache_write_input_tokens: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub number: u16,
    pub status: AttemptStatus,
    pub elapsed_ms: u64,
    pub check_elapsed_ms: Option<u64>,
    pub profile: String,
    pub snapshot_digest: String,
    pub report: Option<Artifact>,
    pub cost: Option<Cost>,
    pub usage: Option<Usage>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub actor: Actor,
    pub label: Label,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub diagnostic_id: String,
    pub issue_id: Option<String>,
    pub report_digest: String,
    pub check_id: String,
    pub fingerprint: String,
    pub attribution: Attribution,
    pub review: Option<Review>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub assignment_id: String,
    pub observed_at: u64,
    pub attempts: Vec<Attempt>,
    pub findings: Vec<Finding>,
    pub review_active_ms: Option<u64>,
    pub review_comments: Option<u64>,
    pub rework_rounds: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub protocol: Protocol,
    pub assignments: Vec<Assignment>,
    pub observations: Vec<Observation>,
}

mod metrics;
pub use metrics::{Reports, summarize};
mod validation;
pub use validation::validate;
mod comparison;
mod rows;

#[cfg(test)]
mod tests;

/// Comparable executable/version identities, excluding per-run paths and source inputs.
pub fn tool_inventory_digest(report: &super::Report) -> String {
    use sha2::{Digest, Sha256};
    let mut producers = super::policy_evaluation::producer_environment(report);
    producers.retain(|_, value| {
        !value["command_executable"].is_null()
            || value["tools"]
                .as_array()
                .is_some_and(|tools| !tools.is_empty())
    });
    for producer in producers.values_mut() {
        if let Some(tools) = producer["tools"].as_array_mut() {
            for tool in tools {
                tool.as_object_mut()
                    .expect("producer tool")
                    .remove("inputs");
            }
        }
    }
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&producers).expect("producer facts"))
    )
}
