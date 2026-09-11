//! A trusted harness attests the complete, ordered sequence of code transformations.

use super::{PolicyBinding, SnapshotBinding};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceSubject {
    pub repository: String,
    pub snapshot: SnapshotBinding,
    pub policy: PolicyBinding,
    pub rule_id: String,
    pub input_content_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub coverage: ProvenanceCoverage,
    pub subject: ProvenanceSubject,
    pub issued_at: u64,
    pub expires_at: u64,
    pub runs: Vec<ProvenanceRun>,
    pub steps: Vec<ProvenanceStep>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceCoverage {
    CompleteComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceRun {
    pub run_id: String,
    pub actor: ProvenanceActor,
    pub started_at: u64,
    pub ended_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceActor {
    pub kind: ActorKind,
    pub id: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Agent,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceStep {
    pub run_id: String,
    pub input_content_digest: String,
    pub output_content_digest: String,
    pub changes: Vec<ProvenanceFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceFileChange {
    pub path: String,
    pub before: Option<ProvenanceFileIdentity>,
    pub after: Option<ProvenanceFileContents>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceFileIdentity {
    pub digest: String,
    pub bytes: usize,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceFileContents {
    pub content_base64: String,
    pub executable: bool,
}
