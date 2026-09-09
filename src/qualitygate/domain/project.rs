//! Normalized project facts from a completed, snapshot-bound ecosystem analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyFact {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub artifact_type: String,
    pub classifier: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFacts {
    pub schema_version: u32,
    pub ecosystem: String,
    pub root: String,
    pub manifest: String,
    pub coordinate: String,
    pub source_root: String,
    pub test_source_root: String,
    pub declared: Vec<DependencyFact>,
    pub resolved: Vec<DependencyFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_usage: Option<DependencyUsage>,
    pub producer_check: String,
    pub snapshot_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyUsage {
    pub analyzer: String,
    pub compiled_main_sources: usize,
    pub compiled_test_sources: usize,
    pub used_undeclared: Vec<DependencyFact>,
}

impl ProjectFacts {
    pub fn owns_test(&self, file: &str) -> bool {
        file.strip_prefix(&self.test_source_root)
            .is_some_and(|rest| rest.starts_with('/'))
    }

    pub fn has_test_dependency(&self, group: &str, artifact: &str) -> bool {
        self.declared.iter().any(|dependency| {
            dependency.group == group
                && dependency.artifact == artifact
                && dependency.artifact_type == "jar"
                && dependency.classifier.is_empty()
                && ["compile", "provided", "system", "test"].contains(&dependency.scope.as_str())
                && self.resolved.contains(dependency)
        })
    }
}
