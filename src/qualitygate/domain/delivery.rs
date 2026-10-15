//! Content-addressed delivery references; paths alone never authorize content.
use super::{Artifact, Report, SnapshotIdentity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivery {
    pub schema_version: u32,
    pub format: String,
    pub artifact: Artifact,
    pub snapshot: SnapshotIdentity,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportReport {
    pub schema_version: u32,
    pub kind: String,
    pub report: Report,
    pub delivery: Option<Delivery>,
}
