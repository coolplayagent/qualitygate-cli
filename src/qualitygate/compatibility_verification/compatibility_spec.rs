//! Paired builds and a bounded, digest-pinned compatibility analyzer.

use crate::config::CheckKind;
use crate::config::CommandCheck;
use crate::config::constraints;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilitySpec {
    pub tool: CompatibilityTool,
    pub analyzer_jar: String,
    pub analyzer_sha256: String,
    pub artifacts: Vec<ArchivePair>,
    #[serde(default)]
    pub classpath: Vec<ArchivePair>,
    #[serde(default = "java")]
    pub java: String,
    #[serde(default)]
    pub level: CompatibilityLevel,
    #[serde(default = "timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivePair {
    pub baseline: String,
    pub current: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityTool {
    Japicmp,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityLevel {
    Binary,
    Source,
    #[default]
    Both,
}

fn java() -> String {
    "java".into()
}
fn timeout() -> u64 {
    120
}

pub(crate) fn validate(check: &CommandCheck) -> Result<()> {
    let Some(spec) = &check.compatibility else {
        return Ok(());
    };
    if check.kind != CheckKind::Command
        || check.tools.is_empty()
        || !check.reports.is_empty()
        || !check.projects.is_empty()
        || check.expected_exit_code != 0
        || !check.findings_exit_codes.is_empty()
    {
        bail!(
            "Compatibility needs a successful build command with tool probes and no report/project mappings"
        );
    }
    if spec.analyzer_jar.trim().is_empty()
        || spec.java.trim().is_empty()
        || spec.java.contains(['\0', '\n', '\r'])
        || spec.analyzer_jar.contains(['\0', '\n', '\r'])
        || !valid_digest(&spec.analyzer_sha256)
        || spec.artifacts.is_empty()
        || spec.artifacts.len() > 32
        || spec.classpath.len() > 64
        || !(1..=3600).contains(&spec.timeout_seconds)
    {
        bail!(
            "Compatibility requires a digest-pinned analyzer, Java command, 1..32 archive pairs, at most 64 classpath pairs and a 1..3600 second deadline"
        );
    }
    for baseline in [false, true] {
        let mut outputs = BTreeSet::new();
        for pair in spec.artifacts.iter().chain(&spec.classpath) {
            let path = if baseline {
                &pair.baseline
            } else {
                &pair.current
            };
            if path.is_empty()
                || crate::paths::relative(Path::new(path))? != *path
                || !path.ends_with(".jar")
                || path.contains([';', ':'])
                || !outputs.insert(path)
            {
                bail!("Compatibility outputs must be distinct normalized relative JAR paths");
            }
        }
    }
    constraints::id(&check.id)
}

fn valid_digest(value: &str) -> bool {
    crate::domain::digest::is_sha256(value)
}

#[cfg(test)]
mod scenario_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paired_build_contract_rejects_unpinned_ambiguous_or_unbounded_analyzers() {
        let base = json!({"id":"abi","argv":["build"],"tools":[{"id":"jdk","argv":["java","-version"]}],
            "compatibility":{"tool":"japicmp","analyzer_jar":"tools/analyzer.jar","analyzer_sha256":format!("sha256:{}", "a".repeat(64)),"artifacts":[{"baseline":"old.jar","current":"new.jar"}]}});
        let check: CommandCheck = serde_json::from_value(base.clone()).unwrap();
        validate(&check).unwrap();
        for (field, value) in [
            ("analyzer_sha256", json!("sha256:ABC")),
            ("analyzer_jar", json!("")),
            ("java", json!("java\ncommand")),
            ("timeout_seconds", json!(0)),
            ("timeout_seconds", json!(3601)),
            ("artifacts", json!([])),
            (
                "artifacts",
                json!([{"baseline":"../outside.jar","current":"new.jar"}]),
            ),
            (
                "artifacts",
                json!([{"baseline":"old.zip","current":"new.jar"}]),
            ),
            (
                "artifacts",
                json!([{"baseline":"old.jar","current":"new.jar"},{"baseline":"old.jar","current":"other.jar"}]),
            ),
            (
                "classpath",
                json!([{"baseline":"old.jar","current":"dependency.jar"}]),
            ),
        ] {
            let mut value_base = base.clone();
            value_base["compatibility"][field] = value;
            let check: CommandCheck = serde_json::from_value(value_base).unwrap();
            assert!(validate(&check).is_err(), "{field}");
        }
        for (field, value) in [
            ("expected_exit_code", json!(1)),
            ("tools", json!([])),
            ("findings_exit_codes", json!([2])),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            assert!(
                validate(&serde_json::from_value(changed).unwrap()).is_err(),
                "{field}"
            );
        }
        let mut none = check;
        none.compatibility = None;
        validate(&none).unwrap();
    }
}
