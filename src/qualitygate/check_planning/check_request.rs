//! Shared request for planning, capturing and executing a repository check.

use crate::snapshot::{CaptureOptions, Selection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub root: PathBuf,
    pub config: String,
    pub selection: Selection,
    pub snapshot_options: CaptureOptions,
    pub profile: String,
    pub task: Option<String>,
    pub policy_ref: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub trust_store: Option<PathBuf>,
    pub evidence_dir: Option<PathBuf>,
}
