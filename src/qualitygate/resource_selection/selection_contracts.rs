//! Serializable, snapshot-bound command applicability evidence; no execution effects.
use serde::{Deserialize, Serialize};

/// Source selectors intersect; additional input patterns independently trigger execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhenChanged {
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
}

/// One side of a changed path, including deleted and renamed source inputs.
#[derive(Debug, Clone)]
pub struct ChangedInput {
    pub path: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub execute: bool,
    pub reason: String,
    /// First matching input (the full changed inventory is bound by the snapshot).
    pub matched_path: Option<String>,
    pub required_by: Option<String>,
}

impl Selection {
    pub fn new(execute: bool, reason: impl Into<String>) -> Self {
        Self {
            execute,
            reason: reason.into(),
            matched_path: None,
            required_by: None,
        }
    }
}
