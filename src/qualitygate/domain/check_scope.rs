//! Explicit verification scope; independent from the selected check profile.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CheckScope {
    Delivery,
    #[default]
    Repository,
}

impl CheckScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delivery => "delivery",
            Self::Repository => "repository",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeEvidence {
    pub mode: CheckScope,
    pub exclude: Vec<String>,
    pub excluded_paths: Vec<String>,
    pub changed_files: Vec<String>,
    pub changed_lines: usize,
    pub execution_context_digest: String,
    pub empty_delivery: bool,
}
