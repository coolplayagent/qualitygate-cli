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
pub enum SourceKind {
    Issue,
    Commit,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSource {
    pub input_id: String,
    pub kind: SourceKind,
    pub source_id: String,
    pub path: String,
    pub digest: String,
    pub bytes: u64,
    pub selected_at: u64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_ratio_max: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotBudget {
    pub currency: String,
    pub priced_at: u64,
    pub source: String,
    pub max_total_micros: u64,
    pub human_hourly_micros: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotBudgetAssessment {
    pub currency: String,
    pub priced_at: u64,
    pub max_total_micros: u64,
    pub known_model_micros: u64,
    pub known_infrastructure_micros: u64,
    pub known_human_micros: u64,
    pub known_total_micros: u64,
    pub unknown_inputs: usize,
    pub status: ThresholdStatus,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_progress_limit: Option<u16>,
    pub review_fraction_min: f64,
    pub monetary_cap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PilotBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_mix: Option<std::collections::BTreeMap<TaskKind, usize>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_report: Option<Artifact>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_evidence: Option<ModelEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_evidence: Option<ExecutionEvidence>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_sequence: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_evidence: Option<ModelEvidence>,
    pub attempts: Vec<Attempt>,
    pub findings: Vec<Finding>,
    pub review_active_ms: Option<u64>,
    pub review_comments: Option<u64>,
    pub rework_rounds: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelIdentityStatus {
    Reported,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapture {
    pub assignment_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_number: Option<u16>,
    pub captured_at: u64,
    pub agent_version: String,
    pub harness_digest: String,
    pub requested_model: String,
    pub reasoning_effort: String,
    pub actual_model: Option<String>,
    pub status: ModelIdentityStatus,
    pub unknown_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEvidence {
    pub artifact: Artifact,
    pub capture: ModelCapture,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionCapture {
    pub assignment_id: String,
    pub attempt_number: u16,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub harness_digest: String,
    pub status: AttemptStatus,
    pub snapshot_digest: String,
    pub report_digest: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidence {
    pub artifact: Artifact,
    pub capture: ExecutionCapture,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSeal {
    pub algorithm: String,
    pub digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAuthorizationSubject {
    pub repository: String,
    pub pilot_id: String,
    pub plan_seal: PlanSeal,
    pub owner: String,
    pub reviewer: String,
    pub sealed_at: u64,
    pub start_at: u64,
    pub end_at: u64,
    pub task_count: usize,
    pub assignment_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAuthorizationRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub subject: PlanAuthorizationSubject,
    pub authorizer: Actor,
    pub reason: String,
    pub issued_at: u64,
    pub expires_at: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct VerifiedPlanAuthorization {
    pub record: PlanAuthorizationRecord,
    pub signer_key_id: String,
    pub public_key_digest: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdDirection {
    Minimum,
    Maximum,
    AllTrue,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdStatus {
    Met,
    NotMet,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdCheck {
    pub metric: String,
    pub direction: ThresholdDirection,
    pub threshold: Option<f64>,
    pub observed: Option<f64>,
    pub numerator: Option<u64>,
    pub denominator: Option<u64>,
    pub samples: usize,
    pub passed: usize,
    pub failed: usize,
    pub unknown: usize,
    pub status: ThresholdStatus,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotThresholdAssessment {
    pub schema_version: u32,
    pub evaluator: String,
    pub metrics_digest: String,
    pub outcome: ThresholdStatus,
    pub checks: Vec<ThresholdCheck>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotAcceptanceSubject {
    pub repository: String,
    pub pilot_id: String,
    pub plan_seal: PlanSeal,
    pub owner: String,
    pub reviewer: String,
    pub observation_end: u64,
    pub manifest_digest: String,
    pub start_authorization_digest: String,
    pub start_signer_key_id: String,
    pub start_public_key_digest: String,
    pub assessment: PilotThresholdAssessment,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PilotAcceptanceDecision {
    Accepted,
    Rejected,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotAcceptanceRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub subject: PilotAcceptanceSubject,
    pub reviewer: Actor,
    pub decision: PilotAcceptanceDecision,
    pub reason: String,
    pub issued_at: u64,
    pub expires_at: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct VerifiedPilotAcceptance {
    pub record: PilotAcceptanceRecord,
    pub signer_key_id: String,
    pub public_key_digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub protocol: Protocol,
    pub assignments: Vec<Assignment>,
    pub observations: Vec<Observation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<TaskSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run_order: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_seal: Option<PlanSeal>,
}

mod metrics;
pub use metrics::{Reports, summarize, summarize_with_acceptance, summarize_with_authorization};
mod validation;
pub use validation::{authorization_subject, seal, validate};
mod acceptance;
pub use acceptance::{acceptance_decision, acceptance_subject, threshold_assessment};
mod attempt_audit;
mod budget;
mod execution_audit;
pub use attempt_audit::verify_initial_baselines;
mod comparison;
mod rows;
mod schedule;

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
