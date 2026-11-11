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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python: Option<PythonResolution>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonResolution {
    pub pip_version: String,
    pub environment: std::collections::BTreeMap<String, String>,
    pub extras: Vec<String>,
    pub install_target: String,
    pub installed_metadata: std::collections::BTreeMap<String, String>,
}

impl ProjectFacts {
    pub fn owns_test(&self, file: &str) -> bool {
        file.strip_prefix(&self.test_source_root)
            .is_some_and(|rest| rest.starts_with('/'))
    }

    pub fn has_test_dependency(&self, group: &str, artifact: &str) -> bool {
        if self.ecosystem == "python" {
            return self.python.is_some()
                && self.declared.iter().any(|dependency| {
                    dependency.group == group
                        && dependency.artifact == artifact
                        && dependency.artifact_type == "distribution"
                        && dependency.classifier.is_empty()
                        && dependency.scope == "environment"
                        && self.resolved.contains(dependency)
                });
        }
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
