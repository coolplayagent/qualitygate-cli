//! Versioned, compiled fixture inputs; goldens are authored independently.

use super::{CustomRule, ReportFormat};
use crate::domain::{CheckResult, ProjectFacts};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const SUITES: &[(&str, &str, &str)] = &[
    (
        "minimal",
        include_str!("../../../fixtures/minimal/cases.json"),
        include_str!("../../../fixtures/golden/minimal.json"),
    ),
    (
        "typical",
        include_str!("../../../fixtures/typical/cases.json"),
        include_str!("../../../fixtures/golden/typical.json"),
    ),
    (
        "stress",
        include_str!("../../../fixtures/stress/cases.json"),
        include_str!("../../../fixtures/golden/stress.json"),
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub id: String,
    pub rule: String,
    pub description: String,
    pub input: Input,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    Evolution {
        scenario: Box<super::selfcheck_policy::EvolutionFixture>,
    },
    Manual {
        record: Box<crate::domain::ManualRecord>,
        expected: Box<crate::domain::ManualSubject>,
        now: u64,
        #[serde(default)]
        tamper: bool,
    },
    Compatibility {
        content: String,
        classes: BTreeSet<String>,
    },
    Snapshot {
        scenario: SnapshotScenario,
    },
    Runner {
        scenario: RunnerScenario,
    },
    Rule {
        #[serde(default)]
        parameters: BTreeMap<String, Value>,
        #[serde(default)]
        files: BTreeMap<String, String>,
        #[serde(default)]
        binary_files: BTreeMap<String, Vec<u8>>,
        #[serde(default)]
        base_files: BTreeMap<String, String>,
        #[serde(default)]
        commits: Vec<String>,
        #[serde(default)]
        projects: Vec<ProjectFacts>,
        #[serde(default)]
        custom: Option<Box<CustomRule>>,
    },
    Report {
        format: ReportFormat,
        content: String,
    },
    Policy {
        content: String,
    },
    Gate {
        checks: Vec<CheckResult>,
        required: Vec<String>,
        invalid: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotScenario {
    Normal,
    Staged,
    Oversized,
    Symlink,
    MissingObject,
    LongPath,
    ChangedInput,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerScenario {
    Success,
    Failure,
    Timeout,
    Overflow,
    MissingTool,
}

#[derive(Debug)]
pub struct Case {
    pub suite: String,
    pub fixture: Fixture,
    pub input: String,
    pub golden: BTreeMap<String, Value>,
}

pub fn load() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (suite, input, golden) in SUITES {
        cases.extend(parse_suite(suite, input, golden)?);
    }
    Ok(cases)
}

fn parse_suite(suite: &str, input: &str, golden: &str) -> Result<Vec<Case>> {
    if input.len() + golden.len() > 2 * 1024 * 1024 {
        bail!("Fixture suite {suite} exceeds 2 MiB");
    }
    let fixtures: Vec<Fixture> = serde_json::from_str(input)
        .with_context(|| format!("Invalid fixtures/{suite}/cases.json"))?;
    // Validate mapping uniqueness before typed maps can overwrite keys.
    let _: serde_norway::Value = serde_norway::from_str(golden)
        .with_context(|| format!("Invalid fixtures/golden/{suite}.json mapping"))?;
    let mut expected: BTreeMap<String, BTreeMap<String, Value>> = serde_json::from_str(golden)
        .with_context(|| format!("Invalid fixtures/golden/{suite}.json"))?;
    if fixtures.is_empty() || fixtures.len() > 512 {
        bail!("Suite {suite} requires 1..512 fixtures");
    }
    let mut ids = BTreeSet::new();
    let mut cases = Vec::new();
    for (index, fixture) in fixtures.into_iter().enumerate() {
        if fixture.id.is_empty()
            || fixture.rule.is_empty()
            || fixture.description.is_empty()
            || !ids.insert(fixture.id.clone())
        {
            bail!("Invalid or duplicate fixture {suite}/{}", fixture.id);
        }
        let golden = expected
            .remove(&fixture.id)
            .with_context(|| format!("Missing golden for {suite}/{}", fixture.id))?;
        if golden.is_empty()
            || golden.keys().any(|pointer| !pointer.starts_with('/'))
            || golden.values().any(|value| {
                value.get("$contains").is_some()
                    && (value.as_object().is_none_or(|object| object.len() != 1)
                        || value["$contains"].as_str().is_none_or(str::is_empty))
            })
        {
            bail!("Empty or invalid assertions for {suite}/{}", fixture.id);
        }
        cases.push(Case {
            suite: (*suite).into(),
            fixture,
            input: format!("fixtures/{suite}/cases.json#/{index}/input"),
            golden,
        });
    }
    if !expected.is_empty() {
        bail!(
            "Orphan goldens in {suite}: {:?}",
            expected.keys().collect::<Vec<_>>()
        );
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_fixture_inventory_is_rejected() {
        let case =
            r#"[{"id":"a","rule":"line-ending","description":"case","input":{"kind":"rule"}}]"#;
        for (input, golden) in [
            ("[]", "{}"),
            ("invalid", "{}"),
            (case, "{}"),
            (case, r#"{"a":{}}"#),
            (case, r#"{"a":{"bad":1}}"#),
            (case, r#"{"a":{"/verdict":"pass"},"b":{}}"#),
        ] {
            assert!(parse_suite("test", input, golden).is_err());
        }
        assert!(parse_suite("test", case, r#"{"a":{"/verdict":"pass"}}"#).is_ok());
        for golden in [
            r#"{"a":{"/verdict":"pass"},"a":{"/verdict":"fail"}}"#,
            r#"{"a":{"/verdict":"pass","/verdict":"fail"}}"#,
            r#"{"a":{"/reason":{"$contains":""}}}"#,
            r#"{"a":{"/reason":{"$contains":"text","ignored":true}}}"#,
        ] {
            assert!(parse_suite("test", case, golden).is_err());
        }
    }
}
